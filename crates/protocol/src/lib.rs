//! Versioned data shared by the lightweight client and the application server.
//! All source locations describe a retained artifact, not an inferred live page.
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const API_VERSION: &str = "v1";
pub const EXTRACTION_VERSION: &str = "webtool-0.1.0-schema1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Locator {
    Html { selector: String },
    Lines { start: usize, end: usize },
    Timestamp { start_ms: u64, end_ms: u64 },
    Page { number: usize, bbox: Option<[f64; 4]> },
    Cell { sheet: String, row: usize, column: usize },
    Sheet { name: String },
    Slide { number: usize },
    JsonPointer { pointer: String },
    /// No exact original location was supplied by this parser.
    Derived { index: usize },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Cell {
    pub text: String,
    #[serde(default = "one")]
    pub row_span: usize,
    #[serde(default = "one")]
    pub col_span: usize,
    #[serde(default)]
    pub header: bool,
}
fn one() -> usize { 1 }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Content {
    Heading { level: u8, text: String },
    Paragraph { text: String },
    Code { language: Option<String>, text: String },
    Quote { text: String },
    ListItem { text: String, ordered: bool },
    Table { rows: Vec<Vec<Cell>> },
    Image { url: String, alt: String },
    Math { text: String },
    Caption { text: String },
}
impl Content {
    pub fn text(&self) -> String {
        match self {
            Self::Heading { text, .. } | Self::Paragraph { text } |
            Self::Code { text, .. } | Self::Quote { text } | Self::ListItem { text, .. } |
            Self::Math { text } | Self::Caption { text } => text.clone(),
            Self::Image { alt, .. } => alt.clone(),
            Self::Table { rows } => rows.iter().map(|r| r.iter().map(|c| c.text.clone())
                .collect::<Vec<_>>().join("\t")).collect::<Vec<_>>().join("\n"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Block {
    pub id: String,
    pub content: Content,
    pub locator: Locator,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Link { pub url: String, pub text: String }
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Warning { pub code: String, pub message: String }
impl Warning {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self { code: code.into(), message: message.into() }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub sha256: String,
    pub media_type: String,
    pub size: u64,
    /// Describes provenance, for example "http_response" or "rendered_dom".
    pub role: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    pub requested: String,
    pub resolved: String,
    pub retrieved_at: String,
    pub status: Option<u16>,
    pub version: Option<String>,
    pub original: Artifact,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub schema_version: u32,
    pub id: String,
    pub title: String,
    pub source: Source,
    pub parser: String,
    pub extraction_version: String,
    pub blocks: Vec<Block>,
    pub links: Vec<Link>,
    pub metadata: Value,
    pub warnings: Vec<Warning>,
}
impl Document {
    pub fn text(&self) -> String {
        self.blocks.iter().map(|b| b.content.text()).collect::<Vec<_>>().join("\n\n")
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Renderer { #[default] Auto, Http, Captions, Lightpanda, Chromium, Crw }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadRequest {
    pub url: String,
    #[serde(default)] pub refresh: bool,
    #[serde(default)] pub renderer: Renderer,
    #[serde(default = "default_language")] pub language: String,
    #[serde(default)] pub library: Option<String>,
    #[serde(default)] pub selector: Option<String>,
    #[serde(default)] pub actor: Option<String>,
}
pub fn default_language() -> String { "en".into() }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadResponse { pub document: Document, pub cached: bool }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    #[serde(default = "ten")] pub limit: usize,
    #[serde(default)] pub library: Option<String>,
}
fn ten() -> usize { 10 }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    /// This is a provider snippet, not evidence fetched from the destination.
    pub snippet: String,
    pub score: f64,
    pub providers: Vec<String>,
    pub document_id: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub query: String,
    pub results: Vec<SearchResult>,
    pub warnings: Vec<Warning>,
    pub elapsed_ms: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindRequest {
    pub query: String,
    #[serde(default)] pub regex: bool,
    #[serde(default)] pub ignore_case: bool,
    #[serde(default = "hundred")] pub limit: usize,
}
fn hundred() -> usize { 100 }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Match {
    pub block_id: String,
    pub locator: Locator,
    pub text: String,
    /// UTF-8 byte offsets into `text`, not Unicode character indices.
    pub ranges: Vec<[usize; 2]>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindResponse { pub document_id: String, pub matches: Vec<Match>, pub truncated: bool }
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtractKind { Tables, Links, Code, Images, Metadata, Outline, JsonPointer, Css }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractRequest { pub kind: ExtractKind, pub expression: Option<String> }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractResponse { pub document_id: String, pub data: Value, pub warnings: Vec<Warning> }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Library { pub name: String, pub description: String, pub created_at: String, pub items: usize }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryCreate { pub name: String, #[serde(default)] pub description: String }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryAdd { pub document_id: String, #[serde(default)] pub actor: Option<String> }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Annotation {
    pub id: String, pub document_id: String, pub actor: String,
    pub note: String, pub tags: Vec<String>, pub created_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotationCreate { pub actor: String, #[serde(default)] pub note: String, #[serde(default)] pub tags: Vec<String> }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSummary { pub id: String, pub title: String, pub url: String, pub retrieved_at: String, pub warnings: usize }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlRequest {
    pub url: String,
    #[serde(default = "twenty")] pub max_pages: usize,
    #[serde(default = "two")] pub max_depth: usize,
    #[serde(default)] pub library: Option<String>,
    #[serde(default)] pub actor: Option<String>,
}
fn twenty() -> usize { 20 }
fn two() -> usize { 2 }
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum JobState { Queued, Running, Complete, Partial, Failed, Cancelled, Interrupted }
impl JobState {
    pub fn terminal(&self) -> bool { !matches!(self, Self::Queued | Self::Running) }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: String, pub state: JobState, pub request: CrawlRequest,
    pub created_at: String, pub updated_at: String,
    pub document_ids: Vec<String>, pub visited: usize,
    /// Completed unsuccessful page attempts. Old saved jobs default to zero.
    #[serde(default)] pub failed: usize,
    pub warnings: Vec<Warning>, pub error: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Problem { pub code: String, pub message: String }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability { pub name: String, pub available: bool, pub detail: String }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Health { pub version: String, pub api_version: String, pub capabilities: Vec<Capability> }

pub mod render;
