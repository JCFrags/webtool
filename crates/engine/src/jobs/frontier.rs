//! One bounded frontier per job. Checkpoints commit admissions, attempts,
//! attachments, and the public job record in the same SQLite transaction.
use std::collections::{HashMap, HashSet};
use anyhow::{bail, Context, Result};
use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use webtool_protocol::{CrawlProgress, Job, Warning};
use crate::store::{Store, write_job};
use crate::error::ErrorKind;

pub(super) const PAGE_CANDIDATES: usize = 10_000;
pub(super) const SITEMAP_LIMIT: usize = 32;
pub(super) const SITEMAP_DEPTH: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Kind { Page, Sitemap }
impl Kind {
    fn text(self) -> &'static str { match self { Self::Page => "page", Self::Sitemap => "sitemap" } }
    fn parse(s: &str) -> Result<Self> { match s { "page" => Ok(Self::Page), "sitemap" => Ok(Self::Sitemap), _ => bail!(ErrorKind::StorageFault.context("unknown crawl entry kind")) } }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum State { Pending, Active, Saved, Failed, Excluded, Interrupted }
impl State {
    fn text(self) -> &'static str { match self { Self::Pending => "pending", Self::Active => "active", Self::Saved => "saved", Self::Failed => "failed", Self::Excluded => "excluded", Self::Interrupted => "interrupted" } }
    fn parse(s: &str) -> Result<Self> { match s { "pending" => Ok(Self::Pending), "active" => Ok(Self::Active), "saved" => Ok(Self::Saved), "failed" => Ok(Self::Failed), "excluded" => Ok(Self::Excluded), "interrupted" => Ok(Self::Interrupted), _ => bail!(ErrorKind::StorageFault.context("unknown crawl entry state")) } }
}
#[derive(Clone, Debug)]
pub(super) struct Entry {
    pub kind: Kind,
    pub url: String,
    pub depth: usize,
    pub state: State,
    pub attempts: usize,
    pub interruptions: usize,
    pub document_id: Option<String>,
    pub reason: Option<String>,
}
#[derive(Default)]
pub(super) struct Frontier {
    pub entries: Vec<Entry>,
    seen: HashMap<(Kind, String), usize>,
    dirty: HashSet<usize>,
    pub sitemaps_seeded: bool,
}
impl Frontier {
    pub async fn load(store: &Store, id: &str) -> Result<Option<Self>> {
        let id = id.to_owned();
        store.run(move |c| {
            let seeded: Option<bool> = c.query_row("SELECT sitemaps_seeded FROM crawl_runs WHERE job_id=?", [&id], |r| r.get(0)).optional()?;
            let Some(sitemaps_seeded) = seeded else { return Ok(None); };
            let mut q = c.prepare("SELECT ordinal,kind,url,depth,state,attempts,interruptions,document_id,reason FROM crawl_frontier WHERE job_id=? ORDER BY ordinal")?;
            let rows = q.query_map([&id], |r| Ok((r.get::<_,usize>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,usize>(3)?,r.get::<_,String>(4)?,r.get::<_,usize>(5)?,r.get::<_,usize>(6)?,r.get::<_,Option<String>>(7)?,r.get::<_,Option<String>>(8)?)))?;
            let mut frontier = Self { sitemaps_seeded, ..Self::default() };
            for row in rows {
                let (ordinal,kind,url,depth,state,attempts,interruptions,document_id,reason) = row?;
                if ordinal != frontier.entries.len() { bail!(ErrorKind::StorageFault.context("invalid crawl frontier order")); }
                let kind = Kind::parse(&kind)?;
                frontier.seen.insert((kind,url.clone()),ordinal);
                frontier.entries.push(Entry { kind,url,depth,state:State::parse(&state)?,attempts,interruptions,document_id,reason });
            }
            if frontier.entries.iter().filter(|e|e.kind==Kind::Page).count()>PAGE_CANDIDATES || frontier.entries.iter().filter(|e|e.kind==Kind::Sitemap).count()>SITEMAP_LIMIT { bail!(ErrorKind::StorageFault.context("crawl frontier exceeds supported bounds")); }
            Ok(Some(frontier))
        }).await
    }
    pub fn contains(&self, kind: Kind, url: &str) -> bool { self.seen.contains_key(&(kind,url.to_owned())) }
    /// Excluded candidates also occupy admission capacity. A URL's first
    /// admission fixes its depth; breadth-first scheduling keeps that depth minimal.
    pub fn admit(&mut self, kind: Kind, url: String, depth: usize, reason: Option<String>) -> bool {
        if self.contains(kind,&url) { return true; }
        let limit = if kind==Kind::Page { PAGE_CANDIDATES } else { SITEMAP_LIMIT };
        if self.entries.iter().filter(|e|e.kind==kind).count()>=limit { return false; }
        let i = self.entries.len();
        self.seen.insert((kind,url.clone()),i);
        self.entries.push(Entry { kind,url,depth,state:if reason.is_some(){State::Excluded}else{State::Pending},attempts:0,interruptions:0,document_id:None,reason });
        self.dirty.insert(i);
        true
    }
    pub fn next(&self, kind: Kind, depth: Option<usize>) -> Option<usize> {
        self.entries.iter().enumerate().filter(|(_,e)|e.kind==kind && e.state==State::Pending && depth.is_none_or(|d|d==e.depth))
            .min_by_key(|(i,e)|(e.depth,*i)).map(|(i,_)|i)
    }
    pub fn change(&mut self, i: usize, state: State, reason: Option<String>) {
        let entry = &mut self.entries[i];
        if state==State::Active { entry.attempts+=1; }
        if state==State::Interrupted && entry.state==State::Active { entry.interruptions+=1; }
        entry.state=state; entry.reason=reason;
        self.dirty.insert(i);
    }
    pub fn interrupt_active(&mut self) {
        for i in 0..self.entries.len() {
            if self.entries[i].state==State::Active { self.change(i,State::Interrupted,Some("Attempt interrupted; its budget charge is retained.".into())); }
        }
    }
    pub fn requeue_interrupted(&mut self) {
        for i in 0..self.entries.len() {
            if self.entries[i].state==State::Interrupted { self.change(i,State::Pending,None); }
        }
    }
    pub fn progress(&self) -> CrawlProgress {
        let mut p = CrawlProgress::default();
        for e in &self.entries {
            if e.kind==Kind::Page {
                p.candidates+=1; p.attempted+=e.attempts; p.interrupted+=e.interruptions;
                p.pending+=usize::from(e.state==State::Pending); p.active+=usize::from(e.state==State::Active); p.excluded+=usize::from(e.state==State::Excluded);
            } else {
                p.sitemaps+=1; p.sitemap_attempted+=e.attempts; p.sitemap_pending+=usize::from(e.state==State::Pending); p.sitemap_active+=usize::from(e.state==State::Active); p.sitemap_interrupted+=e.interruptions;
            }
        }
        p
    }
    pub async fn checkpoint(&mut self, store: &Store, job: &mut Job, attachment: Option<String>) -> Result<()> {
        job.progress=Some(self.progress());
        job.visited=self.entries.iter().filter(|e|e.kind==Kind::Page && matches!(e.state,State::Saved|State::Failed)).count();
        job.failed=self.entries.iter().filter(|e|e.kind==Kind::Page && e.state==State::Failed).count();
        job.updated_at=Utc::now().to_rfc3339();
        if let Some(id)=&attachment { if !job.document_ids.contains(id) { job.document_ids.push(id.clone()); } }
        let value=job.clone(); let seeded=self.sitemaps_seeded;
        let entries:Vec<_>=self.dirty.iter().map(|i|(*i,self.entries[*i].clone())).collect();
        store.run(move |c| {
            let tx=c.transaction()?;
            write_job(&tx,&value)?;
            tx.execute("INSERT INTO crawl_runs(job_id,sitemaps_seeded) VALUES(?,?) ON CONFLICT(job_id) DO UPDATE SET sitemaps_seeded=excluded.sitemaps_seeded",params![value.id,seeded])?;
            for (i,e) in entries {
                tx.execute("INSERT INTO crawl_frontier(job_id,ordinal,kind,url,depth,state,attempts,interruptions,document_id,reason) VALUES(?,?,?,?,?,?,?,?,?,?) ON CONFLICT(job_id,kind,url) DO UPDATE SET state=excluded.state,attempts=excluded.attempts,interruptions=excluded.interruptions,document_id=excluded.document_id,reason=excluded.reason",
                    params![value.id,i,e.kind.text(),e.url,e.depth,e.state.text(),e.attempts,e.interruptions,e.document_id,e.reason])?;
            }
            let crawl=value.request.crawl().context(ErrorKind::StorageFault.context("crawl checkpoint has no crawl request"))?;
            if let (Some(library),Some(id))=(&crawl.library,attachment) {
                tx.execute("INSERT OR IGNORE INTO library_items(library,document_id,added_by,added_at) VALUES(?,?,?,?)",params![library,id,crawl.actor,value.updated_at])?;
            }
            tx.commit().context(ErrorKind::StorageFault.context("commit crawl checkpoint"))?;
            Ok(())
        }).await?;
        self.dirty.clear();
        Ok(())
    }
}

pub(super) fn warn(job: &mut Job, code: &str, message: impl Into<String>) {
    // The frontier retains each exclusion/failure reason even after this summary cap.
    if job.warnings.len()<256 { job.warnings.push(Warning::new(code,message.into())); }
    else if !job.warnings.iter().any(|w|w.code=="crawl_warning_limit") { job.warnings.push(Warning::new("crawl_warning_limit","Additional warnings were omitted after 256 entries. Frontier states and counts remain saved.")); }
}
pub(super) fn warn_once(job: &mut Job, code: &str, message: impl Into<String>) {
    if !job.warnings.iter().any(|w|w.code==code) { warn(job,code,message); }
}
