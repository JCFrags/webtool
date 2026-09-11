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

pub fn supplemental_table(d:&Document, id:&str)->bool {
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
/// Conservative 88-column ASCII grid. Other cells use explicit row/cell labels
/// so terminal character widths, tabs, line breaks and spans cannot misalign data.
fn plain_table(rows:&[Vec<crate::Cell>])->String{
    if rows.is_empty(){return "(no rows)\n".into();}
    let columns=rows[0].len();
    let header=columns>0 && rows[0].iter().all(|c|c.header);
    let aligned=columns>0 && rows.iter().enumerate().all(|(i,row)|row.len()==columns && row.iter().all(|c|
        c.row_span==1 && c.col_span==1 && c.text.is_ascii() && !c.text.chars().any(char::is_control)
        && c.header==(i==0 && header)));
    if aligned{
        let widths:Vec<usize>=(0..columns).map(|i|rows.iter().map(|row|row[i].text.len()).max().unwrap_or(0)).collect();
        if widths.iter().sum::<usize>()+3*columns+1<=88{
            let border=format!("+{}+\n",widths.iter().map(|w|"-".repeat(w+2)).collect::<Vec<_>>().join("+"));
            let mut out=String::new();
            if header{out.push_str("First row: source header cells\n");}
            out.push_str(&border);
            for row in rows{
                out.push('|');
                for (cell,width) in row.iter().zip(&widths){out.push_str(&format!(" {:width$} |",cell.text,width=width));}
                out.push('\n');out.push_str(&border);
            }
            return out;
        }
    }
    let mut out=String::from("Labeled cells (no inferred column alignment):\n");
    for (r,row) in rows.iter().enumerate(){
        out.push_str(&format!("Row {} ({} cells)\n",r+1,row.len()));
        for (c,cell) in row.iter().enumerate(){
            out.push_str(&format!("Cell {} [header={}, rowspan={}, colspan={}]{}\n",c+1,cell.header,cell.row_span,cell.col_span,if cell.text.is_empty(){" (empty)"}else{""}));
            if !cell.text.is_empty(){out.push_str(&cell.text);if !cell.text.ends_with('\n'){out.push('\n');}}
            out.push_str("End cell\n");
        }
    }
    out
}

pub fn plain_block(b:&crate::Block,supplemental:bool)->String{
    let loc=format!("{} | {}",b.id,location(&b.locator));
    let mut out=String::new();
    match &b.content{
        Content::Heading{level,text}=>out.push_str(&format!("{} {text}\n[{loc}]\n\n","#".repeat((*level).clamp(1,6) as usize))),
        Content::Code{language,text}=>{
            out.push_str(&format!("Code{} [{loc}]\n",language.as_ref().map(|s|format!(" ({s})")).unwrap_or_default()));
            let fence="`".repeat((text.split(|c|c!='`').map(str::len).max().unwrap_or(0)+1).max(3));
            out.push_str(&fence);out.push('\n');out.push_str(text);
            if !text.ends_with('\n'){out.push('\n');}
            out.push_str(&fence);out.push_str("\n\n");
        },
        Content::Table{rows}=>{
            if supplemental{out.push_str("Supplemental structured table (may repeat page text):\n");}
            out.push_str(&format!("Table [{loc}]\n"));out.push_str(&plain_table(rows));out.push('\n');
        },
        Content::Caption{text}=>out.push_str(&format!("Caption [{loc}]\n{text}\n\n")),
        Content::Image{url,alt}=>out.push_str(&format!("Image [{loc}]: {alt}\n{url}\n\n")),
        Content::Quote{text}=>out.push_str(&format!("Quote [{loc}]\n{text}\n\n")),
        Content::Math{text}=>out.push_str(&format!("Math [{loc}]\n{text}\n\n")),
        Content::ListItem{text,ordered}=>out.push_str(&format!("{} {text}\n",if *ordered{"Ordered item:"}else{"-"})),
        Content::Paragraph{text}=>{
            if matches!(b.locator,Locator::Page{..}|Locator::Slide{..}|Locator::Sheet{..}){out.push_str(&format!("[{loc}]\n"));}
            out.push_str(text);out.push_str("\n\n");
        },
    }
    terminal_safe(&out)
}

pub fn plain(d:&Document)->String{
    let mut out=format!("{}\nSource: {}\nRetrieved: {}\nID: {}\n\n",d.title,d.source.resolved,d.source.retrieved_at,d.id);
    for b in &d.blocks{out.push_str(&plain_block(b,supplemental_table(d,&b.id)));}
    if d.metadata.pointer("/github/kind").and_then(|v|v.as_str())==Some("readme") && !d.links.is_empty() {
        out.push_str("Pinned README links (source text above is unchanged):\n");
        for link in &d.links { out.push_str(&format!("{}: {}\n",link.text,link.url)); }
    }
    terminal_safe(&out)
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn terminal_escape_not_executed(){assert_eq!(terminal_safe("a\u{1b}[2Jb"),"a\\u{1b}[2Jb");}
    #[test]fn tabs_and_newlines_preserved(){assert_eq!(terminal_safe("a\tb\n"),"a\tb\n");}
    #[test]fn timestamps_are_exact(){assert_eq!(time(3723004),"01:02:03.004");}
}
