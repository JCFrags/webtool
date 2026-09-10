//! One content selector, followed by structural conversion and verified source matching.
use std::collections::{HashMap,HashSet};
use anyhow::{anyhow,bail,Result};
use scraper::{ElementRef,Html,Selector};
use serde_json::json;
use url::Url;
use webtool_protocol::*;
use super::Parsed;

fn selector(s:&str)->Result<Selector>{Selector::parse(s).map_err(|e|anyhow!("invalid CSS selector: {e:?}"))}
fn normalized(s:&str)->String{s.split_whitespace().collect::<Vec<_>>().join(" ")}
fn text(e:ElementRef<'_>)->String{e.text().collect::<String>()}
fn path(e:ElementRef<'_>)->String{
    let mut result=Vec::new();
    for ancestor in std::iter::once(e).chain(e.ancestors().filter_map(ElementRef::wrap)){
        let tag=ancestor.value().name();
        let n=ancestor.parent().map(|p|p.children().filter_map(ElementRef::wrap)
            .filter(|s|s.value().name()==tag).take_while(|s|s.id()!=ancestor.id()).count()+1).unwrap_or(1);
        result.push(format!("{tag}:nth-of-type({n})"));
    }
    result.reverse();result.join(" > ")
}
fn ignored(e:ElementRef<'_>)->bool{
    e.ancestors().filter_map(ElementRef::wrap).any(|a|matches!(a.value().name(),"script"|"style"|"noscript"|"template"))
}
fn atomic_parent(e:ElementRef<'_>)->bool{
    e.ancestors().filter_map(ElementRef::wrap).any(|a|matches!(a.value().name(),"pre"|"table"|"blockquote"|"li"|"math"))
}
fn absolute(base:Option<&Url>,value:&str)->Option<String>{
    let u=Url::parse(value).ok().or_else(||base.and_then(|b|b.join(value).ok()))?;
    if !matches!(u.scheme(),"http"|"https"){return None;}Some(u.to_string())
}
pub fn links(source:&str,url:&str)->Vec<Link>{
    let doc=Html::parse_document(source);let base=Url::parse(url).ok();let mut seen=HashSet::new();
    doc.select(&Selector::parse("a[href]").expect("constant selector")).filter_map(|e|{
        let url=absolute(base.as_ref(),e.value().attr("href")?)?;
        if !seen.insert(url.clone()){return None;}Some(Link{url,text:normalized(&text(e))})
    }).collect()
}

pub fn parse(bytes:&[u8],url:&str,explicit:Option<&str>)->Result<Parsed>{
    let source=std::str::from_utf8(bytes).map_err(|_|anyhow!("HTML is not UTF-8. Encoding conversion is not implemented in this build."))?;
    let original=Html::parse_document(source);
    let title=original.select(&selector("title")?).next().map(text).unwrap_or_else(||url.into());
    let mut p=Parsed::new(&normalized(&title),"rs-trafilatura/0.2.2+source-blocks/1");
    p.links=links(source,url);
    let selected=if let Some(css)=explicit{
        p.parser="explicit-css+source-blocks/1".into();
        let found=original.select(&selector(css)?).map(|n|n.html()).collect::<Vec<_>>();
        if found.is_empty(){bail!("CSS selector matched no elements");}found.join("\n")
    }else{
        #[cfg(feature="web-extraction")]
        {
            let options=rs_trafilatura::Options{
                include_comments:true,include_tables:true,include_images:true,include_links:true,
                include_formatting:true,output_markdown:false,favor_recall:true,deduplicate:false,
                min_extracted_size:1,min_extracted_len:1,min_output_size:1,
                use_fallback_extraction:false,url:Some(url.into()),..Default::default()
            };
            let r=rs_trafilatura::extract_with_options(source,&options).map_err(|e|anyhow!("HTML extraction failed: {e}"))?;
            if let Some(t)=r.metadata.title{p.title=t;}
            p.metadata=json!({"extractor_estimated_quality":r.extraction_quality,"quality_estimate_is_not_validation":true});
            if r.extraction_quality<0.8{p.warnings.push(Warning::new("low_extractor_estimate","The extractor estimates low confidence. Inspect the retained original."));}
            if let Some(comments)=r.comments_text {
                if !comments.trim().is_empty(){p.metadata["comments_text"]=json!(comments);}
            }
            r.content_html.ok_or_else(||anyhow!("extractor returned no structured HTML; use an explicit CSS selector to read the original structure"))?
        }
        #[cfg(not(feature="web-extraction"))]
        {bail!("HTML content selection requires the web-extraction feature or an explicit selector");}
    };
    let elements=selector("h1,h2,h3,h4,h5,h6,p,pre,table,li,blockquote,img,math")?;
    let mut origins:HashMap<(String,String),Vec<(String,String)>>=HashMap::new();
    for e in original.select(&elements).filter(|e|!ignored(*e)){
        let raw=if e.value().name()=="img"{e.value().attr("src").unwrap_or("").into()}else{text(e)};
        origins.entry((e.value().name().into(),normalized(&raw))).or_default().push((path(e),raw));
    }
    let cleaned=Html::parse_fragment(&selected);let base=Url::parse(url).ok();let mut unmapped=0;
    for e in cleaned.select(&elements).filter(|e|!ignored(*e)&&!atomic_parent(*e)){
        let tag=e.value().name();
        let raw=if tag=="img"{e.value().attr("src").unwrap_or("").into()}else{text(e)};
        let match_key=(tag.to_owned(),normalized(&raw));
        let origin=origins.get(&match_key).and_then(|v|if v.len()==1{v.first()}else{None});
        let locator=if let Some((path,_))=origin{Locator::Html{selector:path.clone()}}else{unmapped+=1;Locator::Derived{index:p.blocks.len()+1}};
        let exact=origin.map(|(_,raw)|raw.clone()).unwrap_or_else(||raw.clone());
        let value=normalized(&exact);
        if value.is_empty()&&tag!="img"{continue;}
        let content=match tag{
            "h1"|"h2"|"h3"|"h4"|"h5"|"h6"=>Content::Heading{level:tag.as_bytes()[1]-b'0',text:value},
            "pre"=>{
                let language=e.select(&selector("code")?).next().and_then(|c|c.value().attr("class"))
                    .and_then(|c|c.split_whitespace().find_map(|s|s.strip_prefix("language-"))).map(str::to_string);
                Content::Code{language,text:exact}
            },
            "table"=>{
                let rows=e.select(&selector("tr")?).map(|row|row.select(&Selector::parse("th,td").expect("constant selector"))
                    .map(|cell|Cell{text:normalized(&text(cell)),row_span:cell.value().attr("rowspan").and_then(|s|s.parse().ok()).unwrap_or(1),
                        col_span:cell.value().attr("colspan").and_then(|s|s.parse().ok()).unwrap_or(1),header:cell.value().name()=="th"}).collect()).collect();
                Content::Table{rows}
            },
            "li"=>Content::ListItem{text:value,ordered:e.parent().and_then(ElementRef::wrap).is_some_and(|n|n.value().name()=="ol")},
            "blockquote"=>Content::Quote{text:value},
            "img"=>{let Some(image_url)=absolute(base.as_ref(),e.value().attr("src").unwrap_or(""))else{continue;};
                Content::Image{url:image_url,alt:e.value().attr("alt").unwrap_or("").into()}},
            "math"=>Content::Math{text:e.html()},
            _=>Content::Paragraph{text:value},
        };
        p.push(content,locator);
    }
    if p.blocks.is_empty(){bail!("no readable blocks found; the page may require JavaScript or an explicit selector");}
    if unmapped>0{p.warnings.push(Warning::new("approximate_source_mapping",format!("{unmapped} blocks could not be mapped uniquely to an original HTML element.")));}
    if let Some(comments)=p.metadata.get("comments_text").and_then(|v|v.as_str()).map(str::to_owned){
        p.push(Content::Paragraph{text:comments},Locator::Derived{index:p.blocks.len()+1});
        p.warnings.push(Warning::new("comment_locations_derived","Extracted comments have no exact original element mapping."));
    }
    let raw_tables=original.select(&selector("main table,article table")?).count();
    let kept_tables=p.blocks.iter().filter(|b|matches!(b.content,Content::Table{..})).count();
    if raw_tables>kept_tables{p.warnings.push(Warning::new("table_coverage_gap",format!("The source main content contains {raw_tables} tables, but extraction retained {kept_tables}.")));}
    if source.contains("id=\"root\"")||source.contains("id=\"__next\"") {
        p.warnings.push(Warning::new("dynamic_content_possible","This page may add content with JavaScript. HTTP extraction does not establish rendered completeness."));
    }
    Ok(p)
}
pub fn select_original(source:&str,css:&str)->Result<Vec<serde_json::Value>>{
    let doc=Html::parse_document(source);
    Ok(doc.select(&selector(css)?).map(|e|json!({"selector":path(e),"text":text(e),"html":e.html()})).collect())
}
#[cfg(test)]mod tests{
    use super::*;
    const SAMPLE:&str="<html><head><title>A</title></head><body><main><h1>Title</h1><p>Exact evidence.</p><pre><code class=\"language-rust\">  let n = 0;\n</code></pre><table><tr><th>Name</th><th>N</th></tr><tr><td>x</td><td>0</td></tr></table></main></body></html>";
    #[test]fn explicit_selector_preserves_code(){let p=parse(SAMPLE.as_bytes(),"https://example.com",Some("main")).unwrap();let c=p.blocks.iter().find(|b|matches!(b.content,Content::Code{..})).unwrap();assert_eq!(c.content.text(),"  let n = 0;\n");assert!(matches!(c.locator,Locator::Html{..}));}
    #[test]fn table_is_not_duplicated_as_paragraphs(){let p=parse(SAMPLE.as_bytes(),"https://example.com",Some("main")).unwrap();assert_eq!(p.blocks.iter().filter(|b|matches!(b.content,Content::Table{..})).count(),1);}
    #[test]fn bad_css_is_an_error(){assert!(select_original(SAMPLE,"[").is_err());}
    #[test]fn relative_links_are_resolved(){assert_eq!(links("<a href='/one'>one</a><a href='javascript:x'>bad</a>","https://example.com").len(),1);}
    #[test]fn absent_selector_is_an_error(){assert!(parse(SAMPLE.as_bytes(),"x",Some(".not-here")).is_err());}
}
