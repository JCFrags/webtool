use std::{env,path::PathBuf,process::Command};
fn main(){
    let root=PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../..");
    let git=|args:&[&str]|Command::new("git").current_dir(&root).args(args).output().ok().filter(|o|o.status.success()).and_then(|o|String::from_utf8(o.stdout).ok()).map(|s|s.trim().to_owned());
    println!("cargo:rerun-if-changed=build.rs");
    for name in ["HEAD".to_owned(),"index".to_owned(),git(&["symbolic-ref","-q","HEAD"]).unwrap_or_default()]{
        if !name.is_empty(){if let Some(path)=git(&["rev-parse","--git-path",&name]){println!("cargo:rerun-if-changed={}",root.join(path).display());}}
    }
    // Also track source changes before commits; never label a dirty build clean.
    for path in ["crates","Cargo.toml","Cargo.lock"]{println!("cargo:rerun-if-changed={}",root.join(path).display());}
    if let Some(mut sha)=git(&["rev-parse","--verify","HEAD"]){
        if sha.len()==40 && sha.bytes().all(|b|b.is_ascii_hexdigit()){
            if git(&["diff","--quiet","HEAD","--","crates","Cargo.toml","Cargo.lock"]).is_none(){sha.push_str("-dirty");}
            println!("cargo:rustc-env=WEBTOOL_BUILD_COMMIT={sha}");
        }
    }
}
