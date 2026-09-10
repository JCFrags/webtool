//! Human-facing summaries only. Machine response values bypass this module.
use anyhow::{Context,Result};
use webtool_protocol::*;

pub fn documents(items:&[DocumentSummary])->String{
    if items.is_empty(){return "No saved documents.\n".into();}
    let mut out=String::new();
    for d in items{
        out.push_str(&format!("{}\n  ID: {}\n",d.title,d.id));
        if !d.url.is_empty(){out.push_str(&format!("  Source: {}\n",d.url));}
        if !d.retrieved_at.is_empty(){out.push_str(&format!("  Retrieved: {}\n",d.retrieved_at));}
        out.push_str(&format!("  Warnings: {}\n\n",d.warnings));
    }
    render::terminal_safe(&out)
}

pub fn libraries(items:&[Library])->String{
    if items.is_empty(){return "No libraries.\n".into();}
    let mut out=String::new();
    for l in items{
        out.push_str(&format!("{} | {} items\n",l.name,l.items));
        if !l.description.is_empty(){out.push_str(&format!("  {}\n",l.description));}
        if !l.created_at.is_empty(){out.push_str(&format!("  Created: {}\n",l.created_at));}
        out.push('\n');
    }
    render::terminal_safe(&out)
}

pub fn job(j:&Job)->String{
    let mut out=format!("Job {} | {:?}\nSource: {}\nVisited attempts: {} | Saved: {} | Failed: {}\nLimits: {} pages, depth {}\n",
        j.id,j.state,j.request.url,j.visited,j.document_ids.len(),j.failed,j.request.max_pages,j.request.max_depth);
    if let Some(l)=&j.request.library{out.push_str(&format!("Library: {l}\n"));}
    if !j.created_at.is_empty(){out.push_str(&format!("Created: {}\n",j.created_at));}
    if !j.updated_at.is_empty(){out.push_str(&format!("Updated: {}\n",j.updated_at));}
    out.push_str(&format!("Warnings: {} (details on stderr)\n",j.warnings.len()));
    if !j.document_ids.is_empty(){out.push_str("Saved IDs:\n");for id in &j.document_ids{out.push_str(&format!("  {id}\n"));}}
    if let Some(error)=&j.error{out.push_str(&format!("Error: {error}\n"));}
    out.push('\n');
    render::terminal_safe(&out)
}

pub fn extract(result:&ExtractResponse,kind:&ExtractKind,document:&Document)->Result<Option<String>>{
    let mut out=format!("Document: {}\n",result.document_id);
    match kind{
        ExtractKind::Code|ExtractKind::Tables|ExtractKind::Outline=>{
            let blocks:Vec<Block>=serde_json::from_value(result.data.clone()).context("unexpected extracted block response")?;
            if blocks.is_empty(){out.push_str("No matching blocks.\n");}
            for block in &blocks{
                // Outlines need each heading's retained location, including HTML.
                if matches!(kind,ExtractKind::Outline){
                    if let Content::Heading{level,text}=&block.content{
                        out.push_str(&format!("Heading {level}: {text}\n  {} | {}\n\n",block.id,render::location(&block.locator)));
                    }
                }else{out.push_str(&render::plain_block(block,render::supplemental_table(document,&block.id)));}
            }
        },
        ExtractKind::Links=>{
            let links:Vec<Link>=serde_json::from_value(result.data.clone()).context("unexpected extracted links response")?;
            out.push_str("Links (individual source positions are not retained):\n");
            if links.is_empty(){out.push_str("No links.\n");}
            for link in &links{
                if !link.text.is_empty(){out.push_str(&format!("{}\n",link.text));}
                out.push_str(&format!("{}\n\n",link.url));
            }
        },
        _=>return Ok(None),
    }
    Ok(Some(render::terminal_safe(&out)))
}
