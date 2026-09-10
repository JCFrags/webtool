//! Client-only settings. Never loads engine configuration or opens server storage.
use std::{env,fs,io::Write,path::{Path,PathBuf}};
use anyhow::{bail,Context,Result};

const DEFAULT_SERVER:&str="http://127.0.0.1:8420";

pub fn path()->Result<PathBuf>{
    let root=env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).filter(|p|p.is_absolute());
    let root=match root{Some(p)=>p,None=>{
        let home=env::var_os("HOME").map(PathBuf::from).filter(|p|p.is_absolute())
            .context("set an absolute XDG_CONFIG_HOME or HOME for client settings")?;
        home.join(".config")
    }};
    Ok(root.join("webtool/client.toml"))
}

pub fn validate_endpoint(value:&str)->Result<String>{
    if value.trim()!=value || value.chars().any(char::is_control){bail!("server URL must not contain surrounding whitespace or control characters");}
    let url=url::Url::parse(value).context("invalid server URL; use http://HOST:PORT or https://HOST")?;
    if !matches!(url.scheme(),"http"|"https") || url.host_str().is_none(){bail!("server URL must be an absolute HTTP or HTTPS URL with a host");}
    if !url.username().is_empty() || url.password().is_some() || url.query().is_some() || url.fragment().is_some(){
        bail!("server URL must not contain credentials, a query or a fragment");
    }
    Ok(url.to_string().trim_end_matches('/').to_owned())
}

fn load(path:&Path)->Result<toml::Table>{
    match fs::read_to_string(path){
        Ok(s)=>toml::from_str(&s).with_context(||format!("parse client settings {}; correct malformed TOML before updating",path.display())),
        Err(e) if e.kind()==std::io::ErrorKind::NotFound=>Ok(toml::Table::new()),
        Err(e)=>Err(e).with_context(||format!("read client settings {}",path.display())),
    }
}

pub fn effective(explicit:Option<&str>,path:&Path)->Result<(String,&'static str)>{
    if let Some(value)=explicit{return Ok((validate_endpoint(value)?,"--server"));}
    if let Some(value)=env::var_os("WEBTOOL_SERVER"){
        return Ok((validate_endpoint(value.to_str().context("WEBTOOL_SERVER must be UTF-8")?)?,"WEBTOOL_SERVER"));
    }
    let settings=load(path)?;
    if let Some(value)=settings.get("server"){
        return Ok((validate_endpoint(value.as_str().context("client setting server must be a string")?)?,"saved client settings"));
    }
    Ok((DEFAULT_SERVER.into(),"localhost default"))
}

pub fn save(path:&Path,value:&str)->Result<String>{
    let server=validate_endpoint(value)?;
    if fs::symlink_metadata(path).is_ok_and(|m|m.file_type().is_symlink()){
        bail!("refusing to replace symlink {}; select its intended configuration directory explicitly",path.display());
    }
    let mut settings=load(path)?;
    settings.insert("server".into(),toml::Value::String(server.clone()));
    let parent=path.parent().context("client configuration has no parent directory")?;
    fs::create_dir_all(parent).with_context(||format!("create {}",parent.display()))?;
    let mut temp=tempfile::NamedTempFile::new_in(parent)?;
    if let Ok(metadata)=fs::metadata(path){temp.as_file().set_permissions(metadata.permissions())?;}
    temp.write_all(toml::to_string_pretty(&settings)?.as_bytes())?;
    temp.as_file().sync_all()?;
    temp.persist(path).with_context(||format!("atomically replace {}",path.display()))?;
    #[cfg(unix)]
    fs::File::open(parent)?.sync_all()?;
    Ok(server)
}
