//! Plain output and Markdown share the same retained document blocks.
use crate::{Content,Document,Locator};
use std::borrow::Cow;
use std::collections::{BTreeMap,HashMap};

pub fn location(l:&Locator)->String{
    match l{
        Locator::Html{selector}=>format!("HTML {selector}"),
        Locator::Lines{start,end}=>format!("lines {start}-{end}"),
        Locator::Timestamp{start_ms,end_ms}=>format!("{}-{}",time(*start_ms),time(*end_ms)),
        Locator::Page{number,bbox}=>match bbox{Some(b)=>format!("page {number}, box {b:?}"),None=>format!("page {number}")},
        Locator::Cell{sheet,row,column}=>format!("sheet {sheet}, row {row}, column {column}"),
        Locator::Sheet{name}=>format!("sheet {name}"),
        Locator::Slide{number}=>format!("slide {number}"),
        Locator::JsonPointer{pointer}=>format!("JSON {pointer}"),
        Locator::Derived{index}=>format!("derived block {index}, no exact source position"),
    }
}
fn time(ms:u64)->String{format!("{:02}:{:02}:{:02}.{:03}",ms/3600000,(ms/60000)%60,(ms/1000)%60,ms%1000)}
fn html(s:&str)->String{s.replace('&',"&amp;").replace('<',"&lt;").replace('>',"&gt;")}
fn md_cell(s:&str)->String{s.replace('\\',"\\\\").replace('|',"\\|").replace('\n',"<br>")}

struct InlineLink<'a>{start:usize,end:usize,url:&'a str}

fn http_link(url:&str)->bool{
    let Some((scheme,rest))=url.split_once("://") else{return false;};
    if !(scheme.eq_ignore_ascii_case("http")||scheme.eq_ignore_ascii_case("https"))
        ||url.chars().any(|c|c.is_control()||c.is_whitespace()||c=='\\'){return false;}
    let authority=rest.split(['/', '?', '#']).next().unwrap_or("");
    // No URL dependency is needed for this conservative rendering boundary.
    // Reject credentials, missing hosts, invalid ports, and malformed IPv6 hosts.
    let port=if let Some(host)=authority.strip_prefix('['){
        let Some((host,suffix))=host.split_once(']') else{return false;};
        if host.parse::<std::net::Ipv6Addr>().is_err(){return false;}
        if suffix.is_empty(){None}else{let Some(port)=suffix.strip_prefix(':') else{return false;};Some(port)}
    }else{
        let (host,port)=authority.split_once(':').map_or((authority,None),|(host,port)|(host,Some(port)));
        if host.is_empty()||!host.chars().all(|c|c.is_alphanumeric()||matches!(c,'.'|'-'|'_')){return false;}
        port
    };
    port.map_or(true,|port|!port.is_empty()&&port.bytes().all(|b|b.is_ascii_digit())&&port.parse::<u16>().is_ok())
}

// Spans describe the unchanged block text, not the document's all-page link list.
// Reject the whole block's metadata if any span is invalid. Do not guess a repair.
fn inline_links<'a>(d:&'a Document,id:&str,text:&str)->Option<Vec<InlineLink<'a>>>{
    let values=d.metadata.get("inline_links")?.as_object()?.get(id)?.as_array()?;
    if values.is_empty(){return None;}
    let mut links=Vec::with_capacity(values.len());
    let mut previous_end=0;
    for value in values{
        let start=usize::try_from(value.get("start")?.as_u64()?).ok()?;
        let end=usize::try_from(value.get("end")?.as_u64()?).ok()?;
        let url=value.get("url")?.as_str()?;
        if start<previous_end||start>=end||text.get(start..end)?.trim().is_empty()||!http_link(url){return None;}
        links.push(InlineLink{start,end,url});
        previous_end=end;
    }
    Some(links)
}

fn md_inline_text(out:&mut String,text:&str,label:bool){
    for c in text.chars(){
        match c{
            '&'=>out.push_str("&amp;"),'<'=>out.push_str("&lt;"),'>'=>out.push_str("&gt;"),
            '\\'|'`'|'*'|'_'|'['|']'|'!'|'~'|'|'=>{out.push('\\');out.push(c);},
            '\n' if label=>out.push_str("&#10;"), '\r' if label=>out.push_str("&#13;"),
            _=>out.push(c),
        }
    }
}

fn markdown_inline<'a>(d:&Document,id:&str,text:&'a str)->Cow<'a,str>{
    let Some(links)=inline_links(d,id,text) else{return Cow::Borrowed(text);};
    let mut out=String::new();
    let mut end=0;
    for link in links{
        md_inline_text(&mut out,&text[end..link.start],false);
        out.push('[');md_inline_text(&mut out,&text[link.start..link.end],true);
        // Angle destinations retain URL parentheses. Entities protect literal &, <, and >.
        out.push_str("](<");out.push_str(&html(link.url));out.push_str(">)");
        end=link.end;
    }
    md_inline_text(&mut out,&text[end..],false);
    Cow::Owned(out)
}

fn plain_actions(d:&Document,block:&crate::Block)->Option<String>{
    let Content::Paragraph{text}=&block.content else{return None;};
    let links=inline_links(d,&block.id,text)?;
    let mut end=0;
    let mut lines=Vec::with_capacity(links.len());
    for link in links{
        if !text[end..link.start].trim().is_empty(){return None;}
        let label=text[link.start..link.end].split_whitespace().collect::<Vec<_>>().join(" ");
        lines.push(format!("{label}: {}",link.url));
        end=link.end;
    }
    if !text[end..].trim().is_empty(){return None;}
    Some(lines.join("\n"))
}

#[derive(Clone,Copy)]
struct ListPosition{depth:usize,ordinal:Option<i64>,first:bool}

fn list_position(d:&Document,id:&str)->Option<ListPosition>{
    let value=d.metadata.get("list_items")?.as_object()?.get(id)?.as_object()?;
    let depth=value.get("depth")?.as_u64()?;
    if depth>32{return None;}
    let ordinal=value.get("ordinal")?;
    let ordinal=if ordinal.is_null(){None}else{Some(ordinal.as_i64()?)};
    Some(ListPosition{depth:depth as usize,ordinal,first:value.get("first")?.as_bool()?})
}

struct InlineFlow<'a>{start:usize,end:usize,separators:Vec<&'a str>}

fn inline_flow<'a>(d:&Document,value:&'a serde_json::Value,indices:&HashMap<&str,Option<usize>>)->Option<InlineFlow<'a>>{
    let ids=value.get("blocks")?.as_array()?;
    let separators=value.get("separators")?.as_array()?;
    if ids.len()<2||ids.len()>d.blocks.len()||separators.len()!=ids.len()-1{return None;}
    let start=indices.get(ids[0].as_str()?).copied().flatten()?;
    let end=start.checked_add(ids.len())?;
    let blocks=d.blocks.get(start..end)?;
    let list_values=match d.metadata.get("list_items"){Some(value)=>Some(value.as_object()?),None=>None};
    let first_list=list_position(d,&blocks[0].id);
    for (offset,(id,block)) in ids.iter().zip(blocks).enumerate(){
        if indices.get(id.as_str()?).copied().flatten()!=Some(start+offset)
            ||!matches!(block.content,Content::Paragraph{..}|Content::Math{..}){return None;}
        let list=list_position(d,&block.id);
        if list_values.is_some_and(|values|values.contains_key(&block.id))&&list.is_none(){return None;}
        match (first_list,list){
            (None,None)=>{},
            (Some(first),Some(item)) if first.depth==item.depth&&first.ordinal==item.ordinal&&(offset==0||!item.first)=>{},
            _=>return None,
        }
    }
    let separators=separators.iter().map(|value|match value.as_str(){Some(s @ (""|" "))=>Some(s),_=>None}).collect::<Option<Vec<_>>>()?;
    Some(InlineFlow{start,end,separators})
}

// Validate before rendering so every overlapping group falls back, not only the later one.
fn inline_flows(d:&Document)->BTreeMap<usize,InlineFlow<'_>>{
    let mut result=BTreeMap::new();
    let Some(values)=d.metadata.get("inline_flows").and_then(|value|value.as_array()) else{return result;};
    if values.is_empty(){return result;}
    let mut indices=HashMap::with_capacity(d.blocks.len());
    for (index,block) in d.blocks.iter().enumerate(){
        indices.entry(block.id.as_str()).and_modify(|index|*index=None).or_insert(Some(index));
    }
    let mut occupied:Vec<Option<usize>>=vec![None;d.blocks.len()];
    let mut flows=Vec::new();
    let mut overlaps=Vec::new();
    for value in values{
        let Some(flow)=inline_flow(d,value,&indices) else{continue;};
        let index=flows.len();
        let mut overlap=false;
        for owner in &mut occupied[flow.start..flow.end]{
            if let Some(previous)=*owner{overlaps[previous]=true;overlap=true;}else{*owner=Some(index);}
        }
        flows.push(flow);overlaps.push(overlap);
    }
    for (flow,overlap) in flows.into_iter().zip(overlaps){if !overlap{result.insert(flow.start,flow);}}
    result
}

#[derive(Clone,Copy)]
enum InlineView{Plain,Markdown,Export}

fn render_inline_flow(d:&Document,flow:&InlineFlow<'_>,view:InlineView)->String{
    // Separators retain source boundaries. Do not trim fragments or infer punctuation spacing.
    let mut out=String::new();
    for (index,block) in d.blocks[flow.start..flow.end].iter().enumerate(){
        if index>0{out.push_str(flow.separators[index-1]);}
        match &block.content{
            Content::Paragraph{text}=>{
                if matches!(view,InlineView::Plain){out.push_str(text);}
                else{out.push_str(&markdown_inline(d,&block.id,text));}
            },
            Content::Math{text}=>out.push_str(if matches!(view,InlineView::Export){text}else{display_math(text)}),
            _=>{}, // Validation excludes all other block types.
        }
    }
    out.push_str("\n\n");out
}

struct ListLayout{content_indents:[Option<usize>;33],ordinals:[Option<i64>;33],open:bool}
impl ListLayout{
    fn new()->Self{Self{content_indents:[None;33],ordinals:[None;33],open:false}}

    // Indent the rendered view only. Keep stored code, table, and math text unchanged.
    fn append(&mut self,out:&mut String,body:&str,position:Option<ListPosition>,markdown:bool){
        let Some(position)=position else{
            if self.open&&!out.ends_with("\n\n"){out.push('\n');}
            self.content_indents.fill(None);self.ordinals.fill(None);self.open=false;out.push_str(body);return;
        };
        let base=if position.depth==0{0}else{self.content_indents[position.depth-1].unwrap_or(position.depth*4)};
        // CommonMark cannot express negative or more than nine-digit ordered markers.
        // Keep those numbers as explicit labels inside a normal bullet item.
        let explicit_number=position.ordinal.filter(|n|markdown&&!(0..=999_999_999).contains(n));
        let marker=match position.ordinal{
            Some(n) if explicit_number.is_none()=>format!("{n}. "),_=>"- ".into(),
        };
        let content_indent=base+marker.len();
        let base_prefix=" ".repeat(base);
        let content_prefix=" ".repeat(content_indent);
        let previous=self.ordinals[position.depth];
        let ordered_start=markdown&&position.first&&explicit_number.is_none()&&position.ordinal.is_some();
        let number_break=ordered_start&&previous.is_some_and(|n|(0..=999_999_999).contains(&n)&&n.checked_add(1)!=position.ordinal);
        let new_ordered_list=ordered_start&&previous.is_none()&&position.ordinal!=Some(1);
        self.content_indents[position.depth]=Some(content_indent);
        self.content_indents[position.depth+1..].fill(None);
        if position.first{self.ordinals[position.depth]=position.ordinal;}
        self.ordinals[position.depth+1..].fill(None);
        if (!self.open||!position.first||number_break||new_ordered_list)&&!out.ends_with("\n\n")&&!out.is_empty(){out.push('\n');}
        // Separate Markdown lists at a retained ordinal jump so renderers cannot renumber it.
        if number_break{out.push_str(&format!("{base_prefix}<!-- retained list ordinal -->\n\n"));}
        self.open=true;
        if position.first{
            if let Some(number)=explicit_number{out.push_str(&format!("{base_prefix}- {number}.\n\n"));}
        }
        for (index,line) in body.split_inclusive('\n').enumerate(){
            if index==0&&position.first&&explicit_number.is_none(){
                out.push_str(&base_prefix);out.push_str(&marker);
            }else if line!="\n"{out.push_str(&content_prefix);}
            out.push_str(line);
        }
    }
}

pub fn supplemental_table(d:&Document, id:&str)->bool {
    d.metadata.get("supplemental_table_blocks").and_then(|v|v.as_array())
        .is_some_and(|items|items.iter().any(|v|v.as_str()==Some(id)))
}

pub fn markdown(d:&Document)->String{
    let mut out=format!("# {}\n\nSource: {}\nRetrieved: {}\nDocument: {}\n\n",d.title,d.source.resolved,d.source.retrieved_at,d.id);
    let mut lists=ListLayout::new();
    let flows=inline_flows(d);
    let mut flow_end=0;
    for (index,block) in d.blocks.iter().enumerate(){
        if index<flow_end{continue;}
        let list=list_position(d,&block.id);
        if let Some(flow)=flows.get(&index){
            let body=render_inline_flow(d,flow,InlineView::Export);
            lists.append(&mut out,&body,list,true);flow_end=flow.end;continue;
        }
        let start=out.len();
        if matches!(block.content,Content::Table{..}) && supplemental_table(d,&block.id) {
            out.push_str("Supplemental structured table (may repeat page text):\n\n");
        }
        match &block.content{
            Content::Heading{level,text}=>out.push_str(&format!("{} {}\n\n","#".repeat((*level).clamp(1,6) as usize),markdown_inline(d,&block.id,text))),
            Content::Paragraph{text}=>out.push_str(&format!("{}\n\n",markdown_inline(d,&block.id,text))),
            Content::Code{language,text}=>{
                let max_run=text.split(|c|c!='`').map(str::len).max().unwrap_or(0);
                let fence="`".repeat((max_run+1).max(3));
                let lang=language.as_deref().unwrap_or("").chars().filter(|c|c.is_ascii_alphanumeric()||"_+-#".contains(*c)).collect::<String>();
                out.push_str(&format!("{fence}{lang}\n{text}{}{}\n\n",if text.ends_with('\n'){""}else{"\n"},fence));
            },
            Content::Quote{text}=>{for line in markdown_inline(d,&block.id,text).lines(){out.push_str(&format!("> {line}\n"));}out.push('\n');},
            Content::ListItem{text,ordered}=>{
                let text=markdown_inline(d,&block.id,text);
                if list.is_some(){out.push_str(&format!("{text}\n"));}
                else{out.push_str(&format!("{} {text}\n",if *ordered{"1."}else{"-"}));}
            },
            Content::Table{rows}=>markdown_table(&mut out,rows),
            Content::Image{url,alt}=>out.push_str(&format!("Image: {alt}\n{url}\n\n")),
            Content::Math{text}=>out.push_str(&format!("{text}\n\n")),
            Content::Caption{text}=>out.push_str(&format!("[{}] {}\n\n",location(&block.locator),markdown_inline(d,&block.id,text))),
        }
        let body=out.split_off(start);lists.append(&mut out,&body,list,true);
    }
    lists.append(&mut out,"",None,true);
    if !d.links.is_empty(){out.push_str("## Links\n\n");for l in &d.links{out.push_str(&format!("- {}: {}\n",l.text,l.url));}out.push('\n');}
    if !d.warnings.is_empty(){out.push_str("## Extraction warnings\n\n");for w in &d.warnings{out.push_str(&format!("- {}: {}\n",w.code,w.message));}}
    out
}
/// Markdown for interactive reads. Export also includes retained links and warnings.
pub fn markdown_read(d:&Document,details:bool)->String{
    let mut out=format!("# {}\n\nSource: {}\n",d.title,d.source.resolved);
    if details{
        out.push_str(&format!("Retrieved: {}\nStatus: {}\nParser: {}\nExtraction: {}\nOriginal: {} bytes, {}, {}, SHA-256 {}\n",
            d.source.retrieved_at,d.source.status.map(|v|v.to_string()).unwrap_or_else(||"unknown".into()),d.parser,d.extraction_version,
            d.source.original.size,d.source.original.media_type,d.source.original.role,d.source.original.sha256));
    }
    out.push_str(&format!("Saved ID: {}\n\n",d.id));
    let mut previous_group:Option<String>=None;
    let mut list_open=false;
    let mut lists=ListLayout::new();
    let flows=if details{BTreeMap::new()}else{inline_flows(d)};
    let mut flow_end=0;
    for (index,block) in d.blocks.iter().enumerate(){
        if index<flow_end{continue;}
        let list=list_position(d,&block.id);
        if !details && index==0 && list.is_none() && matches!(&block.content,Content::Heading{text,..} if text.trim()==d.title.trim()&&inline_links(d,&block.id,text).is_none()){continue;}
        let group=source_group(&block.locator);
        if !details && group!=previous_group{
            if let Some(label)=&group{out.push_str(&format!("**{label}**\n\n"));}
        }
        if group.is_some(){previous_group=group;}
        let is_list=list.is_some()||matches!(block.content,Content::ListItem{..});
        if list_open&&!is_list{out.push('\n');}
        list_open=is_list;
        if let Some(flow)=flows.get(&index){
            let body=render_inline_flow(d,flow,InlineView::Markdown);
            lists.append(&mut out,&body,list,true);flow_end=flow.end;continue;
        }
        let start=out.len();
        if details{out.push_str(&format!("*{} | {}*\n\n",block.id,location(&block.locator)));}
        match &block.content{
            Content::Heading{level,text}=>out.push_str(&format!("{} {}\n\n","#".repeat((*level).clamp(1,6) as usize),markdown_inline(d,&block.id,text))),
            Content::Paragraph{text}=>out.push_str(&format!("{}\n\n",markdown_inline(d,&block.id,text))),
            Content::Code{language,text}=>{
                let max_run=text.split(|c|c!='`').map(str::len).max().unwrap_or(0);
                let fence="`".repeat((max_run+1).max(3));
                let lang=language.as_deref().unwrap_or("").chars().filter(|c|c.is_ascii_alphanumeric()||"_+-#".contains(*c)).collect::<String>();
                out.push_str(&format!("{fence}{lang}\n{text}{}{}\n\n",if text.ends_with('\n'){""}else{"\n"},fence));
            },
            Content::Quote{text}=>{for line in markdown_inline(d,&block.id,text).lines(){out.push_str(&format!("> {line}\n"));}out.push('\n');},
            Content::ListItem{text,ordered}=>{
                let text=markdown_inline(d,&block.id,text);
                if list.is_some(){out.push_str(&format!("{text}\n"));}
                else{out.push_str(&format!("{} {text}\n",if *ordered{"1."}else{"-"}));}
            },
            Content::Table{rows}=>{
                if supplemental_table(d,&block.id){out.push_str("Supplemental structured table (may repeat page text):\n\n");}
                markdown_table(&mut out,rows);
            },
            Content::Image{url,alt}=>{
                if !alt.is_empty(){if details{out.push_str(&format!("![{alt}]({url})\n\n"));}else{out.push_str(&format!("Image: {alt}\n\n"));}}
            },
            Content::Math{text}=>out.push_str(&format!("{}\n\n",display_math(text))),
            Content::Caption{text}=>out.push_str(&format!("[{}] {}\n\n",caption_location(&block.locator),markdown_inline(d,&block.id,text))),
        }
        let body=out.split_off(start);lists.append(&mut out,&body,list,true);
    }
    if list_open{out.push('\n');}
    if details&&!d.links.is_empty(){out.push_str("## Links\n\n");for link in &d.links{out.push_str(&format!("- {}: {}\n",link.text,link.url));}out.push('\n');}
    out
}

/// Escape terminal control sequences only in the terminal view. JSON and original exports remain lossless.
pub fn terminal_safe(s:&str)->String{
    s.chars().map(|c|if (c.is_control()&&c!='\n'&&c!='\t')||('\u{80}'..='\u{9f}').contains(&c){c.escape_unicode().to_string()}else{c.to_string()}).collect()
}
fn markdown_table(out:&mut String,rows:&[Vec<crate::Cell>]){
    if rows.is_empty(){return;}
    let width=rows[0].len();
    let rectangular=rows.iter().all(|r|r.len()==width&&r.iter().all(|c|c.col_span==1&&c.row_span==1));
    let header=rows[0].iter().all(|c|c.header);
    if rectangular&&header{
        for (i,row) in rows.iter().enumerate(){
            out.push_str(&format!("| {} |\n",row.iter().map(|c|md_cell(&c.text)).collect::<Vec<_>>().join(" | ")));
            if i==0{out.push_str(&format!("| {} |\n",vec!["---";width].join(" | ")));}
        }
    }else{
        out.push_str("<table>\n");for row in rows{out.push_str("<tr>");for c in row{
            let tag=if c.header{"th"}else{"td"};out.push_str(&format!("<{tag} rowspan=\"{}\" colspan=\"{}\">{}</{tag}>",c.row_span,c.col_span,html(&c.text)));
        }out.push_str("</tr>\n");}out.push_str("</table>\n");
    }
    out.push('\n');
}

fn raw_mathml(text:&str)->bool{
    let value=text.trim_start().to_ascii_lowercase();
    value.starts_with("<math")||value.contains("</math>")||value.contains("mathml")
}
fn display_math(text:&str)->&str{
    if raw_mathml(text){"[Equation notation is available only in the retained source.]"}else{text}
}
fn caption_location(locator:&Locator)->String{
    match locator{Locator::Timestamp{..}|Locator::Page{..}|Locator::Slide{..}=>location(locator),_=>"Caption".into()}
}
fn source_group(locator:&Locator)->Option<String>{
    match locator{
        Locator::Page{number,..}=>Some(format!("Page {number}")),
        Locator::Slide{number}=>Some(format!("Slide {number}")),
        Locator::Sheet{name}=>Some(format!("Sheet {name}")),
        _=>None,
    }
}

// Only lay out rows whose retained spans establish the same column count.
fn table_columns(rows:&[Vec<crate::Cell>])->Option<usize>{
    let mut columns=None;
    for row in rows{
        let width=row.iter().try_fold(0usize,|width,cell|{
            if cell.row_span!=1 || cell.col_span==0{None}else{width.checked_add(cell.col_span)}
        })?;
        if width==0 || columns.is_some_and(|n|n!=width){return None;}
        columns=Some(width);
    }
    columns
}

// Printable ASCII has a known terminal width. Keep source line breaks and use
// a nonaligned view for other characters, controls, or tables wider than 88 columns.
fn plain_table_grid(rows:&[Vec<crate::Cell>],columns:usize)->Option<String>{
    const LIMIT:usize=88;
    if columns>(LIMIT-1)/4{return None;}
    let mut widths=vec![1usize;columns];
    for row in rows{
        let mut column=0;
        for cell in row{
            if !cell.text.bytes().all(|b|b==b'\n'||(b' '..=b'~').contains(&b)){return None;}
            let width=cell.text.split('\n').map(str::len).max().unwrap_or(0);
            if width>LIMIT-4{return None;}
            if cell.col_span==1{widths[column]=widths[column].max(width);}
            column+=cell.col_span;
        }
    }
    // Merged cells reuse the space otherwise occupied by internal separators.
    for row in rows{
        let mut column=0;
        for cell in row{
            let end=column+cell.col_span;
            if cell.col_span>1{
                let available=widths[column..end].iter().sum::<usize>()+3*(cell.col_span-1);
                let needed=cell.text.split('\n').map(str::len).max().unwrap_or(0);
                let extra=needed.saturating_sub(available);
                for (i,width) in widths[column..end].iter_mut().enumerate(){
                    *width+=extra/cell.col_span+usize::from(i<extra%cell.col_span);
                }
            }
            column=end;
        }
    }
    if widths.iter().sum::<usize>()+3*columns+1>LIMIT{return None;}
    let border=format!("+{}+\n",widths.iter().map(|w|"-".repeat(w+2)).collect::<Vec<_>>().join("+"));
    let header_border=border.replace('-',"=");
    let mut out=border.clone();
    for row in rows{
        let lines:Vec<Vec<&str>>=row.iter().map(|cell|cell.text.split('\n').collect()).collect();
        let height=lines.iter().map(Vec::len).max().unwrap_or(1);
        for line in 0..height{
            out.push('|');
            let mut column=0;
            for (cell,lines) in row.iter().zip(&lines){
                let end=column+cell.col_span;
                let width=widths[column..end].iter().sum::<usize>()+3*(cell.col_span-1);
                let text=lines.get(line).copied().unwrap_or("");
                out.push_str(&format!(" {text:width$} |"));
                column=end;
            }
            out.push('\n');
        }
        out.push_str(if row.iter().all(|cell|cell.header){&header_border}else{&border});
    }
    Some(out)
}

fn plain_table_cell(cell:&crate::Cell,show_header:bool)->String{
    let mut notes=Vec::new();
    if show_header && cell.header{notes.push("header".into());}
    if cell.row_span!=1{notes.push(format!("rowspan={}",cell.row_span));}
    if cell.col_span!=1{notes.push(format!("colspan={}",cell.col_span));}
    let text=if cell.text.is_empty(){"(empty)"}else{cell.text.as_str()};
    if notes.is_empty(){text.into()}else{format!("[{}] {text}",notes.join(", "))}
}

fn plain_table(rows:&[Vec<crate::Cell>])->String{
    if rows.is_empty(){return "(no rows)\n".into();}
    let columns=table_columns(rows);
    if let Some(columns)=columns{
        if let Some(grid)=plain_table_grid(rows,columns){return grid;}
    }
    // A source row header can label its one value without padded columns.
    // Other shapes keep separate rows and cells rather than inventing positions.
    let row_labels=columns.is_some() && rows.iter().all(|row|
        (row.len()==1 && row[0].header) || (row.len()==2 && row[0].header && !row[1].header));
    let mut out=String::from("Table rows (not aligned):\n");
    for (r,row) in rows.iter().enumerate(){
        if r>0{out.push('\n');}
        if row_labels{
            out.push_str(&plain_table_cell(&row[0],false));
            if row.len()==1{out.push('\n');}else{
                out.push_str(":\n");
                for line in plain_table_cell(&row[1],false).split('\n'){out.push_str(&format!("  {line}\n"));}
            }
        }else{
            out.push_str(&format!("Row {}:{}\n",r+1,if row.is_empty(){" (no cells)"}else{""}));
            for cell in row{
                for (i,line) in plain_table_cell(cell,true).split('\n').enumerate(){
                    out.push_str(if i==0{"  - "}else{"    "});out.push_str(line);out.push('\n');
                }
            }
        }
    }
    out
}

pub fn plain_block(b:&crate::Block,supplemental:bool)->String{
    let mut out=format!("[{} | {}]\n",b.id,location(&b.locator));
    match &b.content{
        Content::Heading{level,text}=>out.push_str(&format!("{} {text}\n\n","#".repeat((*level).clamp(1,6) as usize))),
        Content::Code{language,text}=>{
            out.push_str(&format!("Code{}\n",language.as_ref().map(|s|format!(" ({s})")).unwrap_or_default()));
            let fence="`".repeat((text.split(|c|c!='`').map(str::len).max().unwrap_or(0)+1).max(3));
            out.push_str(&fence);out.push('\n');out.push_str(text);
            if !text.ends_with('\n'){out.push('\n');}
            out.push_str(&fence);out.push_str("\n\n");
        },
        Content::Table{rows}=>{
            if supplemental{out.push_str("Supplemental structured table (may repeat page text):\n");}
            out.push_str("Table\n");out.push_str(&plain_table(rows));out.push('\n');
        },
        Content::Caption{text}=>out.push_str(&format!("Caption\n{text}\n\n")),
        Content::Image{url,alt}=>out.push_str(&format!("Image: {alt}\n{url}\n\n")),
        Content::Quote{text}=>out.push_str(&format!("Quote\n{text}\n\n")),
        Content::Math{text}=>out.push_str(&format!("Math\n{}\n\n",display_math(text))),
        Content::ListItem{text,ordered}=>out.push_str(&format!("{} {text}\n",if *ordered{"1."}else{"-"})),
        Content::Paragraph{text}=>out.push_str(&format!("{text}\n\n")),
    }
    terminal_safe(&out)
}

fn clean_plain_block(b:&crate::Block,supplemental:bool)->String{
    let mut out=String::new();
    match &b.content{
        Content::Heading{level,text}=>out.push_str(&format!("{} {text}\n\n","#".repeat((*level).clamp(1,6) as usize))),
        Content::Paragraph{text}=>out.push_str(&format!("{text}\n\n")),
        Content::Code{language,text}=>{
            let fence="`".repeat((text.split(|c|c!='`').map(str::len).max().unwrap_or(0)+1).max(3));
            out.push_str(&format!("{fence}{}\n",language.as_deref().unwrap_or("")));
            out.push_str(text);if !text.ends_with('\n'){out.push('\n');}out.push_str(&format!("{fence}\n\n"));
        },
        Content::Table{rows}=>{
            if supplemental{out.push_str("Supplemental structured table (may repeat page text):\n");}
            out.push_str(&plain_table(rows));out.push('\n');
        },
        Content::Caption{text}=>out.push_str(&format!("[{}] {text}\n\n",caption_location(&b.locator))),
        Content::Image{alt,..}=>{if !alt.is_empty(){out.push_str(&format!("Image: {alt}\n\n"));}},
        Content::Quote{text}=>{for line in text.lines(){out.push_str(&format!("> {line}\n"));}out.push('\n');},
        Content::Math{text}=>out.push_str(&format!("{}\n\n",display_math(text))),
        Content::ListItem{text,ordered}=>out.push_str(&format!("{} {text}\n",if *ordered{"1."}else{"-"})),
    }
    out
}

fn plain_document_block(d:&Document,block:&crate::Block,details:bool,in_list:bool)->String{
    let body=match &block.content{
        Content::ListItem{text,..} if in_list=>Some(format!("{text}\n")),
        _=>plain_actions(d,block).map(|actions|format!("{actions}\n\n")),
    };
    match body{
        Some(body) if details=>format!("[{} | {}]\n{body}",block.id,location(&block.locator)),
        Some(body)=>body,
        None if details=>plain_block(block,supplemental_table(d,&block.id)),
        None=>clean_plain_block(block,supplemental_table(d,&block.id)),
    }
}

pub fn plain(d:&Document)->String{
    let mut out=format!("{}\nSource: {}\nSaved ID: {}\n\n",d.title,d.source.resolved,d.id);
    let mut previous_group:Option<String>=None;
    let mut list_open=false;
    let mut lists=ListLayout::new();
    let flows=inline_flows(d);
    let mut flow_end=0;
    for (index,block) in d.blocks.iter().enumerate(){
        if index<flow_end{continue;}
        let list=list_position(d,&block.id);
        if index==0 && list.is_none() && matches!(&block.content,Content::Heading{text,..} if text.trim()==d.title.trim()){continue;}
        let group=source_group(&block.locator);
        if group!=previous_group{if let Some(label)=&group{out.push_str(&format!("{label}\n\n"));}}
        if group.is_some(){previous_group=group;}
        let is_list=list.is_some()||matches!(block.content,Content::ListItem{..});
        if list_open&&!is_list{out.push('\n');}
        list_open=is_list;
        let body=if let Some(flow)=flows.get(&index){
            flow_end=flow.end;render_inline_flow(d,flow,InlineView::Plain)
        }else{plain_document_block(d,block,false,list.is_some())};
        lists.append(&mut out,&body,list,false);
    }
    if list_open{out.push('\n');}
    terminal_safe(&out)
}

pub fn plain_details(d:&Document)->String{
    let mut out=format!("{}\nSource: {}\nRetrieved: {}\nStatus: {}\nParser: {}\nExtraction: {}\nOriginal: {} bytes, {}, {}, SHA-256 {}\nSaved ID: {}\n\n",
        d.title,d.source.resolved,d.source.retrieved_at,d.source.status.map(|v|v.to_string()).unwrap_or_else(||"unknown".into()),d.parser,d.extraction_version,
        d.source.original.size,d.source.original.media_type,d.source.original.role,d.source.original.sha256,d.id);
    let mut lists=ListLayout::new();
    for block in &d.blocks{
        let list=list_position(d,&block.id);
        let body=plain_document_block(d,block,true,list.is_some());
        lists.append(&mut out,&body,list,false);
    }
    lists.append(&mut out,"",None,false);
    if !d.links.is_empty(){out.push_str("Links (individual source positions are not retained):\n");for link in &d.links{out.push_str(&format!("{}: {}\n",link.text,link.url));}}
    terminal_safe(&out)
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn terminal_escape_not_executed(){assert_eq!(terminal_safe("a\u{1b}[2Jb"),"a\\u{1b}[2Jb");}
    #[test]fn tabs_and_newlines_preserved(){assert_eq!(terminal_safe("a\tb\n"),"a\tb\n");}
    #[test]fn timestamps_are_exact(){assert_eq!(time(3723004),"01:02:03.004");}

    fn inline_document(contents:Vec<Content>)->Document{
        Document{
            schema_version:1,id:"inline-test".into(),title:"Support".into(),
            source:crate::Source{
                requested:"https://example.test/support".into(),resolved:"https://example.test/support".into(),
                retrieved_at:"2026-09-13T00:00:00Z".into(),status:Some(200),version:None,
                original:crate::Artifact{sha256:"test".into(),media_type:"text/html".into(),size:0,role:"http_response".into()},
            },
            parser:"test".into(),extraction_version:crate::EXTRACTION_VERSION.into(),
            blocks:contents.into_iter().enumerate().map(|(index,content)|crate::Block{
                id:format!("b{index}"),content,locator:Locator::Derived{index},
            }).collect(),links:Vec::new(),metadata:serde_json::json!({}),warnings:Vec::new(),
        }
    }
    fn inline_views(d:&Document)->[String;5]{[plain(d),plain_details(d),markdown_read(d,false),markdown_read(d,true),markdown(d)]}

    #[test]fn inline_actions_keep_selected_destinations_and_clean_prose(){
        let actions=" Find Documentation \nLearn More\tLearn More ";
        let prose="Use Radeon™ [docs] & `help` for setup.";
        let mut d=inline_document(vec![Content::Paragraph{text:actions.into()},Content::Paragraph{text:prose.into()}]);
        let span=|text:&str,label:&str,url:&str|{
            let start=text.find(label).unwrap();serde_json::json!({"start":start,"end":start+label.len(),"url":url})
        };
        let last=actions.rfind("Learn More").unwrap();
        d.metadata=serde_json::json!({"inline_links":{
            "b0":[span(actions,"Find Documentation","https://example.test/docs"),
                span(actions,"Learn More","https://example.test/developer"),
                {"start":last,"end":last+"Learn More".len(),"url":"https://example.test/security"}],
            "b1":[span(prose,"Radeon™ [docs] & `help`","https://example.test/docs_(v2)?a=1&b=2&literal=<x>")]
        }});
        d.links.push(crate::Link{text:"Unrelated footer".into(),url:"https://example.test/footer".into()});
        let expected_actions="Find Documentation: https://example.test/docs\nLearn More: https://example.test/developer\nLearn More: https://example.test/security\n\n";
        for output in [plain(&d),plain_details(&d)]{
            assert!(output.contains(expected_actions),"{output}");
            assert!(output.contains(prose));assert!(!output.contains("docs_(v2)"));
        }
        for output in [markdown_read(&d,false),markdown_read(&d,true),markdown(&d)]{
            assert!(output.contains("[Find Documentation](<https://example.test/docs>)"));
            assert!(output.contains("[Learn More](<https://example.test/developer>)"));
            assert!(output.contains("[Learn More](<https://example.test/security>)"));
            assert!(output.contains(r"Use [Radeon™ \[docs\] &amp; \`help\`](<https://example.test/docs_(v2)?a=1&amp;b=2&amp;literal=&lt;x&gt;>) for setup."),"{output}");
        }
        assert!(!plain(&d).contains("https://example.test/footer"));
        assert!(!markdown_read(&d,false).contains("https://example.test/footer"));
        assert!(plain_details(&d).contains("https://example.test/footer"));
        assert_eq!(d.blocks[0].content.text(),actions);assert_eq!(d.blocks[1].content.text(),prose);
    }

    #[test]fn list_metadata_keeps_mdn_nesting_numbers_and_continuations(){
        let code="const response = await fetch(url);\n  console.log(response.status);\n";
        let mut d=inline_document(vec![
            Content::ListItem{ordered:true,text:"a definition of the resource to fetch".into()},
            Content::ListItem{ordered:false,text:"a URL string".into()},
            Content::ListItem{ordered:false,text:"a URL object".into()},
            Content::ListItem{ordered:false,text:"a Request object".into()},
            Content::ListItem{ordered:true,text:"optionally, an options object".into()},
            Content::Paragraph{text:"Basic responses expose response headers.".into()},
            Content::Paragraph{text:"Read the body once.".into()},
            Content::Code{language:Some("js".into()),text:code.into()},
            Content::Math{text:"x + y".into()},
            Content::Table{rows:vec![vec![crate::Cell{text:"Name".into(),row_span:1,col_span:1,header:true}]]},
        ]);
        let original=d.text();
        d.metadata=serde_json::json!({"inline_links":{"b1":[{"start":2,"end":5,"url":"https://example.test/url"}]}});
        let baseline=inline_views(&d);
        let positions=[(0,Some(1i64),true),(1,None,true),(1,None,true),(1,None,true),(0,Some(2),true),
            (0,None,true),(0,None,false),(0,None,false),(0,None,false),(0,None,false)];
        for (block,(depth,ordinal,first)) in d.blocks.iter().zip(positions){
            d.metadata["list_items"][&block.id]=serde_json::json!({"depth":depth,"ordinal":ordinal,"first":first});
        }
        let views=inline_views(&d);
        assert!(views[0].contains("1. a definition of the resource to fetch\n   - a URL string\n   - a URL object\n   - a Request object\n2. optionally, an options object\n"));
        for index in [2,4]{
            assert!(views[index].contains("1. a definition of the resource to fetch\n   - a [URL](<https://example.test/url>) string\n"));
            assert!(views[index].contains("\n2. optionally, an options object\n"));
            assert!(views[index].contains("  | Name |\n  | --- |\n"));
        }
        assert!(views[1].contains("\n2. [b4 |"));assert!(views[1].contains("\n   - [b1 |"));
        assert!(views[3].contains("\n2. *b4 |"));assert!(views[3].contains("\n   - *b1 |"));
        for output in &views{
            assert!(!output.contains("1. 1."));assert!(!output.contains("- - a"));
            assert!(output.contains("  Read the body once.\n\n"));
            assert!(output.contains("  const response = await fetch(url);\n    console.log(response.status);\n"));
            assert!(output.contains("  x + y\n"));
        }
        for index in [0,2,3,4]{assert!(views[index].contains("  ```js\n"));}
        assert_eq!(d.text(),original);

        let mut numbered=inline_document(vec![Content::ListItem{ordered:true,text:"Selected start".into()},
            Content::Code{language:Some("js".into()),text:code.into()}]);
        for ordinal in [12i64,-2,i64::MIN]{
            numbered.metadata=serde_json::json!({"list_items":{
                "b0":{"depth":0,"ordinal":ordinal,"first":true},"b1":{"depth":0,"ordinal":ordinal,"first":false}
            }});
            let prefix=format!("{ordinal}. ");
            let plain_output=plain(&numbered);
            assert!(plain_output.contains(&format!("{prefix}Selected start\n")));
            assert!(plain_output.contains(&format!("{}const response = await fetch(url);"," ".repeat(prefix.len()))));
            for output in [markdown_read(&numbered,false),markdown(&numbered)]{
                if ordinal<0{assert!(output.contains(&format!("- {ordinal}.\n\n  Selected start\n")));assert!(output.contains("  ```js\n"));}
                else{assert!(output.contains(&format!("{prefix}Selected start\n")));assert!(output.contains("    ```js\n"));}
            }
        }
        let mut reversed=inline_document(vec![Content::ListItem{ordered:true,text:"First".into()},
            Content::ListItem{ordered:true,text:"Second".into()}]);
        reversed.metadata=serde_json::json!({"list_items":{
            "b0":{"depth":0,"ordinal":12,"first":true},"b1":{"depth":0,"ordinal":4,"first":true}
        }});
        for output in [markdown_read(&reversed,false),markdown(&reversed)]{
            assert!(output.contains("12. First\n\n<!-- retained list ordinal -->\n\n4. Second\n"));
        }
        for invalid in [
            serde_json::json!(null),serde_json::json!({"depth":33,"ordinal":1,"first":true}),
            serde_json::json!({"depth":u64::MAX,"ordinal":1,"first":true}),
            serde_json::json!({"depth":-1,"ordinal":1,"first":true}),
            serde_json::json!({"depth":0,"ordinal":"2","first":true}),
            serde_json::json!({"depth":0,"ordinal":u64::MAX,"first":true}),
            serde_json::json!({"depth":0,"ordinal":1,"first":"true"}),
            serde_json::json!({"depth":0,"first":true}),
        ]{
            for block in &d.blocks{d.metadata["list_items"][&block.id]=invalid.clone();}
            assert_eq!(inline_views(&d),baseline,"invalid list position: {invalid}");
        }
    }

    #[test]fn inline_flows_keep_source_spacing_links_and_list_context(){
        let math=r"\Delta G \leq 0";
        let tail="; if it is equal to zero, equilibrium holds.";
        let mut d=inline_document(vec![Content::Paragraph{text:"Gibbs free energy is negative,".into()},
            Content::Math{text:math.into()},Content::Paragraph{text:tail.into()}]);
        let original=d.text();
        let equilibrium=tail.find("equilibrium").unwrap();
        d.metadata=serde_json::json!({"inline_links":{
            "b0":[{"start":0,"end":"Gibbs free energy".len(),"url":"https://example.test/gibbs"}],
            "b2":[{"start":equilibrium,"end":equilibrium+"equilibrium".len(),"url":"https://example.test/equilibrium"}]
        }});
        let baseline=inline_views(&d);
        let flow=serde_json::json!({"blocks":["b0","b1","b2"],"separators":[" ",""]});
        d.metadata["inline_flows"]=serde_json::json!([flow]);
        let joined=format!("Gibbs free energy is negative, {math}{tail}");
        let linked=format!("[Gibbs free energy](<https://example.test/gibbs>) is negative, {math}; if it is equal to zero, [equilibrium](<https://example.test/equilibrium>) holds.");
        let views=inline_views(&d);
        assert!(views[0].contains(&joined));
        for index in [2,4]{assert!(views[index].contains(&linked));assert_eq!(views[index].matches(math).count(),1);}
        assert_eq!(views[1],baseline[1]);assert_eq!(views[3],baseline[3]);assert_eq!(d.text(),original);
        let without_flow=|document:&Document|{
            let mut copy=document.clone();copy.metadata.as_object_mut().unwrap().remove("inline_flows");inline_views(&copy)
        };

        let mut listed=d.clone();
        for (index,block) in listed.blocks.iter().enumerate(){
            listed.metadata["list_items"][&block.id]=serde_json::json!({"depth":0,"ordinal":2,"first":index==0});
        }
        assert!(plain(&listed).contains(&format!("2. {joined}\n\n")));
        assert!(markdown_read(&listed,false).contains(&format!("2. {linked}\n\n")));
        for invalid_list in [
            serde_json::json!({"depth":1,"ordinal":2,"first":false}),
            serde_json::json!({"depth":0,"ordinal":3,"first":false}),
            serde_json::json!({"depth":0,"ordinal":2,"first":true}),serde_json::json!(null),
        ]{
            let mut invalid=listed.clone();invalid.metadata["list_items"]["b1"]=invalid_list;
            assert_eq!(inline_views(&invalid),without_flow(&invalid));
        }
        listed.metadata["list_items"]["b0"]["first"]=serde_json::json!(false);
        listed.blocks.insert(0,crate::Block{id:"lead".into(),content:Content::ListItem{ordered:true,text:"Setup.".into()},locator:Locator::Derived{index:0}});
        listed.metadata["list_items"]["lead"]=serde_json::json!({"depth":0,"ordinal":2,"first":true});
        assert!(plain(&listed).contains(&format!("2. Setup.\n\n   {joined}\n\n")));
        assert!(markdown_read(&listed,false).contains(&format!("2. Setup.\n\n   {linked}\n\n")));

        for invalid_flows in [
            serde_json::json!([{"blocks":["b0"],"separators":[]}]),
            serde_json::json!([{"blocks":["b0","missing"],"separators":[" "]}]),
            serde_json::json!([{"blocks":["b0","b2"],"separators":[" "]}]),
            serde_json::json!([{"blocks":["b1","b0"],"separators":[" "]}]),
            serde_json::json!([{"blocks":["b0","b1","b1"],"separators":[" ",""]}]),
            serde_json::json!([{"blocks":["b0","b1","b2"],"separators":[" "]}]),
            serde_json::json!([{"blocks":["b0","b1","b2"],"separators":[" ","\n"]}]),
            serde_json::json!([{"blocks":["b0","b1"],"separators":[" "]},{"blocks":["b1","b2"],"separators":[""]}]),
        ]{
            let mut invalid=d.clone();invalid.metadata["inline_flows"]=invalid_flows;
            assert_eq!(inline_views(&invalid),baseline);
        }
        let mut sliced=d.clone();sliced.blocks.remove(1);
        assert_eq!(inline_views(&sliced),without_flow(&sliced));
        let mut code=d.clone();code.blocks[1].content=Content::Code{language:None,text:math.into()};
        assert_eq!(inline_views(&code),without_flow(&code));
    }

    #[test]fn invalid_inline_metadata_and_payload_spans_leave_output_unchanged(){
        let text="é action";
        let mut d=inline_document(vec![
            Content::Paragraph{text:text.into()},Content::Code{language:Some("text".into()),text:text.into()},
            Content::Math{text:text.into()},Content::Table{rows:vec![vec![crate::Cell{text:text.into(),row_span:1,col_span:1,header:true}]]},
            Content::Image{url:"https://example.test/image.png".into(),alt:text.into()},
        ]);
        let baseline=inline_views(&d);
        let url="https://example.test/action";
        for block in &d.blocks[1..]{
            d.metadata["inline_links"][&block.id]=serde_json::json!([{"start":0,"end":text.len(),"url":url}]);
        }
        assert_eq!(inline_views(&d),baseline);
        let mut invalid=vec![
            serde_json::json!(null),serde_json::json!({"start":0,"end":2,"url":url}),
            serde_json::json!([{"start":0,"end":1,"url":url}]),
            serde_json::json!([{"start":1,"end":2,"url":url}]),
            serde_json::json!([{"start":3,"end":3,"url":url}]),
            serde_json::json!([{"start":4,"end":3,"url":url}]),
            serde_json::json!([{"start":0,"end":99,"url":url}]),
            serde_json::json!([{"start":"0","end":2,"url":url}]),
            serde_json::json!([{"start":0,"end":2}]),
            serde_json::json!([{"start":0,"end":4,"url":url},{"start":3,"end":9,"url":url}]),
            serde_json::json!([{"start":3,"end":9,"url":url},{"start":0,"end":2,"url":url}]),
        ];
        for url in ["javascript:alert(1)","https:///missing","https://example.test/\npath","https://example.test:bad/","https://[not-ip]/"]{
            invalid.push(serde_json::json!([{"start":0,"end":text.len(),"url":url}]));
        }
        for metadata in invalid{
            d.metadata["inline_links"]["b0"]=metadata.clone();
            assert_eq!(inline_views(&d),baseline,"invalid metadata: {metadata}");
        }
    }
}
