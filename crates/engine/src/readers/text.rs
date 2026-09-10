use anyhow::{bail, Context, Result};
use serde_json::{json,Value};
use webtool_protocol::*;
use super::Parsed;

pub fn plain(text:&str,name:&str)->Parsed {
    let mut p=Parsed::new(name,"native-text/1");
    let lang=match super::extension(name).as_str(){
        "rs"=>Some("rust"),"py"=>Some("python"),"js"=>Some("javascript"),"ts"=>Some("typescript"),
        "sh"=>Some("bash"),"toml"=>Some("toml"),"yaml"|"yml"=>Some("yaml"),"tex"=>Some("latex"),_=>None,
    }.map(str::to_string);
    p.push(Content::Code{language:lang,text:text.into()},Locator::Lines{start:1,end:text.lines().count().max(1)});
    p
}

pub fn markdown(text:&str,name:&str)->Parsed {
    let mut p=Parsed::new(name,"native-markdown-blocks/1");
    let lines:Vec<&str>=text.split_inclusive('\n').collect();
    let mut i=0;
    while i<lines.len() {
        let line=lines[i].trim_end_matches(['\r','\n']);
        if line.trim().is_empty(){i+=1;continue;}
        let start=i;
        let trim=line.trim_start();
        if trim.starts_with("```")||trim.starts_with("~~~") {
            let marker=trim.as_bytes()[0] as char;
            let count=trim.chars().take_while(|c|*c==marker).count();
            let language=trim[count..].trim();
            let language=if language.is_empty(){None}else{Some(language.into())};
            i+=1; let mut body=String::new(); let mut closed=false;
            while i<lines.len() {
                let candidate=lines[i].trim();
                let n=candidate.chars().take_while(|c|*c==marker).count();
                if n>=count && candidate[n..].trim().is_empty(){closed=true;i+=1;break;}
                body.push_str(lines[i]); i+=1;
            }
            p.push(Content::Code{language,text:body},Locator::Lines{start:start+1,end:i});
            if !closed { p.warnings.push(Warning::new("unclosed_fence","The source contains an unclosed code fence.")); }
            continue;
        }
        let level=trim.chars().take_while(|c|*c=='#').count();
        if (1..=6).contains(&level)&&trim.as_bytes().get(level)==Some(&b' ') {
            let heading=trim[level+1..].trim_end().to_string();
            if p.title==name&&level==1 {p.title=heading.clone();}
            p.push(Content::Heading{level:level as u8,text:heading},Locator::Lines{start:i+1,end:i+1});i+=1;continue;
        }
        let mut body=String::new();
        while i<lines.len() {
            let current=lines[i].trim_start();
            if current.trim().is_empty(){break;}
            if i>start&&(current.starts_with('#')||current.starts_with("```")||current.starts_with("~~~")){break;}
            body.push_str(lines[i]);i+=1;
        }
        p.push(Content::Paragraph{text:body.trim_end_matches('\n').into()},Locator::Lines{start:start+1,end:i});
    }
    p
}

pub fn csv(text:&str,name:&str,delimiter:u8)->Result<Parsed> {
    let mut reader=::csv::ReaderBuilder::new().has_headers(false).flexible(true).delimiter(delimiter).from_reader(text.as_bytes());
    let mut rows=Vec::new();let mut widths=std::collections::HashSet::new();
    for result in reader.records() {
        let record=result.context("parse delimited record")?;
        widths.insert(record.len());
        rows.push(record.iter().map(|text|Cell{text:text.into(),row_span:1,col_span:1,header:false}).collect());
    }
    let mut p=Parsed::new(name,"csv/1");
    p.push(Content::Table{rows},Locator::Lines{start:1,end:text.lines().count().max(1)});
    p.metadata=json!({"delimiter":(delimiter as char).to_string(),"header_inferred":false});
    if widths.len()>1 {p.warnings.push(Warning::new("ragged_table","Rows have different field counts. No values were padded or discarded."));}
    Ok(p)
}
pub fn json(text:&str,name:&str)->Result<Parsed> {
    let _:Value=serde_json::from_str(text).context("parse JSON")?;
    let mut p=Parsed::new(name,"serde-json/1");
    p.push(Content::Code{language:Some("json".into()),text:text.into()},Locator::JsonPointer{pointer:"".into()});
    Ok(p)
}
pub fn jsonl(text:&str,name:&str)->Result<Parsed> {
    let mut p=Parsed::new(name,"serde-json-lines/1");
    for (i,line) in text.lines().enumerate(){
        if line.trim().is_empty(){continue;}
        let _:Value=serde_json::from_str(line).with_context(||format!("invalid JSON on line {}",i+1))?;
        p.push(Content::Code{language:Some("json".into()),text:line.into()},Locator::Lines{start:i+1,end:i+1});
    } Ok(p)
}
fn cell_source(v:&Value)->String {
    if let Some(s)=v.as_str(){return s.into();}
    v.as_array().map(|a|a.iter().filter_map(Value::as_str).collect::<String>()).unwrap_or_default()
}
pub fn notebook(text:&str,name:&str)->Result<Parsed> {
    let notebook:Value=serde_json::from_str(text)?;
    if notebook["nbformat"].as_u64()!=Some(4){bail!("only notebook format 4 is supported");}
    let cells=notebook["cells"].as_array().context("notebook lacks a cells array")?;
    let language=notebook.pointer("/metadata/language_info/name").and_then(Value::as_str).map(str::to_string);
    let mut p=Parsed::new(name,"native-notebook-v4/1");
    let mut omitted=0;
    for (i,c) in cells.iter().enumerate(){
        let source=cell_source(&c["source"]);
        let content=match c["cell_type"].as_str(){
            Some("code")=>Content::Code{language:language.clone(),text:source},
            _=>Content::Paragraph{text:source},
        };
        p.push(content,Locator::JsonPointer{pointer:format!("/cells/{i}/source")});
        if let Some(outputs)=c["outputs"].as_array(){
            for (j,output) in outputs.iter().enumerate(){
                let (value,path)=if output.get("text").is_some(){(&output["text"],"text")}
                    else{(&output["data"]["text/plain"],"data/text~1plain")};
                if !value.is_null(){
                    p.push(Content::Code{language:None,text:cell_source(value)},Locator::JsonPointer{pointer:format!("/cells/{i}/outputs/{j}/{path}")});
                }else{omitted+=1;}
            }
        }
    }
    if omitted>0{p.warnings.push(Warning::new("rich_outputs_retained",format!("{omitted} non-text outputs remain in the original notebook.")));}
    p.metadata=json!({"executed":false,"nbformat":4});Ok(p)
}
pub fn xml(text:&str,name:&str)->Result<Parsed> {
    let tree=roxmltree::Document::parse(text).context("parse XML")?;
    if tree.root_element().has_tag_name("rss")||tree.root_element().has_tag_name("feed") {
        return super::parse(text.as_bytes(),name,"application/atom+xml",None);
    }
    let mut p=Parsed::new(name,"native-xml/1");
    let scientific=tree.root_element().has_tag_name("article");
    if !scientific { return Ok(plain(text,name)); }
    for node in tree.descendants().filter(|n|n.is_element() && matches!(n.tag_name().name(),"article-title"|"title"|"p"|"disp-formula")) {
        if node.ancestors().skip(1).any(|a|a.has_tag_name("p")||a.has_tag_name("disp-formula")){continue;}
        let value=node.descendants().filter(|n|n.is_text()).filter_map(|n|n.text()).collect::<String>();
        if value.trim().is_empty(){continue;}
        let content=match node.tag_name().name(){
            "article-title"=>{p.title=value.clone();Content::Heading{level:1,text:value}},
            "title"=>Content::Heading{level:2,text:value},
            "disp-formula"=>Content::Math{text:value},_=>Content::Paragraph{text:value},
        };
        let range=node.range();
        let start=text[..range.start].bytes().filter(|b|*b==b'\n').count()+1;
        let end=text[..range.end].bytes().filter(|b|*b==b'\n').count()+1;
        p.push(content,Locator::Lines{start,end});
    }
    p.warnings.push(Warning::new("xml_partial_structure","JATS prose is extracted. Complex tables and formula markup remain in the original XML."));
    Ok(p)
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn code_whitespace_survives(){let p=markdown("# T\n\n```rust\n  let x = 1;\n```\n","t");assert_eq!(p.blocks[1].content.text(),"  let x = 1;\n");}
    #[test] fn csv_keeps_zero_and_empty(){let p=csv("a,b,c\n0,,=SUM(A1:A2)\n","x",b',').unwrap();if let Content::Table{rows}=&p.blocks[0].content {assert_eq!(rows[1][0].text,"0");assert_eq!(rows[1][1].text,"");assert_eq!(rows[1][2].text,"=SUM(A1:A2)");}else{panic!("not a table");}}
    #[test] fn csv_multiline(){let p=csv("\"first\nsecond\",3\n","x",b',').unwrap();assert!(p.blocks[0].content.text().contains("first\nsecond"));}
    #[test] fn malformed_jsonl_reports_line(){let e=jsonl("{}\nnot json\n","x").unwrap_err();assert!(e.to_string().contains("line 2"));}
    #[test] fn notebook_never_executes(){let p=notebook(r#"{"nbformat":4,"cells":[{"cell_type":"code","source":["raise RuntimeError()"],"outputs":[]}] }"#,"n").unwrap();assert_eq!(p.metadata["executed"],false);}
    #[test] fn source_text_not_reformatted(){let p=plain("a\r\n  b\t", "x.txt");assert_eq!(p.blocks[0].content.text(),"a\r\n  b\t");}
}
