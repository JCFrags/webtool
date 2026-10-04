//! Normalize the selected Xberg result. Originals and upstream records stay intact.
use std::collections::HashSet;
use serde_json::{json,Value};
use webtool_protocol::{Cell,Content,Link,Locator,Warning};
use super::{office,Parsed,positive_number};
fn compact(text:&str)->String{text.chars().filter(|c|!c.is_whitespace()).collect()}
fn nodes(payload:&Value)->Option<&Vec<Value>>{payload.pointer("/document/nodes")?.as_array()}
fn bbox(value:&Value)->Option<[f64;4]>{
    let result=[value.get("x0")?.as_f64()?,value.get("y0")?.as_f64()?,value.get("x1")?.as_f64()?,value.get("y1")?.as_f64()?];
    if result.iter().all(|n|n.is_finite())&&result[0]<=result[2]&&result[1]<=result[3]{Some(result)}else{None}
}
fn annotations(node:&Value,text:&str,p:&mut Parsed){
    let Some(values)=node.get("annotations").and_then(Value::as_array)else{return;};
    let id=p.blocks.last().unwrap().id.clone();let mut styles=Vec::new();let mut links=Vec::new();
    for value in values{
        let range=value.get("start").and_then(Value::as_u64).and_then(|v|usize::try_from(v).ok()).zip(value.get("end").and_then(Value::as_u64).and_then(|v|usize::try_from(v).ok()));
        let Some((start,end))=range.filter(|(a,b)|a<b&&text.get(*a..*b).is_some())else{
            p.warnings.push(Warning::new("document_annotation_unavailable","An upstream style/link range is not a valid UTF-8 range. It was not applied."));return;
        };
        let kind=value.pointer("/kind/annotation_type").and_then(Value::as_str).unwrap_or("");
        if kind=="link"{
            if let Some(url)=value.pointer("/kind/url").and_then(Value::as_str){links.push(json!({"start":start,"end":end,"url":url}));p.links.push(Link{url:url.into(),text:text[start..end].into()});}
        }else if matches!(kind,"bold"|"italic"|"code"|"underline"|"strikethrough"|"subscript"|"superscript"){
            styles.push(json!({"start":start,"end":end,"kind":kind}));
        }else{p.warnings.push(Warning::new("document_style_partial","An upstream color, highlight, font, or custom style is retained only in metadata."));}
    }
    links.sort_by_key(|value|value["start"].as_u64());
    if !styles.is_empty(){p.metadata["inline_styles"][&id]=json!(styles);}
    if !links.is_empty(){p.metadata["inline_links"][&id]=json!(links);}
}
fn grid(grid:&Value)->Option<Vec<Vec<Cell>>>{
    let rows=usize::try_from(grid.get("rows")?.as_u64()?).ok()?;let cols=usize::try_from(grid.get("cols")?.as_u64()?).ok()?;
    let count=rows.checked_mul(cols)?;if count==0||count>1_000_000{return None;}
    let cells=grid.get("cells")?.as_array()?;let mut occupied=vec![false;count];let mut result=vec![Vec::new();rows];
    let mut ordered=Vec::new();
    for cell in cells{
        let row=usize::try_from(cell.get("row")?.as_u64()?).ok()?;let col=usize::try_from(cell.get("col")?.as_u64()?).ok()?;
        let rs=usize::try_from(cell.get("row_span").map_or(Some(1),Value::as_u64)?).ok()?;let cs=usize::try_from(cell.get("col_span").map_or(Some(1),Value::as_u64)?).ok()?;
        if rs==0||cs==0||row.checked_add(rs)?>rows||col.checked_add(cs)?>cols{return None;}
        for r in row..row+rs{for c in col..col+cs{let slot=&mut occupied[r*cols+c];if *slot{return None;}*slot=true;}}
        ordered.push((row,col,Cell{text:cell.get("content")?.as_str()?.into(),row_span:rs,col_span:cs,header:cell.get("is_header").and_then(Value::as_bool).unwrap_or(false)}));
    }
    // Absent grid entries are not explicit empty source cells.
    if occupied.iter().any(|v|!*v){return None;}
    ordered.sort_by_key(|(row,col,_)|(*row,*col));for (row,_,cell) in ordered{result[row].push(cell);}Some(result)
}
fn candidate(p:&Parsed)->Parsed{Parsed::new(&p.title,&p.parser)}
fn accept(p:&mut Parsed,mut result:Parsed){
    p.blocks=result.blocks;p.links.extend(result.links);
    p.warnings.retain(|w|!matches!(w.code.as_str(),"document_tables_supplement"|"document_location_unavailable"|"document_structure_partial"));
    p.warnings.append(&mut result.warnings);
    if let Some(values)=result.metadata.as_object(){for (key,value) in values{p.metadata[key]=value.clone();}}
    p.metadata["supplemental_table_blocks"]=json!([]);
    p.warnings.push(Warning::new("document_structure_partial","Supplied text, structures, and selected inline styles are normalized. Drawing layout, downloadable figures, and exact DOCX XML positions are not implemented. Full upstream records and originals remain available."));
}
fn docx(payload:&Value,source:Option<&office::Source>,p:&mut Parsed)->bool{
    let Some(nodes)=nodes(payload)else{return false;};let mut result=candidate(p);
    // Xberg stores reading-order children. Traverse them once, without cycles or guessing parents.
    fn walk(index:usize,nodes:&[Value],source:Option<&office::Source>,result:&mut Parsed,seen:&mut HashSet<usize>,depth:usize)->Option<()> {
        if depth>64||!seen.insert(index){return None;}
        let node=nodes.get(index)?;let content=node.get("content")?;let kind=content.get("node_type")?.as_str()?;
        let text=content.get("text").and_then(Value::as_str).unwrap_or("");
        let mut list=None;
        let block=match kind{
            "title"=>Some(Content::Heading{level:1,text:text.into()}),
            "heading"=>Some(Content::Heading{level:content.get("level").and_then(Value::as_u64).filter(|v|(1..=6).contains(v)).unwrap_or(1) as u8,text:text.into()}),
            "paragraph"|"footnote"|"comment"|"citation"=>Some(Content::Paragraph{text:text.into()}),
            "list_item"=>{
                list=source.and_then(|s|s.list(text));
                if let Some(list)=list{Some(Content::ListItem{ordered:list.ordered,text:text.into()})}
                else{result.warnings.push(Warning::new("document_numbering_unavailable","A selected DOCX list item has no unique supported source numbering match. Its text is displayed without a guessed label."));Some(Content::Paragraph{text:text.into()})}
            },
            "table"=>Some(Content::Table{rows:grid(content.get("grid")?)?}),
            "code"=>Some(Content::Code{language:content.get("language").and_then(Value::as_str).map(str::to_owned),text:text.into()}),
            "formula"=>Some(Content::Math{text:text.into()}),
            "group"|"list"|"quote"|"page_break"=>None,
            // Do not append unhandled source material or silently omit its selected text.
            _=>return None,
        };
        if let Some(block)=block{
            result.push(block,Locator::Derived{index:result.blocks.len()+1});let id=result.blocks.last().unwrap().id.clone();
            result.metadata["document_nodes"][&id]=json!({"id":node.get("id"),"page":node.get("page"),"content_layer":node.get("content_layer"),"exact_position":false});
            if let Some(list)=list{
                result.metadata["list_items"][&id]=json!({"depth":list.depth,"ordinal":list.ordinal,"first":true});
                result.metadata["office_list_sources"][&id]=json!({"part":list.part,"paragraph":list.paragraph});
            }
            annotations(node,text,result);
            if matches!(kind,"footnote"|"comment"){result.metadata["block_roles"][&id]=json!(kind);}
        }
        if let Some(children)=node.get("children").and_then(Value::as_array){for child in children{
            let child=usize::try_from(child.as_u64()?).ok()?;
            if nodes.get(child)?.get("parent").and_then(Value::as_u64)!=Some(index as u64){return None;}
            walk(child,nodes,source,result,seen,depth+1)?;
        }}Some(())
    }
    let mut seen=HashSet::new();
    for (index,node) in nodes.iter().enumerate(){if node.get("parent").is_none()&&walk(index,nodes,source,&mut result,&mut seen,0).is_none(){return false;}}
    if seen.len()!=nodes.len()||result.blocks.is_empty(){return false;}
    if result.blocks.iter().any(|b|matches!(b.content,Content::Table{..})){result.warnings.push(Warning::new("document_table_roles_upstream","Table spans and header roles are supplied by Xberg. Header-role accuracy was not independently established."));}
    accept(p,result);true
}
fn pptx(payload:&Value,source:Option<&office::Source>,p:&mut Parsed)->bool{
    let Some(pages)=payload.get("pages").and_then(Value::as_array)else{return false;};let mut result=candidate(p);
    let upstream_notes=pages.iter().filter_map(|page|page.get("speaker_notes").and_then(Value::as_str)).filter(|note|!note.trim().is_empty()).collect::<Vec<_>>();
    let mut notes_used=HashSet::new();
    for page in pages{
        let Some(text)=page.get("content").and_then(Value::as_str).filter(|v|!v.trim().is_empty())else{continue;};
        let slide=source.and_then(|s|s.slide(text));
        if slide.is_none(){result.warnings.push(Warning::new("document_slide_location_unavailable","The source relationship order did not uniquely corroborate a complete extracted slide. Its blocks use derived positions."));}
        for paragraph in text.split("\n\n").filter(|v|!v.trim().is_empty()){
            let locator=slide.map_or(Locator::Derived{index:result.blocks.len()+1},|s|Locator::Slide{number:s.number});
            result.push(Content::Paragraph{text:paragraph.into()},locator);let id=result.blocks.last().unwrap().id.clone();
            if let Some(slide)=slide{result.metadata["office_slide_sources"][&id]=json!({"part":slide.part,"number":slide.number,"corroboration":"complete_slide_text_and_internal_relationship"});}
            if let Some(nodes)=nodes(payload){
                let matches=nodes.iter().filter(|node|node.get("page")==page.get("page_number")&&node.pointer("/content/text").and_then(Value::as_str)==Some(paragraph)).collect::<Vec<_>>();
                if matches.len()==1{annotations(matches[0],paragraph,&mut result);}
            }
        }
        if let Some(slide)=slide{
            if let Some(note)=&slide.note{
                let matches=upstream_notes.iter().enumerate().filter(|(_,text)|compact(text)==note.text_key).collect::<Vec<_>>();
                let owners=source.unwrap().slides.iter().filter(|s|s.note.as_ref().is_some_and(|n|n.text_key==note.text_key)).count();
                if matches.len()==1&&owners==1&&!note.text_key.is_empty(){
                    let (index,text)=matches[0];notes_used.insert(index);
                    result.push(Content::Paragraph{text:(*text).to_owned()},Locator::Slide{number:slide.number});let id=result.blocks.last().unwrap().id.clone();
                    result.metadata["block_roles"][&id]=json!("speaker_notes");
                    let reported=pages.iter().filter(|page|page.get("speaker_notes").and_then(Value::as_str)==Some(*text)).filter_map(|page|positive_number(page.get("page_number"))).collect::<Vec<_>>();
                    result.metadata["office_note_sources"][&id]=json!({"part":note.part,"relationship_id":note.relationship_id,"slide_part":slide.part,"number":slide.number,"upstream_reported_pages":reported});
                    if reported.as_slice()!=[slide.number]{result.warnings.push(Warning::new("document_notes_source_correction","An upstream note/slide association differs from its retained internal relationship. The source-confirmed slide is used. The upstream record remains unchanged."));}
                }else{result.warnings.push(Warning::new("document_notes_unavailable","A source notes relationship has no unique matching upstream note. Notes are not inferred from source XML."));}
            }
        }
    }
    for (index,text) in upstream_notes.iter().enumerate(){if !notes_used.contains(&index){
        result.push(Content::Paragraph{text:(*text).to_owned()},Locator::Derived{index:result.blocks.len()+1});let id=&result.blocks.last().unwrap().id;
        result.metadata["block_roles"][id]=json!("speaker_notes");
        result.warnings.push(Warning::new("document_notes_location_unavailable","An extracted speaker note has no unique internal source relationship match. Its slide number is not inferred."));
    }}
    // Keep any supplied tables as explicit supplements. Their layout order is not inferred.
    if let Some(tables)=payload.get("tables").and_then(Value::as_array){for table in tables{
        let Some(rows)=plain_table(table)else{return false;};result.push(Content::Table{rows},Locator::Derived{index:result.blocks.len()+1});
        // The block IDs are collected after all supplied tables are appended.
    }}
    if result.blocks.is_empty(){return false;}
    let supplements=result.blocks.iter().filter(|b|matches!(b.content,Content::Table{..})).map(|b|b.id.clone()).collect::<Vec<_>>();
    if !supplements.is_empty(){result.warnings.push(Warning::new("document_tables_supplement","Supplied presentation tables supplement slide text. Their exact position is unavailable."));}
    accept(p,result);p.metadata["supplemental_table_blocks"]=json!(supplements);true
}
fn plain_table(table:&Value)->Option<Vec<Vec<Cell>>>{table.get("cells")?.as_array()?.iter().map(|row|row.as_array()?.iter().map(|cell|Some(Cell{text:cell.as_str()?.into(),row_span:1,col_span:1,header:false})).collect()).collect()}
fn xlsx(payload:&Value,source:Option<&office::Source>,p:&mut Parsed)->bool{
    let Some(pages)=payload.get("pages").and_then(Value::as_array)else{return false;};let Some(nodes)=nodes(payload)else{return false;};let mut result=candidate(p);
    let mut tables_used=HashSet::new();
    for page in pages{
        let Some(name)=page.get("sheet_name").and_then(Value::as_str)else{return false;};
        if pages.iter().filter(|p|p.get("sheet_name").and_then(Value::as_str)==Some(name)).count()!=1{return false;}
        let hidden=source.and_then(|s|s.sheets.iter().find(|(n,_)|n==name).map(|(_,hidden)|*hidden));
        let label=if hidden==Some(true){format!("{name} (hidden)")}else{name.into()};
        result.push(Content::Heading{level:2,text:label},Locator::Sheet{name:name.into()});
        let Some(tables)=page.get("tables").and_then(Value::as_array)else{return false;};
        for table in tables{
            let Some(supplied)=plain_table(table)else{return false;};
            let matches=nodes.iter().enumerate().filter(|(_,n)|n.get("page")==page.get("page_number")&&n.pointer("/content/node_type").and_then(Value::as_str)==Some("table")).filter_map(|(index,node)|{
                let rows=grid(node.pointer("/content/grid")?)?;
                if rows.len()==supplied.len()&&rows.iter().zip(&supplied).all(|(a,b)|a.len()==b.len()&&a.iter().zip(b).all(|(a,b)|a.text==b.text)){Some((index,node,rows))}else{None}
            }).collect::<Vec<_>>();
            if matches.len()!=1{return false;}
            let (index,node,rows)=&matches[0];if !tables_used.insert(*index){return false;}
            result.push(Content::Table{rows:rows.clone()},Locator::Sheet{name:name.into()});let id=&result.blocks.last().unwrap().id;
            result.metadata["document_nodes"][id]=json!({"id":node.get("id"),"sheet":name,"grid_origin":"upstream","cell_origin":"sheet_scope_not_exact_cell_addresses"});
        }
        if tables.is_empty(){return false;}
    }
    if tables_used.len()!=nodes.iter().filter(|n|n.pointer("/content/node_type").and_then(Value::as_str)==Some("table")).count(){return false;}
    result.warnings.push(Warning::new("spreadsheet_values_not_recalculated","Spreadsheet cells retain supplied values. Formula caches may be stale or absent. No formula or external workbook was calculated or fetched. Table header roles are upstream estimates."));
    accept(p,result);true
}
fn pdf(payload:&Value,p:&mut Parsed){
    let mut matched=0;
    if let Some(nodes)=nodes(payload){for block in &mut p.blocks{
        let Locator::Page{number,bbox:ref mut location}=&mut block.locator else{continue;};
        let text=compact(&block.content.text());
        let matches=nodes.iter().filter(|n|positive_number(n.get("page"))==Some(*number)&&n.pointer("/content/text").and_then(Value::as_str).is_some_and(|t|compact(t)==text)).filter_map(|n|bbox(n.get("bbox")?)).collect::<Vec<_>>();
        if matches.len()==1{*location=Some(matches[0]);matched+=1;}
    }}
    p.metadata["pdf_geometry"]=json!({"matched_whole_page_blocks":matched,"reading_order":"upstream_page_text_unchanged","geometry_origin":"upstream_only"});
    if p.blocks.iter().any(|b|matches!(b.content,Content::Table{..})){
        p.warnings.push(Warning::new("pdf_tables_heuristic","Native PDF tables are Xberg layout estimates. Aligned prose can be identified as a table. Page text remains in upstream reading order and tables are labeled supplements."));
    }
}
pub fn apply(payload:&Value,format:&str,source:Option<&office::Source>,p:&mut Parsed){
    if payload.get("mime_type").and_then(Value::as_str)==Some("application/pdf"){pdf(payload,p);return;}
    let supported=match format{"docx"=>docx(payload,source,p),"pptx"=>pptx(payload,source,p),"xlsx"=>xlsx(payload,source,p),_=>return};
    if !supported{p.warnings.push(Warning::new("document_structure_unavailable","The supplied structure is missing, ambiguous, or outside this normalizer's scope. Supplied page text and table supplements remain visible instead."));}
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn sparse_grid_is_not_an_empty_source_cell(){
        assert!(grid(&json!({"rows":1,"cols":2,"cells":[{"row":0,"col":0,"content":"0"}]})).is_none());
        let rows=grid(&json!({"rows":1,"cols":2,"cells":[{"row":0,"col":0,"content":"0"},{"row":0,"col":1,"content":""}]})).unwrap();
        assert_eq!(rows[0][0].text,"0");assert_eq!(rows[0][1].text,"");
    }
    #[test]fn pdf_geometry_is_only_a_unique_complete_match(){
        let payload=json!({"mime_type":"application/pdf","document":{"nodes":[{"page":2,"bbox":{"x0":1.,"y0":2.,"x1":3.,"y1":4.},"content":{"node_type":"paragraph","text":"Exact text"}}]}});
        let mut parsed=Parsed::new("PDF","xberg");parsed.push(Content::Paragraph{text:"Exact text".into()},Locator::Page{number:2,bbox:None});pdf(&payload,&mut parsed);
        assert_eq!(parsed.blocks[0].locator,Locator::Page{number:2,bbox:Some([1.,2.,3.,4.])});
    }
}
