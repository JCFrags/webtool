use anyhow::{bail,Context,Result};
use webtool_protocol::*;
use super::Parsed;

pub fn timestamp(s:&str)->Result<u64>{
    let s=s.trim().replace(',',".");let parts:Vec<&str>=s.split(':').collect();
    if parts.len()!=2&&parts.len()!=3{bail!("invalid caption timestamp: {s}");}
    let (hours,minutes,seconds)=if parts.len()==3{(parts[0].parse::<u64>()?,parts[1].parse::<u64>()?,parts[2])}
        else{(0,parts[0].parse::<u64>()?,parts[1])};
    let (seconds,fraction)=seconds.split_once('.').unwrap_or((seconds,""));
    let seconds=seconds.parse::<u64>()?;
    if minutes>=60||seconds>=60||fraction.len()>3||!fraction.bytes().all(|c|c.is_ascii_digit()){bail!("invalid caption timestamp");}
    let millis=if fraction.is_empty(){0}else{format!("{fraction:0<3}").parse::<u64>()?};
    hours.checked_mul(3600000).and_then(|v|v.checked_add(minutes*60000+seconds*1000+millis)).context("timestamp overflow")
}
pub fn parse(text:&str,name:&str)->Result<Parsed>{
    let normalized=text.trim_start_matches('\u{feff}').replace("\r\n","\n");
    let mut p=Parsed::new(name,"native-captions/2");
    for section in normalized.split("\n\n"){
        let lines:Vec<&str>=section.lines().collect();
        if lines.first().is_some_and(|l|l.starts_with("NOTE")||l.starts_with("STYLE")||l.starts_with("REGION")){continue;}
        let Some(index)=lines.iter().position(|l|l.contains("-->")) else {
            if super::extension(name)=="sbv"{
                if let Some((start,end))=lines.first().and_then(|s|s.split_once(',')){
                    let a=timestamp(start)?;let b=timestamp(end)?;
                    if b<a{bail!("caption ends before it starts");}
                    let cue=lines[1..].join("\n");
                    if cue.trim().is_empty(){bail!("caption cue has no text");}
                    p.push(Content::Caption{text:cue},Locator::Timestamp{start_ms:a,end_ms:b});
                }
            }
            continue;
        };
        let (start,end)=lines[index].split_once("-->").context("missing caption arrow")?;
        let end=end.split_whitespace().next().context("missing caption end")?;
        let a=timestamp(start)?;let b=timestamp(end)?;
        if b<a{bail!("caption ends before it starts");}
        let source=lines[index+1..].join("\n");
        if source.trim().is_empty(){bail!("caption cue has no text");}
        // Preserve inline speaker tags and positioning markup, rather than silently deleting them.
        let value=html_escape::decode_html_entities(&source).into_owned();
        p.push(Content::Caption{text:value},Locator::Timestamp{start_ms:a,end_ms:b});
    }
    if p.blocks.is_empty(){bail!("no timestamped captions found");}
    Ok(p)
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn timestamp_precision(){assert_eq!(timestamp("00:01:02.034").unwrap(),62034);assert_eq!(timestamp("01:02,5").unwrap(),62500);}
    #[test]fn timestamps_reject_bad_minutes(){assert!(timestamp("00:99:00").is_err());}
    #[test]fn parses_vtt(){let p=parse("WEBVTT\n\n00:00.000 --> 00:02.100 align:start\nHello\nworld\n","a.vtt").unwrap();assert_eq!(p.blocks.len(),1);assert_eq!(p.blocks[0].content.text(),"Hello\nworld");}
    #[test]fn parses_srt(){let p=parse("1\n00:00:01,000 --> 00:00:02,000\nTest\n","a.srt").unwrap();assert_eq!(p.blocks.len(),1);}
    #[test]fn parses_sbv(){assert_eq!(parse("0:00:01.000,0:00:02.000\nTest\n","a.sbv").unwrap().blocks.len(),1);}
    #[test]fn rejects_reversed_cue(){assert!(parse("00:05.000 --> 00:01.000\nno","a.vtt").is_err());}
}
