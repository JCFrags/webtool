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
/// Markdown for interactive reads. The export renderer above remains stable and complete.
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
    for (index,block) in d.blocks.iter().enumerate(){
        if !details && index==0 && matches!(&block.content,Content::Heading{text,..} if text.trim()==d.title.trim()){continue;}
        let group=source_group(&block.locator);
        if !details && group!=previous_group{
            if let Some(label)=&group{out.push_str(&format!("**{label}**\n\n"));}
        }
        if group.is_some(){previous_group=group;}
        let is_list=matches!(block.content,Content::ListItem{..});
        if list_open&&!is_list{out.push('\n');}
        list_open=is_list;
        if details{out.push_str(&format!("*{} | {}*\n\n",block.id,location(&block.locator)));}
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
                if supplemental_table(d,&block.id){out.push_str("Supplemental structured table (may repeat page text):\n\n");}
                markdown_table(&mut out,rows);
            },
            Content::Image{url,alt}=>{
                if !alt.is_empty(){if details{out.push_str(&format!("![{alt}]({url})\n\n"));}else{out.push_str(&format!("Image: {alt}\n\n"));}}
            },
            Content::Math{text}=>out.push_str(&format!("{}\n\n",display_math(text))),
            Content::Caption{text}=>out.push_str(&format!("[{}] {text}\n\n",caption_location(&block.locator))),
        }
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

pub fn plain(d:&Document)->String{
    let mut out=format!("{}\nSource: {}\nSaved ID: {}\n\n",d.title,d.source.resolved,d.id);
    let mut previous_group:Option<String>=None;
    let mut list_open=false;
    for (index,block) in d.blocks.iter().enumerate(){
        if index==0 && matches!(&block.content,Content::Heading{text,..} if text.trim()==d.title.trim()){continue;}
        let group=source_group(&block.locator);
        if group!=previous_group{if let Some(label)=&group{out.push_str(&format!("{label}\n\n"));}}
        if group.is_some(){previous_group=group;}
        let is_list=matches!(block.content,Content::ListItem{..});
        if list_open&&!is_list{out.push('\n');}
        list_open=is_list;
        out.push_str(&clean_plain_block(block,supplemental_table(d,&block.id)));
    }
    if list_open{out.push('\n');}
    terminal_safe(&out)
}

pub fn plain_details(d:&Document)->String{
    let mut out=format!("{}\nSource: {}\nRetrieved: {}\nStatus: {}\nParser: {}\nExtraction: {}\nOriginal: {} bytes, {}, {}, SHA-256 {}\nSaved ID: {}\n\n",
        d.title,d.source.resolved,d.source.retrieved_at,d.source.status.map(|v|v.to_string()).unwrap_or_else(||"unknown".into()),d.parser,d.extraction_version,
        d.source.original.size,d.source.original.media_type,d.source.original.role,d.source.original.sha256,d.id);
    for block in &d.blocks{out.push_str(&plain_block(block,supplemental_table(d,&block.id)));}
    if !d.links.is_empty(){out.push_str("Links (individual source positions are not retained):\n");for link in &d.links{out.push_str(&format!("{}: {}\n",link.text,link.url));}}
    terminal_safe(&out)
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn terminal_escape_not_executed(){assert_eq!(terminal_safe("a\u{1b}[2Jb"),"a\\u{1b}[2Jb");}
    #[test]fn tabs_and_newlines_preserved(){assert_eq!(terminal_safe("a\tb\n"),"a\tb\n");}
    #[test]fn timestamps_are_exact(){assert_eq!(time(3723004),"01:02:03.004");}
}
