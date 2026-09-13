//! One content selector, followed by structural conversion and verified source matching.
use std::{collections::{HashMap,HashSet},error::Error,fmt};
use anyhow::{anyhow,bail,Result};
use scraper::{ElementRef,Html,Selector};
use serde_json::json;
use url::Url;
use webtool_protocol::*;
use super::Parsed;

pub const PARSER:&str="rs-trafilatura/0.2.2+main-content/9";

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
fn compact(s:&str)->String{s.chars().filter(|c|!c.is_whitespace()).collect()}
fn text(e:ElementRef<'_>)->String{e.text().collect::<String>()}
fn code_language(e:ElementRef<'_>)->Option<String>{
    std::iter::once(e).chain(e.select(&Selector::parse("code").expect("constant selector")))
        .filter_map(|node|node.value().attr("class")).find_map(|classes|{
            let mut tokens=classes.split_whitespace();
            while let Some(token)=tokens.next(){
                let value=token.strip_prefix("language-").or_else(||token.strip_prefix("lang-"))
                    .or_else(||token.strip_prefix("brush:").and_then(|value|if value.is_empty(){tokens.next()}else{Some(value)}));
                if let Some(value)=value.map(|value|value.trim_end_matches(';')).filter(|value|!value.is_empty()){
                    return Some(value.into());
                }
            }
            None
        })
}
// Numeric scripts retain their displayed meaning. Longer expressions use an
// explicit text notation rather than an invented Unicode approximation.
fn script_text(e:ElementRef<'_>)->Option<String>{
    let tag=e.value().name();
    if !matches!(tag,"sup"|"sub") || e.select(&Selector::parse("a").expect("constant selector")).next().is_some()
        || e.ancestors().filter_map(ElementRef::wrap).any(|a|matches!(a.value().name(),"pre"|"code"|"math"|"table")){return None;}
    let value=normalized(&text(e));
    if value.is_empty(){return None;}
    let alphabet=if tag=="sup"{"⁰¹²³⁴⁵⁶⁷⁸⁹⁺⁻⁼⁽⁾"}else{"₀₁₂₃₄₅₆₇₈₉₊₋₌₍₎"};
    let converted:Option<String>=value.chars().map(|c|"0123456789+-=()".chars().position(|v|v==if c=='−'{'-'}else{c})
        .and_then(|index|alphabet.chars().nth(index))).collect();
    Some(converted.unwrap_or_else(||format!("{}({value})",if tag=="sup"{"^"}else{"_"})))
}
fn script_prose(e:ElementRef<'_>)->String{
    let mut out=String::new();
    for child in e.children(){
        if let Some(value)=child.value().as_text(){out.push_str(value);}
        else if let Some(child)=ElementRef::wrap(child){out.push_str(&script_text(child).unwrap_or_else(||script_prose(child)));}
    }
    out
}
fn supplied_math(e:ElementRef<'_>)->String{
    e.select(&Selector::parse("annotation").expect("constant selector")).find_map(|annotation|
        annotation.value().attr("encoding").filter(|encoding|encoding.to_ascii_lowercase().contains("tex"))
            .map(|_|text(annotation).trim().to_owned())).filter(|value|!value.is_empty())
        .unwrap_or_else(||"[MathML source required; see retained original]".into())
}
struct MathSource{text:String,locator:Locator,inline:bool,before:String,after:String}
type MathSources=HashMap<String,MathSource>;
// Find the actual source boundary around a math representation, including
// transparent accessibility/image wrappers. Do not infer it from punctuation.
fn math_separator(mut e:ElementRef<'_>,before:bool)->String{
    loop{
        let siblings=if before{e.prev_siblings().collect::<Vec<_>>()}else{e.next_siblings().collect::<Vec<_>>()};
        for sibling in siblings{
            if sibling.value().as_element().is_some_and(|e|e.name()=="math"){return String::new();}
            let mut texts=sibling.descendants().filter_map(|node|node.value().as_text()).collect::<Vec<_>>();
            if before{texts.reverse();}
            for value in texts{
                let edge=if before{value.chars().last()}else{value.chars().next()};
                if let Some(edge)=edge{return if edge.is_whitespace(){" ".into()}else{String::new()};}
            }
        }
        let Some(parent)=e.parent().and_then(ElementRef::wrap) else{return String::new();};
        if is_block(parent){return String::new();}
        e=parent;
    }
}
fn carried_math<'a>(e:ElementRef<'_>,sources:&'a MathSources)->Option<&'a MathSource>{
    if e.value().name()!="code"{return None;}
    e.value().attr("class")?.split_whitespace().find_map(|class|sources.get(class))
}
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
fn popup_widget(e: ElementRef<'_>) -> bool {
    // Remove modal semantics before wrapper stripping can discard them. Do not
    // classify prose by words such as "newsletter" or "subscribe" alone.
    if e.value().name()=="dialog" || has_role(e,"dialog") || has_role(e,"alertdialog")
        || e.value().attr("aria-modal").is_some_and(|value|value.eq_ignore_ascii_case("true")) { return true; }
    if !matches!(e.value().name(),"div"|"section"|"aside"|"form"|"small") { return false; }
    let fixed=e.value().attr("style").is_some_and(|style|style.split(';').any(|declaration| {
        declaration.split_once(':').is_some_and(|(name,value)|name.trim().eq_ignore_ascii_case("position")
            && value.split('!').next().unwrap_or("").trim().eq_ignore_ascii_case("fixed"))
    }));
    let named=[e.value().attr("id"),e.value().attr("class")].into_iter().flatten().any(|value|
        value.to_ascii_lowercase().split(|c:char|!c.is_ascii_alphanumeric())
            .any(|token|matches!(token,"newsletter"|"subscribe"|"subscription"|"signup"|"popup"|"modal"|"overlay")));
    (fixed||named) && e.select(&Selector::parse("input[type=email]").expect("constant selector")).next().is_some()
        && e.select(&Selector::parse("main,article,[role=main],[role=article]").expect("constant selector")).next().is_none()
}
#[cfg(feature="web-extraction")]
fn control_widget(e: ElementRef<'_>) -> bool {
    if !matches!(e.value().name(),"div"|"section"|"aside"|"form")
        || e.value().attr("inert").is_none() || has_role(e,"main") || has_role(e,"article")
        || e.ancestors().filter_map(ElementRef::wrap).any(|a|a.value().name()=="article" || has_role(a,"article")) { return false; }
    // Inert article panels can contain real content. Exclude only inactive
    // choice controls with no prose, headings, lists, code, or table structure.
    e.select(&Selector::parse("input[type=checkbox],input[type=radio],select,[role=checkbox],[role=radio],[role=combobox],[role=listbox]").expect("constant selector")).next().is_some()
        && e.select(&Selector::parse("main,article,[role=main],[role=article],h1,h2,h3,h4,h5,h6,p,li,dt,dd,pre,code,table,blockquote,figcaption,[role=doc-endnotes]").expect("constant selector")).next().is_none()
}
#[cfg(feature="web-extraction")]
fn recommendation_widget(e: ElementRef<'_>) -> bool {
    // An auxiliary browsing-history widget is not a main-content section about
    // recommendations. Require its accessible UI name, not matching body text.
    (e.value().name()=="aside" || has_role(e,"complementary"))
        && e.value().attr("aria-label").is_some_and(|label|label.to_ascii_lowercase().contains("recently viewed"))
        && !e.ancestors().filter_map(ElementRef::wrap).any(|a|matches!(a.value().name(),"main"|"article") || has_role(a,"main") || has_role(a,"article"))
        && e.select(&Selector::parse("main,article,[role=main],[role=article]").expect("constant selector")).next().is_none()
}
#[cfg(feature="web-extraction")]
fn main_scope(e: ElementRef<'_>) -> Option<ElementRef<'_>> {
    for ancestor in e.ancestors().filter_map(ElementRef::wrap) {
        if matches!(ancestor.value().name(),"form"|"aside"|"template"|"script"|"style"|"noscript")
            || chrome_landmark(ancestor) || popup_widget(ancestor) { return None; }
        if matches!(ancestor.value().name(),"main"|"article") || has_role(ancestor,"main") || has_role(ancestor,"article") {
            return Some(ancestor);
        }
    }
    None
}
#[cfg(feature="web-extraction")]
fn disclosure_body(e: ElementRef<'_>) -> bool {
    e.descendants().any(|node|node.value().as_text().is_some_and(|value|!value.trim().is_empty())
        && !node.ancestors().take_while(|ancestor|ancestor.id()!=e.id()).filter_map(ElementRef::wrap)
            .any(|ancestor|matches!(ancestor.value().name(),"summary"|"button"|"script"|"style"|"template"|"noscript"|"form")))
}
#[cfg(feature="web-extraction")]
fn selection_source(original:&Html)->Result<(String,Vec<String>,MathSources)>{
    let mut selection=original.clone();
    let excluded=selection.select(&selector("nav,footer,dialog,aside,[role],[aria-modal],[style],[id],[class],[inert],template,noscript")?)
        // Remove inactive payloads before the extractor can unwrap them into
        // literal markup text. Original bytes and explicit CSS are unchanged.
        .filter(|e|chrome_landmark(*e)||popup_widget(*e)||control_widget(*e)||recommendation_widget(*e)
            || matches!(e.value().name(),"template"|"noscript"))
        .map(|e|e.id()).collect::<Vec<_>>();
    for id in excluded{
        if let Some(mut node)=selection.tree.get_mut(id){node.detach();}
    }
    // Some sites identify a modal through its activating control rather than
    // ARIA. Require a unique named modal target, not a class or hidden flag alone.
    let mut modals=Vec::new();
    let numeric_attribute=regex::Regex::new(r"(\[[A-Za-z_][A-Za-z0-9_-]*=)([0-9]+)(\])").expect("constant pattern");
    for control in selection.select(&selector("button[data-modal],a[data-modal]")?){
        // Some source controls use jQuery-style unquoted numeric values. Quote
        // only those values for CSS parsing, without relaxing explicit selectors.
        let value=control.value().attr("data-modal").unwrap();
        let value=numeric_attribute.replace_all(value,"${1}\"${2}\"${3}");
        let Ok(target)=Selector::parse(&value) else{continue;};
        let targets=selection.select(&target).collect::<Vec<_>>();
        if targets.len()!=1{continue;}
        let panel=targets[0];
        let named=panel.value().attr("class").is_some_and(|classes|classes.split(|c:char|!c.is_ascii_alphanumeric()).any(|token|token.eq_ignore_ascii_case("modal")));
        if named && matches!(panel.value().name(),"div"|"section"|"aside")
            && panel.select(&selector("main,article,[role=main],[role=article]")?).next().is_none()
            && !control.ancestors().any(|ancestor|ancestor.id()==panel.id()){
            modals.push(panel.id());
        }
    }
    for id in modals{if let Some(mut node)=selection.tree.get_mut(id){node.detach();}}
    // Article footers can contain qualifications. The extractor's unbounded
    // footer class filter overrides its article exception. Neutralize that
    // layout token and prose disclaimer labels only within these article footers.
    let footers=selection.select(&selector("article footer,[role=article] footer")?)
        .filter(|e|e.select(&Selector::parse("p,blockquote,cite").unwrap()).any(|p|!text(p).trim().is_empty())
            && e.select(&Selector::parse("form,input,button,select,textarea").unwrap()).next().is_none())
        .map(|e|(e.id(),e.descendants().filter_map(ElementRef::wrap).map(|child|child.id()).collect::<Vec<_>>())).collect::<Vec<_>>();
    for (id,descendants) in footers{
        for child in descendants{
            if let Some(mut node)=selection.tree.get_mut(child){
                if let scraper::node::Node::Element(element)=node.value(){
                    if child==id{element.name.local="section".into();}
                    let qualifier=matches!(element.name(),"p"|"blockquote");
                    for (name,value) in &mut element.attrs{
                        if matches!(name.local.as_ref(),"class"|"id"){
                            *value=value.split_whitespace().map(|token|{
                                let layout=|part:&str|part.eq_ignore_ascii_case("footer") || (qualifier&&part.eq_ignore_ascii_case("disclaimer"));
                                if token.split(['-','_']).any(layout){
                                    token.split(['-','_']).filter(|part|!layout(part)).collect::<Vec<_>>().join("-")
                                }else{token.to_owned()}
                            }).filter(|token|!token.is_empty()).collect::<Vec<_>>().join(" ").into();
                        }
                    }
                }
            }
        }
    }
    // The active extractor already reads delivered collapsed bodies. Keep their
    // visibility state unchanged. Only protect labels and structural boundaries
    // from its button deletion and details/summary wrapper stripping.
    let mut rename=Vec::new();
    let mut unavailable=Vec::new();
    let mut ids:HashMap<&str,Vec<ElementRef<'_>>>=HashMap::new();
    for element in selection.select(&selector("[id]")?) {
        ids.entry(element.value().attr("id").unwrap()).or_default().push(element);
    }
    for summary in selection.select(&selector("details > summary")?).filter(|e|main_scope(*e).is_some()) {
        let details=summary.parent().and_then(ElementRef::wrap).expect("details parent");
        if details.select(&selector("form,input,select,textarea")?).next().is_some() { continue; }
        if details.children().filter_map(ElementRef::wrap).find(|e|e.value().name()=="summary").is_none_or(|first|first.id()!=summary.id()) { continue; }
        if !disclosure_body(details) { unavailable.push(normalized(&text(summary))); }
        let tag=if summary.descendants().filter_map(ElementRef::wrap).any(is_block) { "div" } else { "p" };
        rename.push((summary.id(),tag));
    }
    for control in selection.select(&selector("button[aria-controls],a[aria-controls],[role=button][aria-controls]")?) {
        let Some(scope)=main_scope(control) else { continue; };
        if !matches!(control.value().attr("aria-expanded"),Some("true"|"false"))
            && control.value().attr("data-toggle")!=Some("collapse")
            && control.value().attr("data-bs-toggle")!=Some("collapse") { continue; }
        let targets=control.value().attr("aria-controls").unwrap().split_ascii_whitespace().collect::<Vec<_>>();
        if targets.len()!=1 { continue; }
        let panel=match ids.get(targets[0]) {
            Some(matches) if matches.len()==1 => matches[0],
            _ => continue, // Missing or ambiguous IDs do not identify a source panel.
        };
        if main_scope(panel).is_none_or(|parent|parent.id()!=scope.id())
            || panel.ancestors().any(|ancestor|ancestor.id()==control.id())
            || control.ancestors().any(|ancestor|ancestor.id()==panel.id())
            || panel.select(&selector("form,input,select,textarea")?).next().is_some() { continue; }
        if !disclosure_body(panel) { unavailable.push(normalized(&text(control))); }
        if control.value().name()=="button" {
            let inline_parent=control.ancestors().take_while(|ancestor|ancestor.id()!=scope.id()).filter_map(ElementRef::wrap)
                .any(|ancestor|matches!(ancestor.value().name(),"h1"|"h2"|"h3"|"h4"|"h5"|"h6"|"p"|"li"|"dt"|"dd"));
            rename.push((control.id(),if inline_parent {"span"} else {"p"}));
        }
    }
    drop(ids);
    for (id,tag) in rename {
        if let Some(mut node)=selection.tree.get_mut(id) {
            if let scraper::node::Node::Element(element)=node.value() { element.name.local=tag.into(); }
        }
    }
    // A citation's access-status icon describes its destination, not an access
    // gate on the already-delivered title. Keep other subscription filters.
    let citations=selection.select(&selector("cite span.id-lock-subscription")?)
        .filter(|e|!text(*e).trim().is_empty() && e.select(&Selector::parse("a[href]").unwrap()).count()==1
            && e.select(&Selector::parse("form,input,button,select,textarea,[role=dialog],[aria-modal]").unwrap()).next().is_none())
        .map(|e|e.id()).collect::<Vec<_>>();
    for id in citations{
        if let Some(mut node)=selection.tree.get_mut(id){
            if let scraper::node::Node::Element(element)=node.value(){
                for (name,value) in &mut element.attrs{
                    if name.local.as_ref()=="class"{*value=value.split_whitespace().filter(|token|*token!="id-lock-subscription").collect::<Vec<_>>().join(" ").into();}
                }
            }
        }
    }
    // Omit only a redundant language toolbar paired with its actual code block.
    // A paragraph that happens to say "js" is not a toolbar.
    let toolbars=selection.select(&selector(".code-example > .example-header")?).filter(|header|{
        let parent=header.parent().and_then(ElementRef::wrap).unwrap();
        let codes=parent.children().filter_map(ElementRef::wrap).filter(|e|e.value().name()=="pre").collect::<Vec<_>>();
        codes.len()==1 && code_language(codes[0]).is_some_and(|language|normalized(&text(*header))==language)
            && header.select(&Selector::parse("a,p,pre,table,li,input").unwrap()).next().is_none()
    }).map(|e|e.id()).collect::<Vec<_>>();
    for id in toolbars{if let Some(mut node)=selection.tree.get_mut(id){node.detach();}}
    let scripts=selection.select(&selector("sup,sub")?).filter_map(|e|script_text(e).map(|value|(e.id(),value))).collect::<Vec<_>>();
    for (id,value) in scripts{
        let children=selection.tree.get(id).unwrap().children().map(|node|node.id()).collect::<Vec<_>>();
        for child in children{selection.tree.get_mut(child).unwrap().detach();}
        selection.tree.get_mut(id).unwrap().append(scraper::node::Node::Text(scraper::node::Text{text:value.into()}));
    }
    // The pinned cleaner deletes MathML and the serializer drops fallback images.
    // Carry exact supplied notation through the same selector at its source
    // position. Rejected carriers stay rejected; no source subtree is restored.
    let existing=original.html();
    let maths=selection.select(&selector("math")?).filter(|e|!e.ancestors().filter_map(ElementRef::wrap)
        .any(|a|matches!(a.value().name(),"pre"|"code"|"table"|"math"|"script"|"style"|"template"|"noscript")))
        .map(|e|(e.id(),supplied_math(e))).collect::<Vec<_>>();
    let mut sources=MathSources::new();
    let mut index=0;
    for (id,value) in maths{
        let key=loop{index+=1;let key=format!("webtool-math-{index}");if !existing.contains(&key){break key;}};
        let original_math=original.tree.get(id).and_then(ElementRef::wrap).expect("clone preserves node identity");
        let inline=original_math.value().attr("display")!=Some("block") && !original_math.ancestors().filter_map(ElementRef::wrap)
            .take_while(|e|!is_block(*e)).any(|e|e.value().attr("class").is_some_and(|classes|classes.split_whitespace().any(|class|class=="mwe-math-element-block")));
        sources.insert(key.clone(),MathSource{text:value.clone(),locator:Locator::Html{selector:path(original_math)},inline,
            before:math_separator(original_math,true),after:math_separator(original_math,false)});
        let template=Html::parse_fragment(&format!("<code class=\"{key}\"></code>"));
        let code=template.select(&selector("code")?).next().unwrap();
        let children=selection.tree.get(id).unwrap().children().map(|node|node.id()).collect::<Vec<_>>();
        for child in children{selection.tree.get_mut(child).unwrap().detach();}
        let mut node=selection.tree.get_mut(id).unwrap();
        if let scraper::node::Node::Element(element)=node.value(){
            element.name=code.value().name.clone();
            let class=code.value().attrs.keys().next().unwrap().clone();
            let classes=element.attr("class").map(|classes|format!("{classes} {key}")).unwrap_or(key);
            element.attrs.insert(class,classes.into());
        }
        node.append(scraper::node::Node::Text(scraper::node::Text{text:value.into()}));
    }
    // Originals, exact selectors and independent all-page links stay intact.
    Ok((selection.html(),unavailable,sources))
}
fn chrome_hint(e: ElementRef<'_>) -> bool {
    std::iter::once(e).chain(e.ancestors().filter_map(ElementRef::wrap)).any(|a| {
        if matches!(a.value().name(), "html"|"body") { return false; }
        if chrome_landmark(a) || popup_widget(a) { return true; }
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
// Internal offsets count non-whitespace characters until spacing recovery ends.
// Saved metadata uses half-open UTF-8 byte ranges in the unchanged block text.
type InlineLinks = HashMap<String, Vec<(usize, usize, String)>>;
struct ReadStructure<'a>{
    links:InlineLinks,
    math:MathSources,
    math_spacing:HashMap<String,(String,String)>,
    original_lists:HashMap<String,Vec<ElementRef<'a>>>,
    seen_items:HashSet<String>,
}
impl<'a> ReadStructure<'a>{
    fn new(original:&'a Html,math:MathSources)->Self{
        let mut original_lists:HashMap<String,Vec<ElementRef<'a>>>=HashMap::new();
        for item in original.select(&Selector::parse("li").expect("constant selector")).filter(|e|!ignored(*e)){
            original_lists.entry(compact(&script_prose(item))).or_default().push(item);
        }
        Self{links:InlineLinks::new(),math,math_spacing:HashMap::new(),original_lists,seen_items:HashSet::new()}
    }
    fn list_item(&mut self,e:ElementRef<'_>,origin:Option<ElementRef<'_>>,p:&mut Parsed){
        let Some(item)=std::iter::once(e).chain(e.ancestors().filter_map(ElementRef::wrap)).find(|a|a.value().name()=="li") else{return;};
        let source=origin.and_then(|e|std::iter::once(e).chain(e.ancestors().filter_map(ElementRef::wrap)).find(|a|a.value().name()=="li"))
            .or_else(||self.original_lists.get(&compact(&text(item))).filter(|items|items.len()==1).map(|items|items[0])).unwrap_or(item);
        let depth=item.ancestors().filter_map(ElementRef::wrap).filter(|a|matches!(a.value().name(),"ul"|"ol")).count().saturating_sub(1);
        let ordinal=source.parent().and_then(ElementRef::wrap).filter(|list|list.value().name()=="ol").map(|list|{
            let reversed=list.value().attr("reversed").is_some();
            let items=list.children().filter_map(ElementRef::wrap).filter(|e|e.value().name()=="li").collect::<Vec<_>>();
            let mut number=list.value().attr("start").and_then(|v|v.parse::<i64>().ok()).unwrap_or(if reversed{items.len() as i64}else{1});
            for sibling in items{
                if let Some(value)=sibling.value().attr("value").and_then(|v|v.parse().ok()){number=value;}
                if sibling.id()==source.id(){break;}
                number=number.saturating_add(if reversed{-1}else{1});
            }
            number
        });
        let first=self.seen_items.insert(path(item));
        let id=&p.blocks.last().expect("just pushed block").id;
        p.metadata["list_items"][id]=json!({"depth":depth,"ordinal":ordinal,"first":first});
    }
}
#[derive(Default)]
struct ProseRun<'a> {
    text:String,
    characters:usize,
    links:Vec<(usize,usize,ElementRef<'a>)>,
}
fn containing_link(e:ElementRef<'_>)->Option<ElementRef<'_>>{
    std::iter::once(e).chain(e.ancestors().filter_map(ElementRef::wrap))
        .find(|a|a.value().name()=="a" && a.value().attr("href").is_some())
}
impl<'a> ProseRun<'a> {
    fn append(&mut self,value:&str,link:Option<ElementRef<'a>>){
        let start=self.characters;
        self.text.push_str(value);
        self.characters+=value.chars().filter(|c|!c.is_whitespace()).count();
        if let Some(link)=link.filter(|_|start<self.characters){
            if let Some((_,end,_))=self.links.last_mut().filter(|(_,end,previous)|*end==start && previous.id()==link.id()){
                *end=self.characters;
            }else{self.links.push((start,self.characters,link));}
        }
    }
    fn record(&self,p:&Parsed,base:Option<&Url>,links:&mut InlineLinks){
        let spans=self.links.iter().filter_map(|(start,end,anchor)|{
            let href=anchor.value().attr("href")?.trim();
            if href.is_empty(){return None;}
            Some((*start,*end,absolute(base,href)?))
        }).collect::<Vec<_>>();
        if !spans.is_empty(){links.insert(p.blocks.last().expect("just pushed block").id.clone(),spans);}
    }
}
fn linked_text(e:ElementRef<'_>)->ProseRun<'_>{
    let mut run=ProseRun::default();
    for node in e.descendants(){
        if let Some(value)=node.value().as_text(){
            run.append(value,node.parent().and_then(ElementRef::wrap).and_then(containing_link));
        }
    }
    run
}
fn finish_inline_links(p:&mut Parsed,links:InlineLinks){
    let mut records=serde_json::Map::new();
    for block in &p.blocks{
        let Some(spans)=links.get(&block.id) else{continue;};
        let value=block.content.text();
        let characters=value.char_indices().filter(|(_,c)|!c.is_whitespace())
            .map(|(index,c)|(index,index+c.len_utf8())).collect::<Vec<_>>();
        let ranges=spans.iter().filter_map(|(start,end,url)|{
            if start>=end || *end>characters.len(){return None;}
            Some(json!({"start":characters[*start].0,"end":characters[*end-1].1,"url":url}))
        }).collect::<Vec<_>>();
        if !ranges.is_empty(){records.insert(block.id.clone(),json!(ranges));}
    }
    if !records.is_empty(){p.metadata["inline_links"]=json!(records);}
}

// Walk only the selected tree. Never substitute an original container subtree:
// doing so could restore navigation or other descendants removed by selection.
enum Part<'a> { Text(String,Option<ElementRef<'a>>), Block(ElementRef<'a>) }
fn parts<'a>(e: ElementRef<'a>, out: &mut Vec<Part<'a>>, math:&MathSources) {
    for child in e.children() {
        if let Some(t) = child.value().as_text() {
            out.push(Part::Text(t.to_string(),containing_link(e)));
        } else if let Some(child) = ElementRef::wrap(child) {
            if matches!(child.value().name(), "script"|"style"|"noscript"|"template") { continue; }
            if is_block(child) || carried_math(child,math).is_some() { out.push(Part::Block(child)); }
            else if child.value().name() == "br" { out.push(Part::Text("\n".into(),containing_link(child))); }
            else { parts(child, out,math); }
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
fn flush_run(e: ElementRef<'_>, run: &mut ProseRun<'_>, p: &mut Parsed, unmapped: &mut usize, base:Option<&Url>, structure:&mut ReadStructure<'_>) {
    let value = normalized(&run.text);
    if !value.is_empty() {
        // A fragment of a container is not an exact whole-element location.
        *unmapped += 1;
        p.push(prose(e, value), Locator::Derived { index:p.blocks.len()+1 });
        run.record(p,base,&mut structure.links);
        structure.list_item(e,None,p);
    }
    *run=ProseRun::default();
}
fn emit(e: ElementRef<'_>, origins: &Origins<'_>, base: Option<&Url>, filter_chrome: bool, p: &mut Parsed, unmapped: &mut usize, structure:&mut ReadStructure<'_>) -> Result<()> {
    if ignored(e) || matches!(e.value().name(), "script"|"style"|"noscript"|"template") { return Ok(()); }
    if let Some(source)=carried_math(e,&structure.math){
        p.push(Content::Math{text:source.text.clone()},source.locator.clone());
        if source.inline{structure.math_spacing.insert(p.blocks.last().unwrap().id.clone(),(source.before.clone(),source.after.clone()));}
        structure.list_item(e,None,p);
        return Ok(());
    }
    if !is_block(e) {
        // Keep inline descendants in one prose run. Structural transparent
        // containers remain boundaries, so repeated cards keep source order.
        let mut run = ProseRun::default();
        for child in e.children() {
            if let Some(t) = child.value().as_text() { run.append(t,containing_link(e)); }
            else if let Some(child) = ElementRef::wrap(child) {
                if matches!(child.value().name(), "script"|"style"|"noscript"|"template") { continue; }
                if is_inline(child) {
                    let mut inline_parts=Vec::new();
                    parts(child,&mut inline_parts,&structure.math);
                    for part in inline_parts {
                        match part {
                            Part::Text(value,link)=>run.append(&value,link),
                            Part::Block(block)=>{
                                flush_run(e,&mut run,p,unmapped,base,structure);
                                emit(block,origins,base,filter_chrome,p,unmapped,structure)?;
                            },
                        }
                    }
                } else {
                    flush_run(e, &mut run, p, unmapped, base, structure);
                    emit(child, origins, base, filter_chrome, p, unmapped, structure)?;
                }
            }
        }
        flush_run(e, &mut run, p, unmapped, base, structure);
        return Ok(());
    }
    let tag = e.value().name();
    if !matches!(tag, "pre"|"table"|"img"|"math") {
        let mut children = Vec::new();
        parts(e, &mut children,&structure.math);
        if children.iter().any(|part| matches!(part, Part::Block(_))) {
            let inline_flow=tag=="p" && children.iter().all(|part|match part{
                Part::Text(..)=>true,Part::Block(e)=>carried_math(*e,&structure.math).is_some_and(|source|source.inline),
            });
            let start=p.blocks.len();
            let mut run = ProseRun::default();
            for child in children {
                match child {
                    Part::Text(t,link) => run.append(&t,link),
                    Part::Block(child) => {
                        flush_run(e, &mut run, p, unmapped, base, structure);
                        emit(child, origins, base, filter_chrome, p, unmapped, structure)?;
                    }
                }
            }
            flush_run(e, &mut run, p, unmapped, base, structure);
            if inline_flow && p.blocks.len()>start+1{
                let blocks=&p.blocks[start..];
                let separators=blocks.windows(2).map(|pair|structure.math_spacing.get(&pair[0].id).map(|(_,after)|after.clone())
                    .or_else(||structure.math_spacing.get(&pair[1].id).map(|(before,_)|before.clone())).unwrap_or_default()).collect::<Vec<_>>();
                if p.metadata["inline_flows"].is_null(){p.metadata["inline_flows"]=json!([]);}
                p.metadata["inline_flows"].as_array_mut().unwrap().push(json!({"blocks":blocks.iter().map(|b|&b.id).collect::<Vec<_>>(),"separators":separators}));
            }
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
            let language = code_language(code);
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
            Content::Math { text:supplied_math(math) }
        },
        _ => prose(e, normalized(&raw)),
    };
    p.push(content, locator);
    if !matches!(tag,"pre"|"table"|"img"|"math"){linked_text(e).record(p,base,&mut structure.links);}
    structure.list_item(e,origin,p);
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
    let title=original.select(&selector("title")?).next().map(text).filter(|value|!value.trim().is_empty());
    let mut display_title=title.as_deref().map(normalized).unwrap_or_else(||url.into());
    // Prefer the page's own title over a JSON-LD headline that may be a short
    // description. Remove a site suffix only when one main h1 confirms it.
    let headings=original.select(&selector("main h1,article h1,[role=main] h1")?).filter(|e|!chrome_hint(*e))
        .map(|e|normalized(&text(e))).filter(|value|!value.is_empty()).collect::<HashSet<_>>();
    if headings.len()==1{
        let heading=headings.iter().next().unwrap();
        if display_title.strip_prefix(heading).is_some_and(|suffix|suffix.is_empty() || [" - "," | "," – "," — "].iter().any(|separator|suffix.starts_with(separator))){display_title=heading.clone();}
    }
    let mut p=Parsed::new(&display_title,PARSER);
    p.links=links(source,url);
    let mut readable_text:Option<String>=None;
    let mut unavailable_disclosures=Vec::new();
    let mut math_sources=MathSources::new();
    let selected=if let Some(css)=explicit{
        p.parser="explicit-css+source-blocks/6".into();
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
            let (selection,unavailable,math)=selection_source(&original)?;
            unavailable_disclosures=unavailable;
            math_sources=math;
            let r=rs_trafilatura::extract_with_options(&selection,&options).map_err(|e|anyhow!("HTML extraction failed: {e}"))?;
            readable_text=Some(r.content_text);
            if title.is_none(){if let Some(t)=&r.metadata.title{p.title=t.clone();}}
            p.metadata=json!({"extractor_estimated_quality":r.extraction_quality,"quality_estimate_is_not_validation":true,"source_title":title,"extractor_title":r.metadata.title});
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
    let mut structure=ReadStructure::new(&original,math_sources);
    emit(cleaned.root_element(), &origins, base.as_ref(), explicit.is_none(), &mut p, &mut unmapped, &mut structure)?;
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
        for element in original.select(&selector("div,p,li,blockquote,figcaption,details,summary")?) {
            if ignored(element){continue;}
            // Direct disclosure text can lose whitespace next to inline links
            // when wrappers are stripped. Use only identical-character runs,
            // with their actual source whitespace, never an original subtree.
            let disclosure=matches!(element.value().name(),"details"|"summary");
            let mut run=String::new();let mut spaced=false;
            let mut record=|run:&mut String,spaced:&mut bool|{
                if (*spaced||disclosure)&&!run.trim().is_empty(){let line=normalized(run);lines.entry(key(&line)).or_default().insert(line);}
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
        // Prefer actual source spacing when a complete selected run has one
        // identical-character original. This also keeps possessives and citation
        // brackets adjacent to their labels. No missing words are reconstructed.
        let mut source_lines:HashMap<String,HashSet<String>>=HashMap::new();
        for element in original.select(&selector("p,li,figcaption,blockquote,h1,h2,h3,h4,h5,h6")?).filter(|e|!ignored(*e)){
            let line=normalized(&script_prose(element));
            source_lines.entry(key(&line)).or_default().insert(line);
        }
        for (key,values) in source_lines{if values.len()==1{lines.insert(key,values);}}
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
    finish_inline_links(&mut p,structure.links);
    if p.blocks.is_empty(){
        if explicit.is_some(){bail!("no readable blocks found for the explicit CSS selector");}
        return Err(MissingContent::new(
            "html_no_readable_blocks",
            "no readable blocks found; the page may require JavaScript or an explicit selector",
        ).into());
    }
    let unavailable=unavailable_disclosures.into_iter().filter(|label|!label.is_empty()
        && p.blocks.iter().any(|block|normalized(&block.content.text())==*label)).collect::<HashSet<_>>().len();
    if unavailable>0 {
        let noun=if unavailable==1 {"section"} else {"sections"};
        p.warnings.push(Warning::new("disclosure_content_unavailable",format!("The captured page has no readable body for {unavailable} selected disclosure {noun}. Content loaded only after interaction is not fetched.")));
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
    #[cfg(feature="web-extraction")]
    #[test]
    fn disclosures_keep_labels_bodies_and_source_values() {
        let source=r#"<html><head><title>Field guide</title></head><body><main><article>
            <h1>Field guide</h1><p>Keep original field observations unchanged and record the instrument used for each measurement.</p>
            <details><summary>References</summary>Read the handbook. <a href="/book">Source reference</a></details>
            <h2><button aria-expanded="false" aria-controls="values">Measurement limits</button></h2>
            <div id="values" hidden aria-hidden="true" style="display:none"><p>A zero value is not a missing value. Keep its unit and uncertainty.</p>
            <pre><code>  let x = "&lt;details&gt;";
</code></pre><table><tr><th>Value</th></tr><tr><td>0</td></tr></table></div>
            <h2><button aria-expanded="false" aria-controls="empty">Appendix</button></h2><div id="empty" hidden></div>
            </article></main></body></html>"#;
        let parsed=parse(source.as_bytes(),"https://example.com/guide",None).unwrap();
        let values=parsed.blocks.iter().map(|b|b.content.text()).collect::<Vec<_>>();
        assert!(values.iter().any(|text|text=="References"));
        assert!(values.iter().any(|text|text=="Read the handbook. Source reference"));
        assert_eq!(values.iter().filter(|text|*text=="Measurement limits").count(),1);
        assert!(values.iter().any(|text|text.contains("A zero value is not a missing value.")));
        assert!(parsed.blocks.iter().any(|b|matches!(&b.content,Content::Code{text,..} if text=="  let x = \"<details>\";\n")));
        assert!(parsed.blocks.iter().any(|b|matches!(&b.content,Content::Table{rows} if rows[1][0].text=="0")));
        assert!(parsed.warnings.iter().any(|warning|warning.code=="disclosure_content_unavailable"));
        assert_eq!(parsed.links,links(source,"https://example.com/guide"));
        let explicit=parse(source.as_bytes(),"https://example.com/guide",Some("#values")).unwrap();
        assert_eq!(explicit.parser,"explicit-css+source-blocks/6");
        assert!(!explicit.warnings.iter().any(|warning|warning.code=="disclosure_content_unavailable"));
    }
    #[cfg(feature="web-extraction")]
    #[test]
    fn selection_filters_widgets_before_wrappers_and_limits_label_normalization() {
        let source=r#"<main><article data-topic-id="42"><p>Newsletters are a topic, not a reason to delete article prose.</p>
            <small role="dialog" aria-modal="true">Modal promotion</small>
            <div style="position: fixed!important"><p>Email promotion</p><input type="email"></div>
            <nav><details><summary>Account menu</summary>Navigation body</details></nav>
            <template><p>Inactive template</p></template>
            <noscript>Inactive fallback &lt;div&gt;markup&lt;/div&gt;</noscript>
            <div inert><input type="checkbox"><span>Keep article controls.</span></div>
            <div inert><p>Keep delivered inert prose.</p><input type="checkbox"></div>
            <div inert><pre><code>literal &lt;noscript&gt; example</code></pre><input type="checkbox"></div>
            <button data-modal="[data-topic-id=42] .print-modal">Print</button><div class="print-modal d-none"><p>Print dialog instructions</p></div>
            <footer class="editorial-footer"><h3>Editorial note</h3><p class="footer-disclaimer">Keep this article qualification.</p></footer>
            <h2><button id="ambiguous" aria-expanded="false" aria-controls="duplicate">Ambiguous</button></h2>
            <div id="duplicate">First</div><div id="duplicate">Second</div>
            <h2><button id="form-toggle" aria-expanded="false" aria-controls="account">Account</button></h2>
            <div id="account"><form><input type="password"></form></div>
            <h2><button id="section-toggle" aria-expanded="false" aria-controls="section">Section</button></h2>
            <div id="section" hidden aria-hidden="true"><p>Retained delivered content.</p></div>
            </article>
            <div inert><input type="checkbox"><span>Inactive filter controls</span></div>
            </main>
            <aside aria-label="Recently viewed items"><span>Personalized widget</span></aside>
            <aside aria-label="Research recommendations"><p>Keep research recommendations.</p></aside>"#;
        let original=Html::parse_document(source);
        let (selection,_,_)=selection_source(&original).unwrap();
        for unwanted in ["Modal promotion","Email promotion","Account menu","Inactive template","Inactive fallback","Inactive filter controls","Personalized widget","Print dialog instructions"] { assert!(!selection.contains(unwanted),"{unwanted}"); }
        for retained in ["Newsletters are a topic","Keep article controls.","Keep delivered inert prose.","literal &lt;noscript&gt; example","Keep research recommendations.","Keep this article qualification."] { assert!(selection.contains(retained),"{retained}"); }
        let selected=Html::parse_document(&selection);
        for id in ["ambiguous","form-toggle"] { assert_eq!(selected.select(&selector(&format!("#{id}")).unwrap()).next().unwrap().value().name(),"button"); }
        assert_eq!(selected.select(&selector("#section-toggle").unwrap()).next().unwrap().value().name(),"span");
        let panel=selected.select(&selector("#section").unwrap()).next().unwrap();
        assert!(panel.value().attr("hidden").is_some());
        assert_eq!(panel.value().attr("aria-hidden"),Some("true"));
        assert!(original.html().contains("Modal promotion"));
    }
    #[cfg(feature="web-extraction")]
    #[test]
    fn selected_links_keep_exact_spans_after_spacing_recovery(){
        let source=r#"<html><head><title>Reading guide</title></head><body><main><article>
            <p>Read <a href="/docs?x=1&amp;y=2">the <em>β guide</em></a> now. The documentation explains how to preserve source values and inspect their original context without changing the supplied evidence.</p>
            <div><a href="/one">Learn <strong>more</strong></a> <a href="/two">Learn more</a></div>
            <pre><code>  literal &lt;a href='/code'&gt; example
</code></pre></article></main></body></html>"#;
        for explicit in [None,Some("main")]{
            let parsed=parse(source.as_bytes(),"https://example.com/guide",explicit).unwrap();
            let mut links=Vec::new();
            for block in &parsed.blocks{
                let value=block.content.text();
                if let Some(spans)=parsed.metadata["inline_links"][&block.id].as_array(){
                    for span in spans{
                        links.push((value[span["start"].as_u64().unwrap() as usize..span["end"].as_u64().unwrap() as usize].to_owned(),span["url"].as_str().unwrap().to_owned()));
                    }
                }
                if matches!(block.content,Content::Code{..}){
                    assert_eq!(value,"  literal <a href='/code'> example\n");
                    assert!(parsed.metadata["inline_links"][&block.id].is_null());
                }
            }
            assert!(links.contains(&("the β guide".into(),"https://example.com/docs?x=1&y=2".into())));
            assert!(links.contains(&("Learn more".into(),"https://example.com/one".into())));
            assert!(links.contains(&("Learn more".into(),"https://example.com/two".into())));
        }
    }
    #[cfg(feature="web-extraction")]
    #[test]
    fn selected_notation_citations_and_lists_keep_source_meaning(){
        let source=r#"<html><head><title>Field science - Reference</title></head><body><main><article>
            <h1>Field science</h1><p>Keep the measured amount at 10<sup>23</sup> particles per dm<sup>3</sup>, and keep the H<sub>2</sub>O formula readable. These source values describe the experiment and must not become ordinary adjacent digits.</p>
            <p>The probability factor is <math><semantics><mi>x</mi><annotation encoding="application/x-tex">  e^{-E/kT}  </annotation></semantics></math> before the remaining explanation of the measured reaction rate.</p>
            <p>The next equation has no supplied text notation: <math><mi>x</mi><mo>+</mo><mi>y</mi></math> and requires its original representation.</p>
            <ol start="7" reversed><li><p>First resource description.</p><ul><li>A nested resource.</li></ul><p>Continuation of the same resource.</p></li><li value="3">Second resource description.</li></ol>
            <p><cite>Newman (2011). <span class="id-lock-subscription" title="Paid subscription required"><a href="/paper">What Have We Learned?</a></span> Research journal.</cite> This citation title is already delivered, not permission to fetch its destination.</p>
            <div class="code-example"><div class="example-header"><span class="language-name">js</span></div><pre class="brush: js notranslate"><code>  const n = '&lt;math&gt;';
</code></pre></div><table><tr><th>Value</th></tr><tr><td>0</td></tr></table>
            <nav><math><annotation encoding="application/x-tex">NAV_EQUATION</annotation></math></nav>
            <div role="dialog"><p>Do not restore this subscription gate.</p><input type="email"></div>
            <footer class="editorial-footer"><h3>Editorial note</h3><p class="footer-disclaimer">Analysis uses the supplied measurements. Conditions can change rapidly.</p></footer>
            </article></main></body></html>"#;
        let parsed=parse(source.as_bytes(),"https://example.com/science",None).unwrap();
        assert_eq!(parsed.title,"Field science");
        let all=parsed.blocks.iter().map(|block|block.content.text()).collect::<Vec<_>>();
        assert!(all.iter().any(|value|value.contains("10²³ particles per dm³, and keep the H₂O")),"{all:?}");
        let equation=parsed.blocks.iter().position(|block|matches!(&block.content,Content::Math{text} if text=="e^{-E/kT}")).unwrap();
        assert!(all[equation-1].ends_with("factor is"));
        assert!(all[equation+1].starts_with("before the remaining"));
        let flow=&parsed.metadata["inline_flows"][0];
        assert_eq!(flow["blocks"],json!([parsed.blocks[equation-1].id,parsed.blocks[equation].id,parsed.blocks[equation+1].id]));
        assert_eq!(flow["separators"],json!([" "," "]));
        assert_eq!(parsed.blocks.iter().filter(|block|matches!(block.content,Content::Math{..})).count(),2);
        assert!(all.iter().any(|value|value=="[MathML source required; see retained original]"));
        assert!(all.iter().any(|value|value.contains("What Have We Learned?")));
        assert!(all.iter().any(|value|value=="Analysis uses the supplied measurements. Conditions can change rapidly."));
        for excluded in ["NAV_EQUATION","Do not restore","webtool-math-", "js"]{assert!(!all.iter().any(|value|if excluded=="js"{value==excluded}else{value.contains(excluded)}));}
        let item=|label:&str|{let block=parsed.blocks.iter().find(|block|block.content.text()==label).unwrap();parsed.metadata["list_items"][&block.id].clone()};
        assert_eq!(item("First resource description."),json!({"depth":0,"ordinal":7,"first":true}));
        assert_eq!(item("A nested resource."),json!({"depth":1,"ordinal":null,"first":true}));
        assert_eq!(item("Continuation of the same resource."),json!({"depth":0,"ordinal":7,"first":false}));
        assert_eq!(item("Second resource description."),json!({"depth":0,"ordinal":3,"first":true}));
        assert!(parsed.blocks.iter().any(|block|matches!(&block.content,Content::Code{language:Some(language),text} if language=="js" && text=="  const n = '<math>';\n")));
        assert!(parsed.blocks.iter().any(|block|matches!(&block.content,Content::Table{rows} if rows[1][0].text=="0")));
        assert_eq!(parsed.links,links(source,"https://example.com/science"));
    }
    #[test]fn explicit_selector_preserves_code(){let p=parse(SAMPLE.as_bytes(),"https://example.com",Some("main")).unwrap();let c=p.blocks.iter().find(|b|matches!(b.content,Content::Code{..})).unwrap();assert_eq!(c.content.text(),"  let n = 0;\n");assert!(matches!(c.locator,Locator::Html{..}));}
    #[test]fn table_is_not_duplicated_as_paragraphs(){let p=parse(SAMPLE.as_bytes(),"https://example.com",Some("main")).unwrap();assert_eq!(p.blocks.iter().filter(|b|matches!(b.content,Content::Table{..})).count(),1);}
    #[test]fn bad_css_is_an_error(){assert!(select_original(SAMPLE,"[").is_err());}
    #[test]fn relative_links_are_resolved(){assert_eq!(links("<a href='/one'>one</a><a href='javascript:x'>bad</a>","https://example.com").len(),1);}
    #[test]fn absent_selector_is_an_error(){assert!(parse(SAMPLE.as_bytes(),"x",Some(".not-here")).is_err());}
}
