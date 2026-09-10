//! Plain output and Markdown share the same retained document blocks.
use crate::{Content,Document,Locator};

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

fn supplemental_table(d:&Document, id:&str)->bool {
    d.metadata.get("supplemental_table_blocks").and_then(|v|v.as_array())
        .is_some_and(|items|items.iter().any(|v|v.as_str()==Some(id)))
}

pub fn markdown(d:&Document)->String{
    let mut out=format!("# {}\n\nSource: {}\nRetrieved: {}\nDocument: {}\n\n",d.title,d.source.resolved,d.source.retrieved_at,d.id);
    for block in &d.blocks{
        if matches!(block.content,Content::Table{..}) && supplemental_table(d,&block.id) {
            out.push_str("Supplemental structured table (may repeat page text):\n\n");
        }
        match &block.content{
            Content::Heading{level,text}=>out.push_str(&format!("{} {text}\n\n","#".repeat((*level).clamp(1,6) as usize))),
            Content::Paragraph{text}=>out.push_str(&format!("{text}\n\n")),
            Content::Code{language,text}=>{
                let max_run=text.split(|c|c!='`').map(str::len).max().unwrap_or(0);
                let fence="`".repeat((max_run+1).max(3));
                let lang=language.as_deref().unwrap_or("").chars().filter(|c|c.is_ascii_alphanumeric()||"_+-#".contains(*c)).collect::<String>();
                out.push_str(&format!("{fence}{lang}\n{text}{}{}\n\n",if text.ends_with('\n'){""}else{"\n"},fence));
            },
            Content::Quote{text}=>{for line in text.lines(){out.push_str(&format!("> {line}\n"));}out.push('\n');},
            Content::ListItem{text,ordered}=>out.push_str(&format!("{} {text}\n",if *ordered{"1."}else{"-"})),
            Content::Table{rows}=>{
                if rows.is_empty(){continue;}
                let width=rows[0].len();
                let rectangular=rows.iter().all(|r|r.len()==width&&r.iter().all(|c|c.col_span==1&&c.row_span==1));
                let header=rows[0].iter().all(|c|c.header);
                if rectangular&&header{
                    for (i,row) in rows.iter().enumerate(){
                        out.push_str(&format!("| {} |\n",row.iter().map(|c|md_cell(&c.text)).collect::<Vec<_>>().join(" | ")));
                        if i==0{out.push_str(&format!("| {} |\n",vec!["---";width].join(" | ")));}
                    }
                }else{
                    // HTML retains absent headers, merged cells, and ragged rows without inventing cells.
                    out.push_str("<table>\n");for row in rows{out.push_str("<tr>");for c in row{
                        let tag=if c.header{"th"}else{"td"};out.push_str(&format!("<{tag} rowspan=\"{}\" colspan=\"{}\">{}</{tag}>",c.row_span,c.col_span,html(&c.text)));
                    }out.push_str("</tr>\n");}out.push_str("</table>\n");
                }out.push('\n');
            },
            Content::Image{url,alt}=>out.push_str(&format!("Image: {alt}\n{url}\n\n")),
            Content::Math{text}=>out.push_str(&format!("{text}\n\n")),
            Content::Caption{text}=>out.push_str(&format!("[{}] {text}\n\n",location(&block.locator))),
        }
    }
    if !d.links.is_empty(){out.push_str("## Links\n\n");for l in &d.links{out.push_str(&format!("- {}: {}\n",l.text,l.url));}out.push('\n');}
    if !d.warnings.is_empty(){out.push_str("## Extraction warnings\n\n");for w in &d.warnings{out.push_str(&format!("- {}: {}\n",w.code,w.message));}}
    out
}
/// Escape terminal control sequences only in the terminal view. JSON and original exports remain lossless.
pub fn terminal_safe(s:&str)->String{
    s.chars().map(|c|if (c.is_control()&&c!='\n'&&c!='\t')||('\u{80}'..='\u{9f}').contains(&c){c.escape_unicode().to_string()}else{c.to_string()}).collect()
}
pub fn plain(d:&Document)->String{
    let mut out=format!("{}\n{}\nSaved {}\nID {}\n\n",d.title,d.source.resolved,d.source.retrieved_at,d.id);
    for b in &d.blocks{
        if matches!(b.content,Content::Table{..}) && supplemental_table(d,&b.id) {
            out.push_str("Supplemental structured table (may repeat page text):\n");
        }
        if matches!(b.locator,Locator::Page{..}|Locator::Slide{..}|Locator::Sheet{..}) {
            out.push_str(&format!("[{} | {}]\n",b.id,location(&b.locator)));
        }
        match &b.content{
            Content::Heading{text,..}=>out.push_str(&format!("{text}\n\n")),
            Content::Caption{text}=>out.push_str(&format!("[{}] {text}\n\n",location(&b.locator))),
            Content::Image{url,alt}=>out.push_str(&format!("Image: {alt}\n{url}\n\n")),
            _=>out.push_str(&format!("{}\n\n",b.content.text())),
        }
    }
    terminal_safe(&out)
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn terminal_escape_not_executed(){assert_eq!(terminal_safe("a\u{1b}[2Jb"),"a\\u{1b}[2Jb");}
    #[test]fn tabs_and_newlines_preserved(){assert_eq!(terminal_safe("a\tb\n"),"a\tb\n");}
    #[test]fn timestamps_are_exact(){assert_eq!(time(3723004),"01:02:03.004");}
}
