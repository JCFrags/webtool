//! One content selector, followed by structural conversion and verified source matching.
use std::{collections::{HashMap,HashSet},error::Error,fmt};
use anyhow::{anyhow,bail,Result};
use scraper::{ElementRef,Html,Selector};
use serde_json::json;
use url::Url;
use webtool_protocol::*;
use super::Parsed;

pub const PARSER:&str="rs-trafilatura/0.2.2+main-content/6";

/// A stable signal that ordinary Auto reads may use for one rendered retry.
/// Parse, encoding, network, and explicit-selector failures are not this error.
#[derive(Debug)]
pub struct MissingContent {
    pub code: &'static str,
    message: &'static str,
}
impl MissingContent {
    fn new(code: &'static str, message: &'static str) -> Self { Self { code, message } }
}
impl fmt::Display for MissingContent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(self.message) }
}
impl Error for MissingContent {}

fn selector(s:&str)->Result<Selector>{Selector::parse(s).map_err(|e|anyhow!("invalid CSS selector: {e:?}"))}
fn normalized(s:&str)->String{s.split_whitespace().collect::<Vec<_>>().join(" ")}
fn text(e:ElementRef<'_>)->String{e.text().collect::<String>()}
fn cell_text(e:ElementRef<'_>)->String{
    fn boundary(out:&mut String){
        while out.ends_with(' '){out.pop();}
        if !out.is_empty()&&!out.ends_with('\n'){out.push('\n');}
    }
    fn walk(e:ElementRef<'_>,out:&mut String){
        for child in e.children(){
            if let Some(value)=child.value().as_text(){
                // HTML whitespace collapses within inline text. Block and br
                // boundaries below are explicit, not inferred word breaks.
                for c in value.chars(){
                    if c.is_whitespace(){
                        if !out.is_empty()&&!out.ends_with([' ','\n']){out.push(' ');}
                    }else{out.push(c);}
                }
            }else if let Some(child)=ElementRef::wrap(child){
                let tag=child.value().name();
                if matches!(tag,"script"|"style"|"noscript"|"template"){continue;}
                if tag=="br"{
                    while out.ends_with(' '){out.pop();}
                    out.push('\n');
                    continue;
                }
                let block=matches!(tag,"div"|"p"|"li"|"ul"|"ol"|"dl"|"dt"|"dd"|"pre"|"blockquote"|"table"|"tr"|"th"|"td");
                if block{boundary(out);}
                walk(child,out);
                if block{boundary(out);}
            }
        }
    }
    let mut out=String::new();
    walk(e,&mut out);
    out.trim().to_owned()
}
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
const BLOCKS: &str = "h1,h2,h3,h4,h5,h6,p,pre,table,li,dt,dd,blockquote,img,figcaption,math";
fn is_block(e: ElementRef<'_>) -> bool {
    BLOCKS.split(',').any(|tag| tag == e.value().name())
}
fn is_inline(e: ElementRef<'_>) -> bool {
    matches!(e.value().name(), "a"|"abbr"|"b"|"bdi"|"bdo"|"cite"|"data"|"del"|"em"|"i"|"ins"|"kbd"|"mark"|"q"|"ruby"|"s"|"samp"|"small"|"span"|"strong"|"sub"|"sup"|"time"|"u"|"var"|"wbr")
}
fn has_role(e:ElementRef<'_>,role:&str)->bool{
    e.value().attr("role").is_some_and(|roles|roles.split_ascii_whitespace().any(|value|value.eq_ignore_ascii_case(role)))
}
fn chrome_landmark(e:ElementRef<'_>)->bool{
    if e.value().name()=="nav"||has_role(e,"navigation")||has_role(e,"contentinfo"){return true;}
    // Some pages use named containers instead of the footer element. Do not
    // treat a prose heading or a link named "footer" as a page landmark.
    let named_footer=matches!(e.value().name(),"div"|"aside"|"section")
        &&[e.value().attr("id"),e.value().attr("class")].into_iter().flatten().any(|value|
            value.to_ascii_lowercase().split(|c:char|!c.is_ascii_alphanumeric())
                .any(|token|matches!(token,"footer"|"pagefooter"|"sitefooter"|"printfooter")));
    // Article-scoped footers and endnotes can contain citations. Leave them to
    // content selection rather than deleting them with page-level navigation.
    (e.value().name()=="footer"||named_footer)&&!has_role(e,"doc-endnotes")
        &&!e.ancestors().filter_map(ElementRef::wrap).any(|a|a.value().name()=="article"||has_role(a,"article")||has_role(a,"doc-endnotes"))
}
#[cfg(feature="web-extraction")]
fn selection_source(original:&Html)->Result<String>{
    let mut selection=original.clone();
    let excluded=selection.select(&selector("nav,footer,[role],div[id],div[class],aside[id],aside[class],section[id],section[class]")?)
        .filter(|e|chrome_landmark(*e)).map(|e|e.id()).collect::<Vec<_>>();
    for id in excluded{
        if let Some(mut node)=selection.tree.get_mut(id){node.detach();}
    }
    // Remove landmarks before the extractor discards their identifying wrappers.
    // The original tree, source bytes, selectors, and all-page links stay intact.
    Ok(selection.html())
}
fn chrome_hint(e: ElementRef<'_>) -> bool {
    std::iter::once(e).chain(e.ancestors().filter_map(ElementRef::wrap)).any(|a| {
        if matches!(a.value().name(), "html"|"body") { return false; }
        if chrome_landmark(a) { return true; }
        let tokens:HashSet<String>=[a.value().attr("id"), a.value().attr("class")].into_iter().flatten()
            .flat_map(|value| value.to_ascii_lowercase().split(|c:char| !c.is_ascii_alphanumeric()).map(str::to_owned).collect::<Vec<_>>()).collect();
        // A section about cookies, social systems, or related work is content.
        // Require widget structure as well as a label before excluding it.
        let linked=a.select(&Selector::parse("a[href]").expect("constant selector"))
            .map(text).collect::<String>().chars().count();
        let length=text(a).chars().count().max(1);
        let link_widget=linked.saturating_mul(2)>length;
        let cookie_widget=(tokens.contains("cookie")||tokens.contains("consent"))
            && (a.value().attr("role")==Some("dialog")||tokens.contains("banner")||tokens.contains("modal"));
        let related_widget=tokens.contains("related")
            && (tokens.contains("posts")||tokens.contains("articles")||tokens.contains("stories"));
        cookie_widget || (link_widget && (tokens.contains("navbar")||tokens.contains("navigation")
            ||tokens.contains("pagination")||tokens.contains("recommendations")||related_widget))
    })
}
fn link_only_chrome(e: ElementRef<'_>) -> bool {
    if !matches!(e.value().name(), "p"|"li") { return false; }
    let value=normalized(&text(e)).to_ascii_lowercase();
    let labels=["login","log in","sign in","sign up","menu"];
    labels.contains(&value.as_str()) && e.select(&Selector::parse("a").expect("constant selector")).next().is_some()
}
type Origins<'a> = HashMap<(String, String), Vec<ElementRef<'a>>>;

// Walk only the selected tree. Never substitute an original container subtree:
// doing so could restore navigation or other descendants removed by selection.
enum Part<'a> { Text(String), Block(ElementRef<'a>) }
fn parts<'a>(e: ElementRef<'a>, out: &mut Vec<Part<'a>>) {
    for child in e.children() {
        if let Some(t) = child.value().as_text() {
            out.push(Part::Text(t.to_string()));
        } else if let Some(child) = ElementRef::wrap(child) {
            if matches!(child.value().name(), "script"|"style"|"noscript"|"template") { continue; }
            if is_block(child) { out.push(Part::Block(child)); }
            else if child.value().name() == "br" { out.push(Part::Text("\n".into())); }
            else { parts(child, out); }
        }
    }
}
fn prose(e: ElementRef<'_>, value: String) -> Content {
    match e.value().name() {
        "h1"|"h2"|"h3"|"h4"|"h5"|"h6" => Content::Heading { level:e.value().name().as_bytes()[1]-b'0', text:value },
        "li" => Content::ListItem { text:value, ordered:e.parent().and_then(ElementRef::wrap).is_some_and(|n|n.value().name()=="ol") },
        "blockquote" => Content::Quote { text:value },
        "figcaption" => Content::Caption { text:value },
        _ => Content::Paragraph { text:value },
    }
}
fn flush_run(e: ElementRef<'_>, run: &mut String, p: &mut Parsed, unmapped: &mut usize) {
    let value = normalized(run);
    run.clear();
    if !value.is_empty() {
        // A fragment of a container is not an exact whole-element location.
        *unmapped += 1;
        p.push(prose(e, value), Locator::Derived { index:p.blocks.len()+1 });
    }
}
fn emit(e: ElementRef<'_>, origins: &Origins<'_>, base: Option<&Url>, filter_chrome: bool, p: &mut Parsed, unmapped: &mut usize) -> Result<()> {
    if ignored(e) || matches!(e.value().name(), "script"|"style"|"noscript"|"template") { return Ok(()); }
    if !is_block(e) {
        // Keep inline descendants in one prose run. Structural transparent
        // containers remain boundaries, so repeated cards keep source order.
        let mut run = String::new();
        for child in e.children() {
            if let Some(t) = child.value().as_text() { run.push_str(t); }
            else if let Some(child) = ElementRef::wrap(child) {
                if matches!(child.value().name(), "script"|"style"|"noscript"|"template") { continue; }
                if is_inline(child) {
                    let mut inline_parts=Vec::new();
                    parts(child,&mut inline_parts);
                    for part in inline_parts {
                        match part {
                            Part::Text(value)=>run.push_str(&value),
                            Part::Block(block)=>{
                                flush_run(e,&mut run,p,unmapped);
                                emit(block,origins,base,filter_chrome,p,unmapped)?;
                            },
                        }
                    }
                } else {
                    flush_run(e, &mut run, p, unmapped);
                    emit(child, origins, base, filter_chrome, p, unmapped)?;
                }
            }
        }
        flush_run(e, &mut run, p, unmapped);
        return Ok(());
    }
    let tag = e.value().name();
    if !matches!(tag, "pre"|"table"|"img"|"math") {
        let mut children = Vec::new();
        parts(e, &mut children);
        if children.iter().any(|part| matches!(part, Part::Block(_))) {
            let mut run = String::new();
            for child in children {
                match child {
                    Part::Text(t) => run.push_str(&t),
                    Part::Block(child) => {
                        flush_run(e, &mut run, p, unmapped);
                        emit(child, origins, base, filter_chrome, p, unmapped)?;
                    }
                }
            }
            flush_run(e, &mut run, p, unmapped);
            return Ok(());
        }
    }
    let raw = if tag=="img" { e.value().attr("src").unwrap_or("").into() } else { text(e) };
    if normalized(&raw).is_empty() && tag!="img" { return Ok(()); }
    let origin = origins.get(&(tag.to_owned(), normalized(&raw)))
        .and_then(|v| if v.len()==1 { v.first().copied() } else { None });
    if filter_chrome && origin.is_some_and(|original| chrome_hint(original) || link_only_chrome(original)) { return Ok(()); }
    let locator = if let Some(original) = origin { Locator::Html { selector:path(original) } }
        else { *unmapped+=1; Locator::Derived { index:p.blocks.len()+1 } };
    let content = match tag {
        "pre" => {
            let code = origin.unwrap_or(e);
            let language = std::iter::once(code).chain(code.select(&selector("code")?))
                .filter_map(|n| n.value().attr("class"))
                .find_map(|classes| classes.split_whitespace().find_map(|c| c.strip_prefix("language-").or_else(||c.strip_prefix("lang-"))))
                .map(str::to_owned);
            Content::Code { language, text:origin.map(text).unwrap_or(raw) }
        },
        "table" => {
            // Matching is unique and covers the entire selected table's text.
            // Read cells and attributes from that verified element, not cleaned HTML.
            let table = origin.unwrap_or(e);
            let rows = table.select(&selector("tr")?)
                .filter(|row| row.ancestors().filter_map(ElementRef::wrap).find(|a|a.value().name()=="table").is_some_and(|a|a.id()==table.id()))
                .map(|row| row.select(&Selector::parse("th,td").expect("constant selector"))
                    .filter(|cell| cell.ancestors().filter_map(ElementRef::wrap).find(|a|a.value().name()=="tr").is_some_and(|a|a.id()==row.id()))
                    .map(|cell| Cell { text:cell_text(cell),
                        row_span:cell.value().attr("rowspan").and_then(|s|s.parse().ok()).unwrap_or(1),
                        col_span:cell.value().attr("colspan").and_then(|s|s.parse().ok()).unwrap_or(1),
                        header:cell.value().name()=="th" }).collect()).collect();
            Content::Table { rows }
        },
        "img" => {
            let Some(url) = absolute(base, e.value().attr("src").unwrap_or("")) else { return Ok(()); };
            Content::Image { url, alt:e.value().attr("alt").unwrap_or("").into() }
        },
        "math" => {
            let math=origin.unwrap_or(e);
            let tex=math.select(&selector("annotation")?).find_map(|annotation| {
                annotation.value().attr("encoding")
                    .filter(|encoding| encoding.to_ascii_lowercase().contains("tex"))
                    .map(|_| text(annotation).trim().to_owned())
            }).filter(|value| !value.is_empty());
            Content::Math { text:tex.unwrap_or_else(||"[MathML source required; see retained original]".into()) }
        },
        _ => prose(e, normalized(&raw)),
    };
    p.push(content, locator);
    Ok(())
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

/// Only browser captures use their DOM base; retained bytes remain unchanged.
pub fn rendered_base(bytes:&[u8],url:&str)->String{
    let Ok(source)=std::str::from_utf8(bytes) else {return url.into()};
    let doc=Html::parse_document(source);let base=Url::parse(url).ok();
    doc.select(&Selector::parse("base[href]").expect("constant selector")).next()
        .and_then(|e|absolute(base.as_ref(),e.value().attr("href")?)).unwrap_or_else(||url.into())
}

pub fn parse(bytes:&[u8],url:&str,explicit:Option<&str>)->Result<Parsed>{
    let source=std::str::from_utf8(bytes).map_err(|_|anyhow!("HTML is not UTF-8. Encoding conversion is not implemented in this build."))?;
    let original=Html::parse_document(source);
    let title=original.select(&selector("title")?).next().map(text).unwrap_or_else(||url.into());
    let mut p=Parsed::new(&normalized(&title),PARSER);
    p.links=links(source,url);
    let mut readable_text:Option<String>=None;
    let selected=if let Some(css)=explicit{
        p.parser="explicit-css+source-blocks/5".into();
        let found=original.select(&selector(css)?).map(|n|n.html()).collect::<Vec<_>>();
        if found.is_empty(){bail!("CSS selector matched no elements");}found.join("\n")
    }else{
        #[cfg(feature="web-extraction")]
        {
            // Standard thresholds guide this extractor's selection but do not
            // reject a short result. Keep internal fallback disabled, so short
            // substantive pages survive without a recall ladder. The extractor's
            // Forum profile overrides include_comments when replies are the main
            // discussion; article comments remain separate and are not appended.
            let options=rs_trafilatura::Options{
                include_comments:false,include_tables:true,include_images:true,include_links:true,
                include_formatting:true,output_markdown:false,favor_precision:false,favor_recall:false,
                deduplicate:false,use_fallback_extraction:false,url:Some(url.into()),
                ..Default::default()
            };
            let selection=selection_source(&original)?;
            let r=rs_trafilatura::extract_with_options(&selection,&options).map_err(|e|anyhow!("HTML extraction failed: {e}"))?;
            readable_text=Some(r.content_text);
            if let Some(t)=r.metadata.title{p.title=t;}
            p.metadata=json!({"extractor_estimated_quality":r.extraction_quality,"quality_estimate_is_not_validation":true});
            if r.extraction_quality<0.8{p.warnings.push(Warning::new("low_extractor_estimate","The extractor estimates low confidence. Inspect the retained original."));}
            for warning in r.warnings{p.warnings.push(Warning::new("html_extraction_warning",warning));}
            r.content_html.ok_or_else(||MissingContent::new(
                "html_no_structured_content",
                "extractor returned no structured HTML; the page may require JavaScript or an explicit selector",
            ))?
        }
        #[cfg(not(feature="web-extraction"))]
        {bail!("HTML content selection requires the web-extraction feature or an explicit selector");}
    };
    let elements=selector(BLOCKS)?;
    let mut origins:Origins<'_>=HashMap::new();
    for e in original.select(&elements).filter(|e|!ignored(*e)) {
        let raw=if e.value().name()=="img" { e.value().attr("src").unwrap_or("").into() } else { text(e) };
        origins.entry((e.value().name().into(),normalized(&raw))).or_default().push(e);
    }
    let cleaned=Html::parse_fragment(&selected);
    let base=Url::parse(url).ok();
    let mut unmapped=0;
    emit(cleaned.root_element(), &origins, base.as_ref(), explicit.is_none(), &mut p, &mut unmapped)?;
    if let Some(readable)=readable_text {
        // The extractor's HTML serializer can join adjacent inline nodes even
        // when its text view keeps their spacing. Recover whitespace only from
        // a unique complete line with identical non-whitespace characters.
        // Never replace a subtree, add a paragraph, or alter code/table/math.
        let key=|value:&str|value.chars().filter(|c|!c.is_whitespace()).collect::<String>();
        let mut lines:HashMap<String,HashSet<String>>=HashMap::new();
        for line in readable.lines().map(normalized).filter(|line|!line.is_empty()) {
            lines.entry(key(&line)).or_default().insert(line);
        }
        // The text view can omit a short final card. Also retain a whitespace
        // boundary between adjacent original quotation/attribution inline nodes.
        // Only a matching selected run can use it, never unselected descendants.
        for element in original.select(&selector("div,p,li,blockquote,figcaption")?) {
            if ignored(element){continue;}
            let mut run=String::new();let mut spaced=false;
            let mut record=|run:&mut String,spaced:&mut bool|{
                if *spaced{let line=normalized(run);lines.entry(key(&line)).or_default().insert(line);}
                run.clear();*spaced=false;
            };
            for child in element.children(){
                if let Some(value)=child.value().as_text(){run.push_str(value);}
                else if let Some(child)=ElementRef::wrap(child){
                    if is_inline(child){
                        let value=text(child);
                        if run.ends_with(['”','»','"'])&&value.chars().next().is_some_and(char::is_alphanumeric){run.push(' ');spaced=true;}
                        run.push_str(&value);
                    }else{record(&mut run,&mut spaced);}
                }
            }
            record(&mut run,&mut spaced);
        }
        for (index,block) in p.blocks.iter_mut().enumerate() {
            let value=match &mut block.content {
                Content::Paragraph{text}|Content::Heading{text,..}|Content::Quote{text}|Content::ListItem{text,..}|Content::Caption{text}=>text,
                _=>continue,
            };
            if let Some(recovered)=lines.get(&key(value)).filter(|matches|matches.len()==1).and_then(|matches|matches.iter().next()) {
                if value!=recovered {
                    *value=recovered.clone();
                    if !matches!(block.locator,Locator::Derived{..}){unmapped+=1;}
                    block.locator=Locator::Derived{index:index+1};
                }
            }
        }
    }
    if p.blocks.is_empty(){
        if explicit.is_some(){bail!("no readable blocks found for the explicit CSS selector");}
        return Err(MissingContent::new(
            "html_no_readable_blocks",
            "no readable blocks found; the page may require JavaScript or an explicit selector",
        ).into());
    }
    if unmapped>0{p.warnings.push(Warning::new("approximate_source_mapping",format!("{unmapped} blocks could not be mapped uniquely to an original HTML element.")));}
    let raw_tables=original.select(&selector("main table,article table")?)
        .filter(|table|explicit.is_some()||!chrome_hint(*table)).count();
    let kept_tables=p.blocks.iter().filter(|b|matches!(b.content,Content::Table{..})).count();
    if raw_tables>kept_tables{p.warnings.push(Warning::new("table_coverage_gap",format!("The source main content contains {raw_tables} tables, but extraction retained {kept_tables}.")));}
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
