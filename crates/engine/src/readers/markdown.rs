//! CommonMark semantics and exact source slices use the same offset parser.
//! Source slices retain syntax that the shared block model cannot express.
use std::ops::Range;
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag};
use serde_json::{json, Value};
use webtool_protocol::{Content, Cell, Link, Locator, Warning};
use super::Parsed;

pub const PARSER: &str = "pulldown-cmark/0.13.4+source-blocks/1";

struct Node<'a> {
    event: Event<'a>,
    range: Range<usize>,
    children: Vec<Node<'a>>,
}
fn nodes<'a>(events: &mut impl Iterator<Item=(Event<'a>,Range<usize>)>) -> Vec<Node<'a>> {
    let mut result = Vec::new();
    while let Some((event,range)) = events.next() {
        if matches!(event,Event::End(_)) { break; }
        let children = if matches!(event,Event::Start(_)) { nodes(events) } else { Vec::new() };
        result.push(Node { event,range,children });
    }
    result
}
#[derive(Default)]
struct Inline {
    text: String,
    links: Vec<Value>,
    styles: Vec<Value>,
    images: Vec<(Range<usize>,String,String)>,
}
impl Inline {
    fn visit(&mut self,node:&Node<'_>) {
        let start=self.text.len();
        match &node.event {
            Event::Text(value)|Event::Code(value)|Event::Html(value)|Event::InlineHtml(value)=>self.text.push_str(value),
            Event::InlineMath(value)=>self.text.push_str(&format!("${value}$")),
            Event::DisplayMath(value)=>self.text.push_str(&format!("$${value}$$")),
            Event::SoftBreak|Event::HardBreak=>self.text.push('\n'),
            Event::FootnoteReference(label)=>self.text.push_str(&format!("[^{label}]")),
            Event::TaskListMarker(checked)=>self.text.push_str(if *checked {"[x] "} else {"[ ] "}),
            _=>for child in &node.children { self.visit(child); },
        }
        let end=self.text.len();
        if let Event::Start(Tag::Image{dest_url,..})=&node.event {
            self.images.push((start..end,dest_url.to_string(),self.text[start..end].to_owned()));
        }
        if let Event::Start(Tag::Link{dest_url,..})=&node.event {
            if start<end { self.links.push(json!({"start":start,"end":end,"url":dest_url.as_ref()})); }
        }
        let kind=match node.event {
            Event::Start(Tag::Emphasis)=>Some("italic"),Event::Start(Tag::Strong)=>Some("bold"),
            Event::Start(Tag::Strikethrough)=>Some("strikethrough"),Event::Code(_)=>Some("code"),
            Event::InlineMath(_)=>Some("math"),_=>None,
        };
        if let Some(kind)=kind.filter(|_|start<end) { self.styles.push(json!({"start":start,"end":end,"kind":kind})); }
    }
}
struct Item { ordinal: Option<i64>, first: bool }
struct Reader<'a> { source:&'a str, line_starts:Vec<usize>, parsed:Parsed, items:Vec<Item>, quotes:usize }
impl Reader<'_> {
    fn location(&self,range:&Range<usize>)->Locator {
        let line=|offset|self.line_starts.partition_point(|start|*start<=offset).max(1);
        Locator::Lines { start:line(range.start),end:line(range.end.saturating_sub(1).max(range.start)) }
    }
    fn push(&mut self,content:Content,range:&Range<usize>) {
        self.parsed.push(content,self.location(range));
        let id=self.parsed.blocks.last().unwrap().id.clone();
        let depth=self.items.len().saturating_sub(1);
        if let Some(item)=self.items.last_mut() {
            self.parsed.metadata["list_items"][&id]=json!({"depth":depth,"ordinal":item.ordinal,"first":item.first});
            item.first=false;
        }
        if self.quotes>0 { self.parsed.metadata["quote_depth"][&id]=json!(self.quotes); }
    }
    fn inline(&mut self,nodes:&[Node<'_>],range:&Range<usize>,heading:Option<u8>) {
        if heading.is_none() && nodes.len()==1 {
            if let Event::DisplayMath(text)=&nodes[0].event {
                self.push(Content::Math{text:format!("$${text}$$")},range);return;
            }
        }
        let mut inline=Inline::default();
        for node in nodes { inline.visit(node); }
        for span in &inline.links {
            let start=span["start"].as_u64().unwrap() as usize; let end=span["end"].as_u64().unwrap() as usize;
            self.parsed.links.push(Link{url:span["url"].as_str().unwrap().into(),text:inline.text[start..end].into()});
        }
        let mut cursor=0;
        // An inline image remains an image block, rather than an invented fetched asset.
        for (image,url,alt) in &inline.images {
            if image.start<cursor { continue; }
            self.text_fragment(&inline,cursor..image.start,range,heading);
            self.push(Content::Image{url:url.clone(),alt:alt.clone()},range);
            cursor=image.end;
        }
        self.text_fragment(&inline,cursor..inline.text.len(),range,heading);
    }
    fn text_fragment(&mut self,inline:&Inline,fragment:Range<usize>,range:&Range<usize>,heading:Option<u8>) {
        let text=&inline.text[fragment.clone()];
        if text.is_empty() { return; }
        let content=if let Some(level)=heading {
            if level==1 && self.parsed.title==self.parsed.metadata["input_name"].as_str().unwrap_or("") {self.parsed.title=text.into();}
            Content::Heading{level,text:text.into()}
        } else if self.items.last().is_some_and(|item|item.first) {
            Content::ListItem{ordered:self.items.last().unwrap().ordinal.is_some(),text:text.into()}
        } else { Content::Paragraph{text:text.into()} };
        self.push(content,range);
        let id=self.parsed.blocks.last().unwrap().id.clone();
        for (key,spans) in [("inline_links",&inline.links),("inline_styles",&inline.styles)] {
            let spans=spans.iter().filter_map(|span|{
                let start=span["start"].as_u64()? as usize; let end=span["end"].as_u64()? as usize;
                if start<fragment.start||end>fragment.end {return None;}
                let mut span=span.clone();span["start"]=json!(start-fragment.start);span["end"]=json!(end-fragment.start);Some(span)
            }).collect::<Vec<_>>();
            if !spans.is_empty() {self.parsed.metadata[key][&id]=json!(spans);}
        }
    }
    fn block(&mut self,node:&Node<'_>,depth:usize) {
        if depth>64 {
            self.push(Content::Code{language:Some("markdown".into()),text:self.source[node.range.clone()].into()},&node.range);
            self.parsed.warnings.push(Warning::new("markdown_depth_limit","Deeply nested Markdown remains literal source syntax rather than inferred structure."));return;
        }
        match &node.event {
            Event::Start(Tag::Heading{level,..})=>self.inline(&node.children,&node.range,Some(*level as u8)),
            Event::Start(Tag::Paragraph)=>self.inline(&node.children,&node.range,None),
            Event::Start(Tag::CodeBlock(kind))=>{
                let language=match kind {CodeBlockKind::Fenced(info)=>info.split_whitespace().next().map(str::to_owned),_=>None};
                let mut text=Inline::default();for child in &node.children {text.visit(child);}
                self.push(Content::Code{language,text:text.text},&node.range);
                if let CodeBlockKind::Fenced(_)=kind {
                    let raw=&self.source[node.range.clone()];
                    let first=raw.lines().next().unwrap_or("").trim_start();
                    let marker=first.chars().next().unwrap_or('`');let count=first.chars().take_while(|c|*c==marker).count();
                    let closed=raw.lines().skip(1).any(|line|{let line=line.trim();let n=line.chars().take_while(|c|*c==marker).count();n>=count&&line[n..].trim().is_empty()});
                    if !closed {self.parsed.warnings.push(Warning::new("unclosed_fence","The source contains an unclosed code fence."));}
                }
            },
            Event::Start(Tag::BlockQuote(_))=>{self.quotes+=1;for child in &node.children{self.block(child,depth+1);}self.quotes-=1;},
            Event::Start(Tag::List(start))=>{
                for (index,item) in node.children.iter().enumerate() {
                    self.items.push(Item{ordinal:start.and_then(|n|n.checked_add(index as u64)).and_then(|n|i64::try_from(n).ok()),first:true});
                    self.block(item,depth+1);self.items.pop();
                }
            },
            Event::Start(Tag::Item)=>{
                let mut start=0;
                for (index,child) in node.children.iter().enumerate() {
                    if block_node(child) {
                        if start<index {self.inline(&node.children[start..index],&node.range,None);}
                        self.block(child,depth+1);start=index+1;
                    }
                }
                if start<node.children.len() {self.inline(&node.children[start..],&node.range,None);}
                if self.items.last().is_some_and(|item|item.first) {self.push(Content::ListItem{text:String::new(),ordered:self.items.last().unwrap().ordinal.is_some()},&node.range);}
            },
            Event::Start(Tag::Table(_))=>{
                let rows=node.children.iter().map(|row|row.children.iter().map(|cell|{
                    let mut inline=Inline::default();for child in &cell.children{inline.visit(child);}
                    for link in &inline.links {self.parsed.links.push(Link{url:link["url"].as_str().unwrap().into(),text:inline.text[link["start"].as_u64().unwrap() as usize..link["end"].as_u64().unwrap() as usize].into()});}
                    Cell{text:inline.text,row_span:1,col_span:1,header:matches!(row.event,Event::Start(Tag::TableHead))}
                }).collect()).collect();
                self.push(Content::Table{rows},&node.range);
            },
            Event::Start(Tag::HtmlBlock)=>{
                let mut text=Inline::default();for child in &node.children{text.visit(child);}
                self.push(Content::Code{language:Some("html".into()),text:text.text},&node.range);
                self.parsed.warnings.push(Warning::new("markdown_raw_html","Raw HTML is retained as source syntax. It is not executed or treated as selected web content."));
            },
            Event::Start(Tag::FootnoteDefinition(label))=>{
                let start=self.parsed.blocks.len();for child in &node.children{self.block(child,depth+1);}
                let ids=self.parsed.blocks[start..].iter().map(|b|b.id.clone()).collect::<Vec<_>>();
                for id in &ids{self.parsed.metadata["block_roles"][id]=json!("footnote");self.parsed.metadata["footnote_labels"][id]=json!(label.as_ref());}
                self.parsed.metadata["footnotes"][label.as_ref()]=json!(ids);
            },
            Event::Rule=>{self.push(Content::Paragraph{text:"---".into()},&node.range);let id=&self.parsed.blocks.last().unwrap().id;self.parsed.metadata["block_kinds"][id]=json!("thematic_break");},
            Event::DisplayMath(text)=>self.push(Content::Math{text:format!("$${text}$$")},&node.range),
            _=>self.inline(std::slice::from_ref(node),&node.range,None),
        }
    }
}
fn block_node(node:&Node<'_>)->bool {
    matches!(node.event,Event::Start(Tag::Paragraph|Tag::Heading{..}|Tag::CodeBlock(_)|Tag::BlockQuote(_)|Tag::List(_)|Tag::Table(_)|Tag::HtmlBlock|Tag::FootnoteDefinition(_))|Event::Rule|Event::DisplayMath(_))
}

pub fn parse(source:&str,name:&str)->Parsed {
    // Do not enable smart punctuation or syntax that changes ordinary CommonMark text.
    let options=Options::ENABLE_TABLES|Options::ENABLE_FOOTNOTES|Options::ENABLE_STRIKETHROUGH|Options::ENABLE_TASKLISTS|Options::ENABLE_MATH;
    let parser=Parser::new_ext(source,options);
    let mut definitions=parser.reference_definitions().iter().map(|(_,definition)|definition.span.clone()).collect::<Vec<_>>();
    definitions.sort_by_key(|range|range.start);
    let references=definitions.iter().map(|range|source[range.clone()].to_owned()).collect::<Vec<_>>();
    let events=parser.into_offset_iter().collect::<Vec<_>>();
    let mut depth=0usize;
    for (event,_) in &events {
        if matches!(event,Event::Start(_)) {depth+=1;}
        if depth>64 {
            let mut p=Parsed::new(name,PARSER);
            p.push(Content::Code{language:Some("markdown".into()),text:source.into()},Locator::Lines{start:1,end:source.lines().count().max(1)});
            p.metadata=json!({"source_markdown":[{"blocks":["b1"],"texts":[source],"markdown":source}]});
            p.warnings.push(Warning::new("markdown_depth_limit","More than 64 nested Markdown containers remain literal source syntax rather than inferred structure."));return p;
        }
        if matches!(event,Event::End(_)) {depth=depth.saturating_sub(1);}
    }
    let roots=nodes(&mut events.into_iter());
    let mut line_starts=vec![0];line_starts.extend(source.bytes().enumerate().filter(|(_,b)|*b==b'\n').map(|(i,_)|i+1));
    let mut reader=Reader{source,line_starts,parsed:Parsed::new(name,PARSER),items:Vec::new(),quotes:0};
    reader.parsed.metadata=json!({"input_name":name,"markdown_extensions":["tables","footnotes","strikethrough","task_lists","math"],"markdown_references":references});
    let mut groups=Vec::new();let mut previous_end=0;
    for (index,root) in roots.iter().enumerate() {
        let first=reader.parsed.blocks.len();reader.block(root,0);
        let end=if index+1==roots.len(){source.len()}else{root.range.end};
        if reader.parsed.blocks.len()>first {
            let blocks=&reader.parsed.blocks[first..];
            groups.push(json!({"blocks":blocks.iter().map(|b|&b.id).collect::<Vec<_>>(),"texts":blocks.iter().map(|b|b.content.text()).collect::<Vec<_>>(),"markdown":&source[previous_end..end]}));
            previous_end=end;
        }
    }
    if reader.parsed.blocks.is_empty() && !source.is_empty() {
        // Definitions-only input is still a retained, inspectable document.
        reader.push(Content::Code{language:Some("markdown".into()),text:source.into()},&(0..source.len()));
        let block=&reader.parsed.blocks[0];groups.push(json!({"blocks":[block.id],"texts":[block.content.text()],"markdown":source}));
    }
    reader.parsed.metadata["source_markdown"]=json!(groups);
    reader.parsed
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn commonmark_structure_and_literal_source() {
        let source="Title\n=====\n\n> Outer\n>\n> 3. **bold** and `x_y`\n>    - inner [link][ref]\n\n    code\n\n---\n\n[ref]: https://example.test/ref \"Title\"\n";
        let p=parse(source,"input.md");
        assert_eq!(p.title,"Title");assert!(p.blocks.iter().any(|b|matches!(&b.content,Content::Code{text,..} if text=="code\n")));
        assert!(p.metadata["list_items"].as_object().unwrap().values().any(|v|v["ordinal"]==3));
        assert!(p.metadata["quote_depth"].as_object().unwrap().values().any(|v|v==1));
        assert!(p.links.iter().any(|l|l.url=="https://example.test/ref"&&l.text=="link"));
        let retained=p.metadata["source_markdown"].as_array().unwrap().iter().map(|g|g["markdown"].as_str().unwrap()).collect::<String>();assert_eq!(retained,source);
    }
    #[test] fn tables_math_images_and_escapes() {
        let p=parse("# A\n\n\\*literal\\* &amp; *style* ![plot](plot.png) $x^2$\n\n| Name | Value |\n| --- | --- |\n| zero | 0 |\n\n$$a=b$$\n", "a.md");
        assert!(p.blocks.iter().any(|b|matches!(&b.content,Content::Paragraph{text} if text.contains("*literal* & style"))));
        assert!(p.blocks.iter().any(|b|matches!(&b.content,Content::Image{url,alt} if url=="plot.png"&&alt=="plot")));
        assert!(p.blocks.iter().any(|b|matches!(&b.content,Content::Table{rows} if rows[1][1].text=="0"&&rows[0][0].header)));
        assert!(p.blocks.iter().any(|b|matches!(&b.content,Content::Math{text} if text=="$$a=b$$")));
    }
    #[test] fn source_lines_and_code_whitespace() {
        let p=parse("# T\r\n\r\n```rust\r\n  let x = 1;\r\n```\r\n","t.md");
        assert_eq!(p.blocks[1].content.text(),"  let x = 1;\n");
        assert_eq!(p.blocks[1].locator,Locator::Lines{start:3,end:5});
        assert!(parse("~~~\nbody\n","t").warnings.iter().any(|w|w.code=="unclosed_fence"));
    }
}
