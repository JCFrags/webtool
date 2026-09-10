//! A conventional, pipe-friendly CLI. No alternate screen, mouse handling, or TUI runtime.
use std::{io::{self,Read,Write},path::PathBuf,process::ExitCode,time::Duration};
use anyhow::{anyhow,bail,Context,Result};
use clap::{Parser,Subcommand,ValueEnum};
use serde::{de::DeserializeOwned,Serialize};
use serde_json::{json,Value};
use webtool_protocol::*;

#[derive(Parser)]
#[command(name="webtool",version,about="Search, read, extract, and save sources through a shared server",after_help="No TUI. Results go to stdout. Warnings and progress go to stderr. Use --format json for scripts.")]
struct Cli{
    #[arg(long,global=true,env="WEBTOOL_SERVER",default_value="http://127.0.0.1:8420")]server:String,
    #[arg(long,global=true,value_enum,default_value="text")]format:Output,
    #[arg(long,global=true,default_value_t=120)]timeout:u64,
    #[command(subcommand)]command:Command,
}
#[derive(Clone,Copy,ValueEnum)]enum Output{Text,Markdown,Json,Jsonl}
#[derive(Clone,Copy,ValueEnum)]enum Browser{Auto,Http,Captions,Lightpanda,Chromium,Crw}
impl From<Browser> for Renderer{fn from(v:Browser)->Self{match v{Browser::Auto=>Self::Auto,Browser::Captions=>Self::Captions,Browser::Http=>Self::Http,Browser::Lightpanda=>Self::Lightpanda,Browser::Chromium=>Self::Chromium,Browser::Crw=>Self::Crw}}}
#[derive(Clone,Copy,ValueEnum)]enum Kind{Tables,Links,Code,Images,Metadata,Outline,JsonPointer,Css}
impl From<Kind> for ExtractKind{fn from(v:Kind)->Self{match v{Kind::Tables=>Self::Tables,Kind::Links=>Self::Links,Kind::Code=>Self::Code,Kind::Images=>Self::Images,Kind::Metadata=>Self::Metadata,Kind::Outline=>Self::Outline,Kind::JsonPointer=>Self::JsonPointer,Kind::Css=>Self::Css}}}
#[derive(Clone,Copy,ValueEnum)]enum ExportKind{Markdown,Json,Original,TableCsv}

#[derive(Subcommand)]
enum Command{
    /// Check server reachability and report compiled or configured capabilities.
    Doctor,
    /// Search the web, or a saved library. Use --library '*' for all saved documents.
    Search{#[arg(required=true,num_args=1..)]query:Vec<String>,#[arg(long,default_value_t=10)]limit:usize,#[arg(long)]library:Option<String>},
    /// Read a URL or a saved document ID. URLs are retained automatically.
    Read{source:String,#[arg(long)]refresh:bool,#[arg(long,value_enum,default_value="auto")]renderer:Browser,
        #[arg(long,default_value="en")]language:String,
        #[arg(long)]selector:Option<String>,#[arg(long)]library:Option<String>,#[arg(long)]actor:Option<String>,
        #[arg(long)]start_block:Option<usize>,#[arg(long)]end_block:Option<usize>,#[arg(long)]page:Option<usize>},
    /// Upload a local file. Use '-' for stdin and --name to identify its format.
    Ingest{file:PathBuf,#[arg(long)]name:Option<String>,#[arg(long)]library:Option<String>,#[arg(long)]actor:Option<String>,#[arg(long)]selector:Option<String>},
    /// Find literal text, or a regex, in a saved document or URL.
    Find{source:String,query:String,#[arg(long)]regex:bool,#[arg(long)]ignore_case:bool,#[arg(long,default_value_t=100)]limit:usize},
    /// Extract structures from a saved document or URL. No LLM is used.
    Extract{source:String,#[arg(value_enum)]kind:Kind,#[arg(long)]expression:Option<String>},
    /// List, create, and populate shared named libraries.
    Library{#[command(subcommand)]action:LibraryCommand},
    /// List recently saved documents.
    Saved{#[arg(long,default_value_t=100)]limit:usize},
    /// Add an attributed note and tags. Names identify contributors, not permissions.
    Note{document:String,#[arg(long)]actor:String,#[arg(long,default_value="")]text:String,#[arg(long="tag")]tags:Vec<String>},
    /// Read notes and tags for a document.
    Notes{document:String},
    /// Submit a bounded, same-origin crawl. Robots rules are respected.
    Crawl{url:String,#[arg(long,default_value_t=20)]max_pages:usize,#[arg(long,default_value_t=2)]max_depth:usize,
        #[arg(long)]library:Option<String>,#[arg(long)]actor:Option<String>,#[arg(long)]wait:bool},
    /// List links from a page or URLs from a sitemap, without article extraction.
    Map{url:String,#[arg(long,default_value_t=100)]limit:usize},
    /// List jobs or inspect one. --wait polls status with progress on stderr.
    Jobs{id:Option<String>,#[arg(long,requires="id",conflicts_with="cancel")]wait:bool,#[arg(long,requires="id")]cancel:bool},
    /// Retrieve existing captions through yt-dlp. No video download or transcription.
    Media{url:String,#[arg(long,default_value="en")]language:String,#[arg(long)]library:Option<String>},
    /// Retrieve bibliography metadata through DOI content negotiation.
    Cite{doi:String,#[arg(long="as",default_value="bibtex",value_parser=["bibtex","ris","csl"])]style:String},
    /// Export a saved document. Existing files require --force.
    Export{document:String,#[arg(long,value_enum,default_value="markdown")]kind:ExportKind,#[arg(long,default_value_t=1)]table:usize,#[arg(short,long)]output:PathBuf,#[arg(long)]force:bool},
    /// Read URLs from a UTF-8 file or stdin. Emit one result per line with --format jsonl.
    Batch{file:PathBuf,#[arg(long)]library:Option<String>},
}
#[derive(Subcommand)]enum LibraryCommand{
    List,
    Create{name:String,#[arg(long,default_value="")]description:String},
    Items{name:String,#[arg(long,default_value_t=100)]limit:usize},
    Add{name:String,document:String,#[arg(long)]actor:Option<String>},
}
struct Client{base:String,http:reqwest::Client}
impl Client{
    fn new(server:&str,timeout:u64)->Result<Self>{
        let url=url::Url::parse(server).context("invalid --server URL")?;
        if !matches!(url.scheme(),"http"|"https"){bail!("--server must use HTTP or HTTPS");}
        if timeout==0{bail!("--timeout must be positive");}
        Ok(Self{base:server.trim_end_matches('/').into(),http:reqwest::Client::builder().connect_timeout(Duration::from_secs(10)).timeout(Duration::from_secs(timeout)).build()?})
    }
    async fn get<T:DeserializeOwned>(&self,path:&str)->Result<T>{Self::decode(self.http.get(format!("{}{path}",self.base)).send().await.context("connect to webtoold")?).await}
    async fn post<B:Serialize,T:DeserializeOwned>(&self,path:&str,body:&B)->Result<T>{Self::decode(self.http.post(format!("{}{path}",self.base)).json(body).send().await.context("send request to webtoold")?).await}
    async fn decode<T:DeserializeOwned>(response:reqwest::Response)->Result<T>{
        let status=response.status();let bytes=response.bytes().await?;
        if !status.is_success(){
            let message=serde_json::from_slice::<Problem>(&bytes).map(|p|format!("{}: {}",p.code,p.message))
                .unwrap_or_else(|_|String::from_utf8_lossy(&bytes).chars().take(2000).collect());
            bail!("server returned {status}: {message}");
        }
        Ok(serde_json::from_slice(&bytes).context("server returned an unexpected response shape")?)
    }
    async fn resolve(&self,source:&str)->Result<Document>{
        if source.starts_with("https://")||source.starts_with("http://"){
            let result:ReadResponse=self.post("/v1/read",&ReadRequest{url:source.into(),refresh:false,renderer:Renderer::Auto,language:default_language(),library:None,selector:None,actor:None}).await?;
            Ok(result.document)
        }else{
            document_id(source)?;self.get(&format!("/v1/documents/{source}")).await
        }
    }
    async fn wait(&self,id:&str)->Result<Job>{
        let mut previous=String::new();
        loop{
            let job:Job=self.get(&format!("/v1/jobs/{id}")).await?;
            let message=format!("{id}: {:?}, {} visited, {} saved",job.state,job.visited,job.document_ids.len());
            if message!=previous{eprintln!("{message}");previous=message;}
            if job.state.terminal(){return Ok(job);}
            tokio::select!{
                _=tokio::time::sleep(Duration::from_millis(500))=>{},
                _=tokio::signal::ctrl_c()=>bail!("stopped waiting; the server job remains active. Use jobs {id} --cancel to cancel it"),
            }
        }
    }
}
fn document_id(id:&str)->Result<()>{if id.len()!=64||!id.bytes().all(|b|b.is_ascii_hexdigit()){bail!("expected a full 64-character saved document ID or an HTTP URL");}Ok(())}
fn stdout(text:&str)->Result<()>{let mut out=io::stdout().lock();out.write_all(text.as_bytes())?;out.flush()?;Ok(())}
fn output<T:Serialize>(value:&T,format:Output)->Result<()>{
    let text=if matches!(format,Output::Jsonl){serde_json::to_string(value)?}else{serde_json::to_string_pretty(value)?};
    stdout(&format!("{text}\n"))
}
fn warnings(items:&[Warning]){for w in items{eprintln!("Warning [{}]: {}",w.code,render::terminal_safe(&w.message));}}
fn document(d:&Document,format:Output)->Result<()>{
    warnings(&d.warnings);
    match format{Output::Text=>stdout(&render::plain(d)),Output::Markdown=>stdout(&render::markdown(d)),_=>output(d,format)}
}
fn excerpt(text:&str,start:usize,end:usize)->String{
    let mut a=start.saturating_sub(100);let mut b=end.saturating_add(200).min(text.len());
    while !text.is_char_boundary(a){a+=1;}while !text.is_char_boundary(b){b-=1;}
    format!("{}{}{}",if a>0{"..."}else{""},&text[a..b],if b<text.len(){"..."}else{""})
}
fn input(file:&PathBuf)->Result<Vec<u8>>{
    const LIMIT:u64=128*1024*1024;
    let mut bytes=Vec::new();
    if file.as_os_str()=="-"{io::stdin().lock().take(LIMIT+1).read_to_end(&mut bytes)?;}
    else{std::fs::File::open(file)?.take(LIMIT+1).read_to_end(&mut bytes)?;}
    if bytes.len() as u64>LIMIT{bail!("local input exceeds 128 MiB");}Ok(bytes)
}

fn export_table(document:&Document,index:usize)->Result<Vec<u8>> {
    if index==0 { bail!("table numbers start at 1"); }
    let rows=document.blocks.iter().filter_map(|b|match &b.content { Content::Table{rows}=>Some(rows),_=>None })
        .nth(index-1).context("table not found")?;
    if rows.iter().flatten().any(|c|c.row_span!=1||c.col_span!=1) {
        bail!("CSV cannot preserve merged cells. Export JSON or Markdown instead");
    }
    let mut writer=csv::WriterBuilder::new().has_headers(false).flexible(true).from_writer(Vec::new());
    for row in rows { writer.write_record(row.iter().map(|c|c.text.as_str()))?; }
    Ok(writer.into_inner().map_err(|e|e.into_error())?)
}

async fn run(cli:Cli)->Result<()>{
    let client=Client::new(&cli.server,cli.timeout)?;let format=cli.format;
    match cli.command{
        Command::Doctor=>{
            let h:Health=client.get("/v1/health").await?;
            if matches!(format,Output::Text|Output::Markdown){
                stdout(&format!("webtoold {} | API {}\n",h.version,h.api_version))?;
                for c in h.capabilities{stdout(&format!("{}: {}\n  {}\n",c.name,if c.available{"enabled"}else{"unavailable"},render::terminal_safe(&c.detail)))?;}
                Ok(())
            }else{output(&h,format)}
        },
        Command::Search{query,limit,library}=>{
            let result:SearchResponse=client.post("/v1/search",&SearchRequest{query:query.join(" "),limit,library}).await?;
            warnings(&result.warnings);
            if matches!(format,Output::Text|Output::Markdown){
                for (i,r) in result.results.iter().enumerate(){
                    stdout(&render::terminal_safe(&format!("{}. {}\n   {}\n   {}\n   Providers: {}{}\n\n",i+1,r.title,r.url,
                        textwrap::fill(&r.snippet,88).replace('\n',"\n   "),r.providers.join(", "),r.document_id.as_ref().map(|id|format!(" | ID {id}")).unwrap_or_default())))?;
                }
                if result.results.is_empty(){stdout("No results returned.\n")?;}
            }else{output(&result,format)?;}
            if result.results.is_empty()&&!result.warnings.is_empty(){bail!("search returned no results and reported provider warnings");}Ok(())
        },
        Command::Read{source,refresh,renderer,language,selector,library,actor,start_block,end_block,page}=>{
            let mut d=if source.starts_with("https://")||source.starts_with("http://"){
                let result:ReadResponse=client.post("/v1/read",&ReadRequest{url:source,refresh,renderer:renderer.into(),language,library,selector,actor}).await?;
                if result.cached{eprintln!("Using saved extraction. Pass --refresh to retrieve again.");}result.document
            }else{
                if refresh||selector.is_some()||library.is_some()||actor.is_some()||!matches!(renderer,Browser::Auto)||language!="en"{bail!("retrieval flags apply to URLs, not saved document IDs");}
                client.resolve(&source).await?
            };
            if let Some(number)=page {
                d.blocks.retain(|b|matches!(&b.locator,Locator::Page{number:n,..}|Locator::Slide{number:n} if *n==number));
                if d.blocks.is_empty() { bail!("no source page or slide {number} is available"); }
                d.warnings.push(Warning::new("selected_page",format!("Showing only source page or slide {number}.")));
            }
            if start_block.is_some()||end_block.is_some(){
                let start=start_block.unwrap_or(1);let end=end_block.unwrap_or(d.blocks.len());
                if start==0||end<start||end>d.blocks.len(){bail!("invalid block range for {} blocks",d.blocks.len());}
                d.blocks=d.blocks[start-1..end].to_vec();d.warnings.push(Warning::new("selected_blocks",format!("Showing only blocks {start} through {end}.")));
            }
            document(&d,format)
        },
        Command::Ingest{file,name,library,actor,selector}=>{
            let bytes=input(&file)?;
            let name=name.or_else(||file.file_name().and_then(|s|s.to_str()).filter(|s|*s!="-").map(str::to_owned)).unwrap_or_else(||"stdin.txt".into());
            let mut form=reqwest::multipart::Form::new().part("file",reqwest::multipart::Part::bytes(bytes).file_name(name));
            if let Some(v)=library{form=form.text("library",v);}if let Some(v)=actor{form=form.text("actor",v);}if let Some(v)=selector{form=form.text("selector",v);}
            let response=client.http.post(format!("{}/v1/ingest",client.base)).multipart(form).send().await?;
            let d:Document=Client::decode(response).await?;document(&d,format)
        },
        Command::Find{source,query,regex,ignore_case,limit}=>{
            let d=client.resolve(&source).await?;
            let found:FindResponse=client.post(&format!("/v1/documents/{}/find",d.id),&FindRequest{query,regex,ignore_case,limit}).await?;
            if matches!(format,Output::Text|Output::Markdown){
                for m in &found.matches{
                    stdout(&format!("{} | {}\n",m.block_id,render::terminal_safe(&render::location(&m.locator))))?;
                    let (start,end)=m.ranges.first().map(|r|(r[0],r[1])).unwrap_or((0,0));
                    stdout(&format!("{}\n\n",render::terminal_safe(&excerpt(&m.text,start,end))))?;
                }
                if found.matches.is_empty(){stdout("No matches.\n")?;}
            }else{output(&found,format)?;}
            if found.truncated{eprintln!("Warning: match limit reached.");}Ok(())
        },
        Command::Extract{source,kind,expression}=>{
            let d=client.resolve(&source).await?;let result:ExtractResponse=client.post(&format!("/v1/documents/{}/extract",d.id),&ExtractRequest{kind:kind.into(),expression}).await?;
            warnings(&result.warnings);output(&result,format)
        },
        Command::Library{action}=>match action{
            LibraryCommand::List=>{let v:Vec<Library>=client.get("/v1/libraries").await?;
                if matches!(format,Output::Text|Output::Markdown){for l in v{stdout(&format!("{}  |  {} items  |  {}\n",l.name,l.items,render::terminal_safe(&l.description)))?;}Ok(())}else{output(&v,format)}},
            LibraryCommand::Create{name,description}=>{let v:Library=client.post("/v1/libraries",&LibraryCreate{name,description}).await?;output(&v,format)},
            LibraryCommand::Items{name,limit}=>{let v:Vec<DocumentSummary>=client.get(&format!("/v1/libraries/{name}/items?limit={limit}")).await?;output(&v,format)},
            LibraryCommand::Add{name,document,actor}=>{document_id(&document)?;let v:Value=client.post(&format!("/v1/libraries/{name}/items"),&LibraryAdd{document_id:document,actor}).await?;output(&v,format)},
        },
        Command::Saved{limit}=>{let v:Vec<DocumentSummary>=client.get(&format!("/v1/documents?limit={limit}")).await?;output(&v,format)},
        Command::Note{document,actor,text,tags}=>{document_id(&document)?;let a:Annotation=client.post(&format!("/v1/documents/{document}/annotations"),&AnnotationCreate{actor,note:text,tags}).await?;output(&a,format)},
        Command::Notes{document}=>{document_id(&document)?;let a:Vec<Annotation>=client.get(&format!("/v1/documents/{document}/annotations")).await?;output(&a,format)},
        Command::Crawl{url,max_pages,max_depth,library,actor,wait}=>{
            let j:Job=client.post("/v1/crawl",&CrawlRequest{url,max_pages,max_depth,library,actor}).await?;
            let j=if wait{client.wait(&j.id).await?}else{j};output(&j,format)?;
            if matches!(j.state,JobState::Failed|JobState::Interrupted){bail!("crawl did not complete successfully");}Ok(())
        },
        Command::Map{url,limit}=>{let v:Value=client.post("/v1/map",&json!({"url":url,"limit":limit})).await?;output(&v,format)},
        Command::Jobs{id,wait,cancel}=>{
            if let Some(id)=id{
                if cancel{let v:Value=client.post(&format!("/v1/jobs/{id}/cancel"),&json!({})).await?;return output(&v,format);}
                let j:Job=if wait{client.wait(&id).await?}else{client.get(&format!("/v1/jobs/{id}")).await?};output(&j,format)
            }else{let j:Vec<Job>=client.get("/v1/jobs").await?;output(&j,format)}
        },
        Command::Media{url,language,library}=>{let d:Document=client.post("/v1/media",&json!({"url":url,"language":language,"library":library})).await?;document(&d,format)},
        Command::Cite{doi,style}=>{let v:Value=client.post("/v1/cite",&json!({"doi":doi,"format":style})).await?;
            if matches!(format,Output::Text|Output::Markdown){stdout(&format!("{}\n",render::terminal_safe(v["text"].as_str().context("citation response has no text")?)))}else{output(&v,format)}},
        Command::Export{document:source,kind,table,output:path,force}=>{
            let d=client.resolve(&source).await?;
            if matches!(kind,ExportKind::Original) && d.source.original.role=="rendered_dom" {
                eprintln!("Export source: retained rendered_dom snapshot, not the original HTTP response.");
            }
            let bytes=match kind{
                ExportKind::Markdown=>render::markdown(&d).into_bytes(),ExportKind::Json=>serde_json::to_vec_pretty(&d)?,
                ExportKind::TableCsv=>export_table(&d,table)?,
                ExportKind::Original=>client.http.get(format!("{}/v1/documents/{}/original",client.base,d.id)).send().await?.error_for_status()?.bytes().await?.to_vec(),
            };
            if force{eprintln!("Warning: --force permits replacing {}.",path.display());}
            let mut file=std::fs::OpenOptions::new().write(true).create(force).truncate(force).create_new(!force).open(&path)
                .with_context(||format!("create {} without overwriting existing files",path.display()))?;
            file.write_all(&bytes)?;file.sync_all()?;
            eprintln!("Exported {} bytes to {}",bytes.len(),path.display());Ok(())
        },
        Command::Batch{file,library}=>{
            let data=String::from_utf8(input(&file)?).context("batch file must be UTF-8")?;let mut errors=0;
            for url in data.lines().map(str::trim).filter(|s|!s.is_empty()&&!s.starts_with('#')){
                let result:Result<ReadResponse>=client.post("/v1/read",&ReadRequest{url:url.into(),refresh:false,renderer:Renderer::Auto,language:default_language(),library:library.clone(),selector:None,actor:None}).await;
                match result{Ok(r)=>{
                    if matches!(format,Output::Jsonl){output(&json!({"ok":true,"url":url,"document":r.document}),format)?;}else{document(&r.document,format)?;}
                },Err(e)=>{errors+=1;
                    if matches!(format,Output::Jsonl){output(&json!({"ok":false,"url":url,"error":e.to_string()}),format)?;}else{eprintln!("{}: {e:#}",render::terminal_safe(url));}
                }}
            }
            if errors>0{bail!("{errors} batch inputs failed");}Ok(())
        },
    }
}
#[tokio::main]
async fn main()->ExitCode{
    match run(Cli::parse()).await{
        Ok(())=>ExitCode::SUCCESS,
        Err(e)=>{
            if e.chain().any(|cause|cause.downcast_ref::<io::Error>().is_some_and(|v|v.kind()==io::ErrorKind::BrokenPipe)){return ExitCode::SUCCESS;}
            eprintln!("webtool: {}",render::terminal_safe(&format!("{e:#}")));ExitCode::from(1)
        },
    }
}
#[cfg(test)]mod tests{
    use super::*;
    use clap::CommandFactory;
    #[test]fn clap_definition_is_consistent(){Cli::command().debug_assert();}
    #[test]fn read_flags_parse(){assert!(Cli::try_parse_from(["webtool","read","https://example.com","--renderer","chromium","--format","json"]).is_ok());}
    #[test]fn no_tui_subcommand(){assert!(Cli::try_parse_from(["webtool","tui"]).is_err());}
    #[test]fn query_words_are_accepted(){assert!(Cli::try_parse_from(["webtool","search","Rust","async","--limit","5"]).is_ok());}
    #[test]fn excerpts_respect_utf8(){let s="é漢字hello🦀";assert!(excerpt(s,5,8).contains("hello"));}
    #[test]fn rejects_short_id(){assert!(document_id("abc").is_err());}
}
