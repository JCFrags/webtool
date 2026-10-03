//! Service-owned UUID staging. Cleanup refuses ambiguous ownership or live writers.
use std::{path::{Path, PathBuf}, time::Duration};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

pub const META_BYTES: u64 = 4 * 1024 * 1024;
#[derive(Clone, Serialize, Deserialize)]
struct Process { pid: u32, start: u64 }
#[derive(Clone, Serialize, Deserialize)]
struct Owner { version: u32, id: String, server: Process, child: Option<Process> }
pub struct Workspace { pub path: PathBuf, owner: Owner }

#[cfg(target_os = "linux")]
fn process(pid: u32) -> Option<(u64, u32, bool)> {
    let text = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let fields: Vec<_> = text.rsplit_once(')')?.1.split_whitespace().collect();
    Some((fields.get(19)?.parse().ok()?, fields.get(2)?.parse().ok()?, matches!(*fields.first()?, "Z" | "X")))
}
#[cfg(not(target_os = "linux"))]
fn process(_: u32) -> Option<(u64, u32, bool)> { None }
fn alive(p: &Process) -> bool { process(p.pid).is_some_and(|(start, _, dead)| start == p.start && !dead) }
fn group_live(pid: u32) -> Result<bool> {
    for entry in std::fs::read_dir("/proc")? {
        if let Some(pid2) = entry?.file_name().to_str().and_then(|s|s.parse().ok()) {
            if process(pid2).is_some_and(|(_, group, dead)| group == pid && !dead) { return Ok(true); }
        }
    }
    Ok(false)
}
fn directory(path: &Path) -> Result<()> {
    let m = std::fs::symlink_metadata(path)?;
    if !m.is_dir() || m.file_type().is_symlink() { bail!("media_staging_unsafe: staging is not a regular owned directory"); }
    #[cfg(unix)] {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if m.uid() != unsafe { nix::libc::geteuid() } || m.permissions().mode() & 0o077 != 0 {
            bail!("media_staging_unsafe: staging ownership or mode changed");
        }
    }
    Ok(())
}
fn root(base: &Path) -> Result<PathBuf> {
    let root = base.join("media-staging");
    if !root.exists() {
        std::fs::create_dir(&root)?;
        #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))?; }
    }
    directory(&root)?;
    Ok(root)
}
impl Workspace {
    pub fn create(base: &Path, id: &str) -> Result<Self> {
        if uuid::Uuid::parse_str(id).is_err() { bail!("media_staging_unsafe: invalid staging identity"); }
        let path = root(base)?.join(id);
        std::fs::create_dir(&path)?;
        #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))?; }
        let pid = std::process::id();
        let start = process(pid).context("media_platform_unsupported: Linux process identity is required")?.0;
        let value = Self { path, owner: Owner { version:1, id:id.into(), server:Process { pid, start }, child:None } };
        value.save()?;
        Ok(value)
    }
    fn save(&self) -> Result<()> {
        let temp = self.path.join("owner.new");
        let mut options = std::fs::OpenOptions::new(); options.write(true).create_new(true);
        #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; options.mode(0o600).custom_flags(nix::libc::O_NOFOLLOW); }
        use std::io::Write;
        let mut file = options.open(&temp)?;
        file.write_all(&serde_json::to_vec(&self.owner)?)?; file.sync_all()?;
        std::fs::rename(temp, self.path.join("owner.json"))?;
        Ok(())
    }
    pub fn child(&mut self, pid: u32) -> Result<()> {
        self.owner.child = Some(Process { pid, start:process(pid).context("media_helper_failed: child identity unavailable")?.0 });
        self.save()
    }
    pub fn regular(&self, name: &str) -> Result<PathBuf> {
        if name.is_empty() || name.len()>128 || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)) || name.starts_with('.') {
            bail!("media_staging_unsafe: unexpected output name");
        }
        let path = self.path.join(name); let m = std::fs::symlink_metadata(&path)?;
        if !m.is_file() || m.file_type().is_symlink() || m.len() == 0 { bail!("media_validation_failed: missing regular output"); }
        Ok(path)
    }
    pub fn bytes(&self) -> Result<u64> { directory_bytes(&self.path, 32) }
    fn verify(&self) -> Result<()> {
        directory(&self.path)?;
        let m = std::fs::symlink_metadata(self.path.join("owner.json"))?;
        if !m.is_file() || m.file_type().is_symlink() || m.len()>4096 { bail!("media_staging_unsafe: invalid owner receipt"); }
        let owner: Owner = serde_json::from_slice(&std::fs::read(self.path.join("owner.json"))?)?;
        if owner.version != 1 || owner.id != self.owner.id || owner.server.start != self.owner.server.start || owner.server.pid != self.owner.server.pid {
            bail!("media_staging_unsafe: owner receipt changed");
        }
        self.bytes()?;
        Ok(())
    }
    pub fn clean(self) -> Result<()> {
        self.verify()?;
        if self.owner.child.as_ref().is_some_and(|p| group_live(p.pid).unwrap_or(true)) {
            bail!("media_staging_unsafe: helper descendants have not stopped");
        }
        std::fs::remove_dir_all(self.path)?;
        Ok(())
    }
}
pub fn directory_bytes(path: &Path, limit: usize) -> Result<u64> {
    let mut total = 0u64; let mut count = 0;
    for entry in std::fs::read_dir(path)? {
        let entry=entry?; let m=std::fs::symlink_metadata(entry.path())?; count+=1;
        if count>limit || !m.is_file() || m.file_type().is_symlink() { bail!("media_staging_unsafe: file count, type, or path limit reached"); }
        total=total.checked_add(m.len()).context("media_budget_exceeded: size overflow")?;
    }
    Ok(total)
}
/// Retained/abandoned staging also consumes allocation. Active reservations may
/// count these bytes twice. Conservative rejection is safer than uncounted files.
pub fn staging_bytes(base: &Path) -> Result<u64> {
    let root=root(base)?; let mut total=0u64;
    for e in std::fs::read_dir(root)? {
        let e=e?; directory(&e.path())?;
        total=total.checked_add(directory_bytes(&e.path(),65536)?).context("media_budget_exceeded: staging size overflow")?;
    }
    Ok(total)
}
pub async fn recover(base: &Path, id: &str) -> Result<()> {
    let path=root(base)?.join(id);
    if !path.exists() { return Ok(()); }
    directory(&path)?;
    let marker=path.join("owner.json"); let m=std::fs::symlink_metadata(&marker)?;
    if !m.is_file() || m.file_type().is_symlink() || m.len()>4096 { bail!("media_staging_unsafe: invalid recovery receipt"); }
    let owner:Owner=serde_json::from_slice(&std::fs::read(marker)?)?;
    if owner.id!=id || owner.version!=1 || uuid::Uuid::parse_str(id).is_err() || alive(&owner.server) {
        bail!("media_staging_unsafe: recovery owner is invalid or still running");
    }
    if let Some(child)=&owner.child {
        if group_live(child.pid)? {
            // A PID alone cannot authorize a kill after reuse or a lost leader.
            if !alive(child) { bail!("media_staging_unsafe: descendant group identity is ambiguous; staging retained"); }
            kill_group(child.pid);
            for _ in 0..40 { if !group_live(child.pid)? { break; } tokio::time::sleep(Duration::from_millis(50)).await; }
        }
    }
    Workspace {path,owner}.clean()
}
pub fn kill_group(pid: u32) {
    #[cfg(unix)] { let _=nix::sys::signal::killpg(nix::unistd::Pid::from_raw(pid as i32),nix::sys::signal::Signal::SIGKILL); }
}
pub async fn stopped(pid: u32) -> Result<()> {
    for _ in 0..40 { if !group_live(pid)? { return Ok(()); } tokio::time::sleep(Duration::from_millis(50)).await; }
    bail!("media_helper_failed: helper descendants have not stopped")
}
pub fn free_bytes(path: &Path) -> Result<u64> {
    #[cfg(unix)] {
        use std::os::unix::ffi::OsStrExt;
        let path=std::ffi::CString::new(path.as_os_str().as_bytes())?;
        let mut value=std::mem::MaybeUninit::<nix::libc::statvfs>::uninit();
        if unsafe { nix::libc::statvfs(path.as_ptr(),value.as_mut_ptr()) } != 0 { return Err(std::io::Error::last_os_error().into()); }
        let value=unsafe { value.assume_init() };
        return (value.f_bavail as u64).checked_mul(value.f_frsize as u64).context("media_budget_exceeded: free-space overflow");
    }
    #[cfg(not(unix))] { let _=path; bail!("media_platform_unsupported: filesystem accounting unavailable") }
}
