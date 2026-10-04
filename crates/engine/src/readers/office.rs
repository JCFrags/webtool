//! Corroboration from retained OOXML, not another content extractor.
//! Only internal XML members are read. No paths are extracted and no URLs are fetched.
use std::{collections::{HashMap,HashSet}, io::{Cursor,Read}};
use anyhow::{bail,Context,Result};
use roxmltree::{Document,Node};
use serde_json::{json,Value};
use webtool_protocol::Warning;

const W:&str="http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const A:&str="http://schemas.openxmlformats.org/drawingml/2006/main";
const P:&str="http://schemas.openxmlformats.org/presentationml/2006/main";
const S:&str="http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const R:&str="http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const REL:&str="http://schemas.openxmlformats.org/package/2006/relationships";
fn tag(node:Node<'_,'_>,ns:&str,name:&str)->bool{node.has_tag_name((ns,name))}
fn compact(text:&str)->String{text.chars().filter(|c|!c.is_whitespace()).collect()}
fn member_target(source:&str,target:&str)->Result<String>{
    if target.is_empty()||target.contains(['\\',':','?','#','%'])||target.chars().any(char::is_control){bail!("unsupported internal relationship target");}
    let mut parts=if target.starts_with('/') {Vec::new()} else {source.split('/').collect::<Vec<_>>()};
    if !target.starts_with('/') {parts.pop();}
    for part in target.split('/') {match part {""|"."=>{},".."=>{if parts.pop().is_none(){bail!("relationship escapes package root");}},part=>parts.push(part)}}
    if parts.is_empty(){bail!("relationship has no member target");}Ok(parts.join("/"))
}
fn rels_path(part:&str)->String{
    match part.rsplit_once('/') {Some((dir,file))=>format!("{dir}/_rels/{file}.rels"),None=>format!("_rels/{part}.rels")}
}
struct Package<'a>{zip:zip::ZipArchive<Cursor<&'a [u8]>>,remaining:usize,member_limit:usize,cache:HashMap<String,String>}
impl<'a> Package<'a>{
    fn new(bytes:&'a [u8],max_bytes:usize,limits:&xberg::extractors::security::SecurityLimits)->Result<Self>{
        if bytes.len()>max_bytes {bail!("OOXML source exceeds the application byte limit");}
        let mut zip=zip::ZipArchive::new(Cursor::new(bytes))?;
        if zip.len()>limits.max_files_in_archive{bail!("OOXML member count exceeds document limits");}
        let mut names=HashSet::new();let mut total=0u64;
        for i in 0..zip.len(){
            let file=zip.by_index(i)?;
            if !names.insert(file.name().to_owned()){bail!("duplicate OOXML archive member");}
            total=total.checked_add(file.size()).context("OOXML size overflow")?;
            if total>limits.max_archive_size as u64{bail!("OOXML archive exceeds document limits");}
            if file.size()>0 && file.size() as f64/(file.compressed_size().max(1) as f64)>limits.max_compression_ratio as f64{bail!("OOXML compression ratio exceeds document limits");}
        }
        // Never enlarge the application's existing decoded/source budget.
        let cap=max_bytes.min(super::super::encoding::DEFAULT_LIMIT).min(100*1024*1024).min(limits.max_content_size).min(limits.max_archive_size);
        Ok(Self{zip,remaining:cap,member_limit:cap,cache:HashMap::new()})
    }
    fn read(&mut self,path:&str)->Result<String>{
        if let Some(value)=self.cache.get(path){return Ok(value.clone());}
        let file=self.zip.by_name(path)?;
        let limit=self.member_limit.min(self.remaining);
        if file.size()>limit as u64{bail!("OOXML XML member exceeds remaining decoded budget");}
        let mut bytes=Vec::new();file.take(limit as u64+1).read_to_end(&mut bytes)?;
        if bytes.len()>limit{bail!("OOXML XML member exceeds remaining decoded budget");}
        self.remaining-=bytes.len();let value=String::from_utf8(bytes).context("OOXML XML member is not UTF-8")?;
        self.cache.insert(path.into(),value.clone());Ok(value)
    }
    fn has(&self,path:&str)->bool{self.zip.file_names().any(|name|name==path)}
    fn relationships(&mut self,part:&str,kind:&str)->Result<HashMap<String,String>>{
        let path=if part.is_empty(){"_rels/.rels".into()}else{rels_path(part)};
        if !self.has(&path){return Ok(HashMap::new());}
        let xml=self.read(&path)?;let doc=Document::parse(&xml)?;let mut records=HashMap::new();
        for rel in doc.descendants().filter(|n|tag(*n,REL,"Relationship")){
            if rel.attribute("Type")!=Some(format!("{R}/{kind}").as_str()){continue;}
            if rel.attribute("TargetMode").is_some_and(|v|v!="Internal"){bail!("requested OOXML relationship is external");}
            let id=rel.attribute("Id").context("relationship has no ID")?;
            let target=member_target(part,rel.attribute("Target").context("relationship has no target")?)?;
            if records.insert(id.into(),target).is_some(){bail!("ambiguous OOXML relationship ID");}
        }
        Ok(records)
    }
}
#[derive(Debug,Clone)]
pub struct ListSource{pub depth:usize,pub ordinal:Option<i64>,pub ordered:bool,pub part:String,pub paragraph:usize}
#[derive(Debug)]
pub struct SlideSource{pub number:usize,pub part:String,pub text_key:String,pub note:Option<NoteSource>}
#[derive(Debug)]
pub struct NoteSource{pub part:String,pub relationship_id:String,pub text_key:String}
#[derive(Default,Debug)]
pub struct Source{pub list_items:HashMap<String,Vec<ListSource>>,pub slides:Vec<SlideSource>,pub sheets:Vec<(String,bool)>,pub formulas:Vec<Value>,pub warnings:Vec<Warning>}
impl Source{
    pub fn read(bytes:&[u8],format:&str,max_bytes:usize,limits:&xberg::extractors::security::SecurityLimits)->Result<Self>{
        let mut package=Package::new(bytes,max_bytes,limits)?;
        let main=package.relationships("","officeDocument")?;
        if main.len()!=1{bail!("OOXML main part is missing or ambiguous");}
        let part=main.values().next().unwrap();
        match format {"docx"=>docx(&mut package,part),"pptx"=>pptx(&mut package,part),"xlsx"=>xlsx(&mut package,part),_=>Ok(Self::default())}
    }
    pub fn list(&self,text:&str)->Option<&ListSource>{self.list_items.get(&compact(text)).filter(|items|items.len()==1).map(|items|&items[0])}
    pub fn slide(&self,text:&str)->Option<&SlideSource>{let key=compact(text);let mut found=self.slides.iter().filter(|s|s.text_key==key);let result=found.next()?;if found.next().is_some(){None}else{Some(result)}}
}
fn unique<'a,'input>(mut nodes:impl Iterator<Item=Node<'a,'input>>)->Option<Node<'a,'input>>{let first=nodes.next()?;if nodes.next().is_none(){Some(first)}else{None}}
fn value<'a,'input>(node:Node<'a,'input>,name:&str)->Option<&'a str>{unique(node.children().filter(|n|tag(*n,W,name)))?.attribute((W,"val"))}
fn docx(package:&mut Package<'_>,part:&str)->Result<Source>{
    let xml=package.read(part)?;let doc=Document::parse(&xml)?;let mut source=Source::default();
    let numbered=doc.descendants().filter(|n|tag(*n,W,"p")).any(|p|p.children().find(|n|tag(*n,W,"pPr")).is_some_and(|props|props.children().any(|n|tag(n,W,"numPr"))));
    if !numbered{return Ok(source);}
    let relationships=package.relationships(part,"numbering")?;
    if relationships.len()!=1{bail!("DOCX numbering part is missing or ambiguous");}
    let nums=package.read(relationships.values().next().unwrap())?;let numbering=Document::parse(&nums)?;
    let mut counters:HashMap<(String,usize),i64>=HashMap::new();
    for (position,p) in doc.descendants().filter(|n|tag(*n,W,"p")).enumerate(){
        let Some(props)=p.children().find(|n|tag(*n,W,"pPr")).and_then(|p|p.children().find(|n|tag(*n,W,"numPr")))else{continue;};
        let Some(id)=value(props,"numId").filter(|id|*id!="0")else{continue;};
        let Some(depth)=value(props,"ilvl").and_then(|v|v.parse::<usize>().ok()).filter(|n|*n<=32)else{source.warnings.push(Warning::new("document_numbering_unavailable","A DOCX list has no supported explicit level. Its source numbering is not inferred."));continue;};
        let matches=numbering.descendants().filter(|n|tag(*n,W,"num")&&n.attribute((W,"numId"))==Some(id)).collect::<Vec<_>>();
        if matches.len()!=1{source.warnings.push(Warning::new("document_numbering_unavailable","A DOCX numbering ID is missing or ambiguous."));continue;}
        let num=matches[0];let abstract_id=value(num,"abstractNumId");
        let abstract_nums=numbering.descendants().filter(|n|tag(*n,W,"abstractNum")&&n.attribute((W,"abstractNumId"))==abstract_id).collect::<Vec<_>>();
        let depth_string=depth.to_string();
        let base=if abstract_nums.len()==1{unique(abstract_nums[0].children().filter(|n|tag(*n,W,"lvl")&&n.attribute((W,"ilvl"))==Some(depth_string.as_str())))}else{None};
        let overrides=num.children().filter(|n|tag(*n,W,"lvlOverride")&&n.attribute((W,"ilvl"))==Some(depth_string.as_str())).collect::<Vec<_>>();
        if overrides.len()>1{source.warnings.push(Warning::new("document_numbering_unavailable","A DOCX numbering override is ambiguous."));continue;}
        let override_node=overrides.first().copied();
        let level=override_node.and_then(|n|unique(n.children().filter(|n|tag(*n,W,"lvl")))).or(base);
        let Some(level)=level else{source.warnings.push(Warning::new("document_numbering_unavailable","A DOCX numbering level is unavailable."));continue;};
        let format=value(level,"numFmt");
        if !matches!(format,Some("decimal"|"bullet")){source.warnings.push(Warning::new("document_numbering_unavailable","Nondecimal DOCX numbering is retained in the original, not converted to guessed decimal labels."));continue;}
        let ordered=format==Some("decimal");
        let start=override_node.and_then(|n|value(n,"startOverride")).or_else(||value(level,"start")).and_then(|v|v.parse::<i64>().ok());
        let key=(id.to_owned(),depth);
        let ordinal=if ordered{start.map(|start|*counters.entry(key.clone()).or_insert(start))}else{None};
        if ordered && (ordinal.is_none()||value(level,"lvlText")!=Some(format!("%{}.",depth+1).as_str())){source.warnings.push(Warning::new("document_numbering_unavailable","A DOCX ordinal or label pattern is not supported. See retained numbering XML."));continue;}
        if let Some(number)=ordinal{if let Some(next)=number.checked_add(1){counters.insert(key,next);}else{bail!("DOCX numbering overflow");}}
        counters.retain(|(number,level),_|number!=id||*level<=depth);
        let text=p.descendants().filter(|n|tag(*n,W,"t")).filter_map(|n|n.text()).collect::<String>();
        source.list_items.entry(compact(&text)).or_default().push(ListSource{depth,ordinal,ordered,part:part.into(),paragraph:position+1});
    }
    Ok(source)
}
fn drawing_key(doc:&Document<'_>)->String{doc.descendants().filter(|n|tag(*n,A,"t")).filter_map(|n|n.text()).flat_map(str::chars).filter(|c|!c.is_whitespace()).collect()}
fn pptx(package:&mut Package<'_>,part:&str)->Result<Source>{
    let xml=package.read(part)?;let doc=Document::parse(&xml)?;let rels=package.relationships(part,"slide")?;let mut source=Source::default();
    let mut seen=HashSet::new();
    for (index,slide) in doc.descendants().filter(|n|tag(*n,P,"sldId")).enumerate(){
        let id=slide.attribute((R,"id")).context("PPTX slide has no relationship ID")?;
        let target=rels.get(id).context("PPTX slide relationship is unavailable")?;
        if !seen.insert(target){bail!("PPTX slide target is ambiguous");}
        let xml=package.read(target)?;let slide_doc=Document::parse(&xml)?;
        let notes=package.relationships(target,"notesSlide")?;
        if notes.len()>1{bail!("PPTX notes relationship is ambiguous");}
        let note=if let Some((id,note_part))=notes.iter().next(){
            let xml=package.read(note_part)?;let notes_doc=Document::parse(&xml)?;
            // Ignore date/page-number placeholders. Only the source body supplies note text.
            let mut text_key=String::new();
            for shape in notes_doc.descendants().filter(|n|tag(*n,P,"sp")){
                if !shape.descendants().any(|n|tag(n,P,"ph")&&n.attribute("type")==Some("body")){continue;}
                for text in shape.descendants().filter(|n|tag(*n,A,"t")).filter_map(|n|n.text()){text_key.push_str(&compact(text));}
            }
            Some(NoteSource{part:note_part.clone(),relationship_id:id.clone(),text_key})
        }else{None};
        source.slides.push(SlideSource{number:index+1,part:target.clone(),text_key:drawing_key(&slide_doc),note});
    }
    Ok(source)
}
fn xlsx(package:&mut Package<'_>,part:&str)->Result<Source>{
    let xml=package.read(part)?;let doc=Document::parse(&xml)?;let rels=package.relationships(part,"worksheet")?;let mut source=Source::default();let mut seen=HashSet::new();
    for sheet in doc.descendants().filter(|n|tag(*n,S,"sheet")){
        let name=sheet.attribute("name").context("XLSX sheet has no name")?;
        if !seen.insert(name){bail!("XLSX sheet name is ambiguous");}
        let id=sheet.attribute((R,"id")).context("XLSX sheet has no relationship ID")?;let target=rels.get(id).context("XLSX sheet relationship is unavailable")?;
        source.sheets.push((name.into(),sheet.attribute("state").is_some_and(|s|matches!(s,"hidden"|"veryHidden"))));
        let xml=package.read(target)?;let sheet_doc=Document::parse(&xml)?;
        for cell in sheet_doc.descendants().filter(|n|tag(*n,S,"c")){
            let Some(formula)=cell.children().find(|n|tag(*n,S,"f"))else{continue;};
            let cached=cell.children().find(|n|tag(*n,S,"v"));
            source.formulas.push(json!({"sheet":name,"cell":cell.attribute("r"),"part":target,"formula":formula.text().unwrap_or(""),"cached_value_present":cached.is_some(),"cached_value":cached.map(|v|v.text().unwrap_or("")),"recalculated":false}));
            if cached.is_none(){source.warnings.push(Warning::new("spreadsheet_formula_cache_missing","A source formula has no cached value. No formula was calculated."));}
        }
    }
    Ok(source)
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn targets_stay_inside_the_archive(){
        assert_eq!(member_target("ppt/slides/slide2.xml","../notesSlides/notesSlide1.xml").unwrap(),"ppt/notesSlides/notesSlide1.xml");
        for value in ["../../../../etc/passwd","https://example.test/x","../x.xml#frag","\\host\\x","%2e%2e/x"]{assert!(member_target("ppt/slides/s.xml",value).is_err());}
    }
}
