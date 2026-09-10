//! SQLite stays on the server. No client ever opens the database file.
use std::{path::{Path, PathBuf}, time::Duration};
use anyhow::{anyhow, bail, Context, Result};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use webtool_protocol::*;

pub const SCHEMA: &str = include_str!("../../../migrations/001_initial.sql");
#[derive(Clone)]
pub struct Store { root: PathBuf, database: PathBuf }

impl Store {
    pub async fn open(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        tokio::fs::create_dir_all(root.join("objects")).await?;
        let store = Self { database: root.join("webtool.sqlite3"), root };
        store.run(|c| {
            let version: u32 = c.pragma_query_value(None, "user_version", |r| r.get(0))?;
            if version > 1 { bail!("database schema {version} is newer than this application"); }
            c.pragma_update(None, "journal_mode", "WAL")?;
            let tx = c.transaction()?;
            tx.execute_batch(SCHEMA)?;
            tx.commit()?;
            Ok(())
        }).await?;
        Ok(store)
    }
    pub fn root(&self) -> &Path { &self.root }
    pub async fn run<T, F>(&self, f: F) -> Result<T>
    where T: Send + 'static, F: FnOnce(&mut Connection) -> Result<T> + Send + 'static {
        let path = self.database.clone();
        tokio::task::spawn_blocking(move || {
            let mut c = Connection::open(path)?;
            c.busy_timeout(Duration::from_secs(5))?;
            c.pragma_update(None, "foreign_keys", true)?;
            f(&mut c)
        }).await.context("database task failed")?
    }
    pub async fn put_bytes(&self, bytes: &[u8], media_type: &str, role: &str) -> Result<Artifact> {
        let sha256 = hex::encode(Sha256::digest(bytes));
        let dest = self.artifact_path(&sha256)?;
        if tokio::fs::metadata(&dest).await.is_err() {
            let temp = self.root.join("objects").join(format!(".{}.tmp", uuid::Uuid::new_v4()));
            let result = async {
                let mut file = tokio::fs::OpenOptions::new().write(true).create_new(true).open(&temp).await?;
                file.write_all(bytes).await?;
                file.sync_all().await?;
                drop(file);
                tokio::fs::rename(&temp, &dest).await?;
                Ok::<(), std::io::Error>(())
            }.await;
            if result.is_err() { let _ = tokio::fs::remove_file(&temp).await; }
            result?;
        }
        Ok(Artifact { sha256, media_type: media_type.into(), size: bytes.len() as u64, role: role.into() })
    }
    pub fn artifact_path(&self, hash: &str) -> Result<PathBuf> {
        if hash.len() != 64 || !hash.bytes().all(|c| c.is_ascii_hexdigit()) {
            bail!("invalid artifact identifier");
        }
        Ok(self.root.join("objects").join(hash))
    }
    pub async fn bytes(&self, artifact: &Artifact) -> Result<Vec<u8>> {
        let bytes = tokio::fs::read(self.artifact_path(&artifact.sha256)?).await?;
        if hex::encode(Sha256::digest(&bytes)) != artifact.sha256 { bail!("saved artifact checksum mismatch"); }
        Ok(bytes)
    }
    pub async fn save(&self, document: Document) -> Result<Document> {
        let id = document.id.clone();
        self.run(move |c| {
            c.execute("INSERT OR IGNORE INTO documents(id,title,source_url,retrieved_at,plain_text,payload) VALUES(?,?,?,?,?,?)",
                params![document.id,document.title,document.source.resolved,document.source.retrieved_at,
                    document.text(),serde_json::to_string(&document)?])?;
            Ok(())
        }).await?;
        self.document(&id).await
    }
    pub async fn document(&self, id: &str) -> Result<Document> {
        let id = id.to_owned();
        self.run(move |c| {
            let value: Option<String> = c.query_row("SELECT payload FROM documents WHERE id=?", [id], |r| r.get(0)).optional()?;
            serde_json::from_str(&value.ok_or_else(|| anyhow!("document not found"))?).map_err(Into::into)
        }).await
    }
    pub async fn cached(&self, key: &str, max_age: u64) -> Result<Option<Document>> {
        let key = key.to_owned();
        let cutoff = Utc::now().timestamp().saturating_sub(max_age.min(i64::MAX as u64) as i64);
        self.run(move |c| {
            let payload: Option<String> = c.query_row(
                "SELECT d.payload FROM fetch_cache f JOIN documents d ON d.id=f.document_id WHERE f.cache_key=? AND f.checked_at>=?",
                params![key,cutoff], |r| r.get(0)).optional()?;
            payload.map(|v| serde_json::from_str(&v).map_err(Into::into)).transpose()
        }).await
    }
    pub async fn cache(&self, key: String, id: String) -> Result<()> {
        self.run(move |c| {
            c.execute("INSERT INTO fetch_cache(cache_key,document_id,checked_at) VALUES(?,?,?) ON CONFLICT(cache_key) DO UPDATE SET document_id=excluded.document_id,checked_at=excluded.checked_at",
                params![key,id,Utc::now().timestamp()])?; Ok(())
        }).await
    }
    pub async fn create_library(&self, request: LibraryCreate) -> Result<Library> {
        validate_library(&request.name)?;
        let library = Library { name: request.name, description: request.description, created_at: Utc::now().to_rfc3339(), items: 0 };
        let l = library.clone();
        self.run(move |c| {
            c.execute("INSERT INTO libraries(name,description,created_at) VALUES(?,?,?)", params![l.name,l.description,l.created_at])?; Ok(())
        }).await?;
        Ok(library)
    }
    pub async fn libraries(&self) -> Result<Vec<Library>> {
        self.run(|c| {
            let mut q = c.prepare("SELECT l.name,l.description,l.created_at,COUNT(i.document_id) FROM libraries l LEFT JOIN library_items i ON l.name=i.library GROUP BY l.name ORDER BY l.name")?;
            let rows = q.query_map([], |r| Ok(Library { name:r.get(0)?,description:r.get(1)?,created_at:r.get(2)?,items:r.get(3)? }))?;
            Ok(rows.collect::<rusqlite::Result<_>>()?)
        }).await
    }
    pub async fn require_library(&self, name: &str) -> Result<()> {
        let name = name.to_owned();
        self.run(move |c| {
            let exists: bool = c.query_row("SELECT EXISTS(SELECT 1 FROM libraries WHERE name=?)", [name], |r| r.get(0))?;
            if !exists { bail!("library not found"); } Ok(())
        }).await
    }
    pub async fn add(&self, library: &str, document_id: &str, actor: Option<String>) -> Result<()> {
        let library = library.to_owned(); let document_id = document_id.to_owned();
        self.run(move |c| {
            c.execute("INSERT OR IGNORE INTO library_items(library,document_id,added_by,added_at) VALUES(?,?,?,?)",
                params![library,document_id,actor,Utc::now().to_rfc3339()])?; Ok(())
        }).await
    }
    pub async fn list_documents(&self, library: Option<String>, limit: usize) -> Result<Vec<DocumentSummary>> {
        self.run(move |c| {
            let mut stmt = c.prepare("SELECT d.payload FROM documents d WHERE (?1 IS NULL OR EXISTS(SELECT 1 FROM library_items i WHERE i.document_id=d.id AND i.library=?1)) ORDER BY d.retrieved_at DESC LIMIT ?2")?;
            let rows = stmt.query_map(params![library,limit.min(1000) as i64], |r| r.get::<_,String>(0))?;
            let mut out = Vec::new();
            for row in rows {
                let d: Document = serde_json::from_str(&row?)?;
                out.push(DocumentSummary { id:d.id,title:d.title,url:d.source.resolved,retrieved_at:d.source.retrieved_at,warnings:d.warnings.len() });
            }
            Ok(out)
        }).await
    }
    pub async fn search(&self, query: String, library: Option<String>, limit: usize) -> Result<Vec<SearchResult>> {
        // Quote tokens so a user's identifier is not silently treated as FTS syntax.
        let terms = query.split_whitespace().map(|t| format!("\"{}\"",t.replace('"',"\"\""))).collect::<Vec<_>>().join(" AND ");
        if terms.is_empty() { bail!("search query must not be empty"); }
        self.run(move |c| {
            let mut stmt = c.prepare("SELECT d.id,d.title,d.source_url,snippet(documents_fts,1,'','', ' ... ',40),bm25(documents_fts) FROM documents_fts JOIN documents d ON d.rowid=documents_fts.rowid WHERE documents_fts MATCH ?1 AND (?2 IS NULL OR EXISTS(SELECT 1 FROM library_items i WHERE i.document_id=d.id AND i.library=?2)) ORDER BY bm25(documents_fts) LIMIT ?3")?;
            let rows = stmt.query_map(params![terms,library,limit.min(100) as i64], |r| Ok(SearchResult {
                document_id:Some(r.get(0)?),title:r.get(1)?,url:r.get(2)?,snippet:r.get(3)?,score:-r.get::<_,f64>(4)?,providers:vec!["saved-library".into()],
            }))?;
            Ok(rows.collect::<rusqlite::Result<_>>()?)
        }).await
    }
    pub async fn add_annotation(&self, id: &str, request: AnnotationCreate) -> Result<Annotation> {
        if request.actor.trim().is_empty() { bail!("actor must not be empty"); }
        if request.note.len() > 65536 { bail!("note exceeds 65536 bytes"); }
        if request.tags.len() > 64 || request.tags.iter().any(|t| t.len() > 100) { bail!("too many or oversized tags"); }
        let a = Annotation { id:uuid::Uuid::new_v4().to_string(),document_id:id.into(),actor:request.actor,
            note:request.note,tags:request.tags,created_at:Utc::now().to_rfc3339() };
        let value = a.clone();
        self.run(move |c| {
            c.execute("INSERT INTO annotations(id,document_id,payload) VALUES(?,?,?)",params![value.id,value.document_id,serde_json::to_string(&value)?])?; Ok(())
        }).await?; Ok(a)
    }
    pub async fn annotations(&self, id: &str) -> Result<Vec<Annotation>> {
        let id=id.to_owned(); self.run(move |c| {
            let mut stmt=c.prepare("SELECT payload FROM annotations WHERE document_id=? ORDER BY rowid")?;
            let rows=stmt.query_map([id],|r|r.get::<_,String>(0))?;
            rows.map(|r|Ok(serde_json::from_str(&r?)?)).collect()
        }).await
    }
    pub async fn put_job(&self, job: Job) -> Result<()> {
        self.run(move |c| {
            let state=serde_json::to_value(&job.state)?.as_str().unwrap_or("failed").to_owned();
            c.execute("INSERT INTO jobs(id,state,created_at,updated_at,payload) VALUES(?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET state=excluded.state,updated_at=excluded.updated_at,payload=excluded.payload",
                params![job.id,state,job.created_at,job.updated_at,serde_json::to_string(&job)?])?; Ok(())
        }).await
    }
    pub async fn job(&self, id: &str) -> Result<Job> {
        let id=id.to_owned(); self.run(move |c| {
            let value:Option<String>=c.query_row("SELECT payload FROM jobs WHERE id=?",[id],|r|r.get(0)).optional()?;
            Ok(serde_json::from_str(&value.ok_or_else(||anyhow!("job not found"))?)?)
        }).await
    }
    pub async fn jobs(&self) -> Result<Vec<Job>> {
        self.run(|c| {
            let mut q=c.prepare("SELECT payload FROM jobs ORDER BY CASE WHEN state IN ('queued','running') THEN 0 ELSE 1 END, created_at DESC LIMIT 1000")?;
            let rows=q.query_map([],|r|r.get::<_,String>(0))?;
            rows.map(|r|Ok(serde_json::from_str(&r?)?)).collect()
        }).await
    }
}

pub fn validate_library(name:&str)->Result<()> {
    if name.is_empty() || name.len()>80 || !name.bytes().all(|c|c.is_ascii_alphanumeric()||b"-_".contains(&c)) {
        bail!("library names must contain 1 to 80 ASCII letters, digits, hyphens, or underscores");
    } Ok(())
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn library_names_are_url_safe() {
        for n in ["../bad","","a b","x/y"] { assert!(validate_library(n).is_err()); }
        assert!(validate_library("papers-2026").is_ok());
    }
    #[tokio::test] async fn schema_and_library_roundtrip() {
        let t=tempfile::tempdir().unwrap(); let s=Store::open(t.path()).await.unwrap();
        s.create_library(LibraryCreate{name:"papers".into(),description:"Research".into()}).await.unwrap();
        assert_eq!(s.libraries().await.unwrap()[0].name,"papers");
        assert!(s.require_library("missing").await.is_err());
    }
    #[tokio::test] async fn original_hash_roundtrip() {
        let t=tempfile::tempdir().unwrap(); let s=Store::open(t.path()).await.unwrap();
        let a=s.put_bytes(b"original", "text/plain", "upload").await.unwrap();
        assert_eq!(s.bytes(&a).await.unwrap(),b"original");
        assert!(s.artifact_path("../../secret").is_err());
    }
}
