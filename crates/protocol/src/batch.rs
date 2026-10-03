//! Compact per-input outcomes for explicit ordinary reads, not a bulk job system.
use serde::{Deserialize, Serialize};
use crate::Problem;

fn auto() -> String { "auto".into() }

pub const BATCH_READ_INPUT_LIMIT: usize = 5;
pub const BATCH_READ_BODY_LIMIT: usize = 64 * 1024;
pub const BATCH_READ_RESPONSE_LIMIT: usize = 8 * 1024;

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BatchReadRequest {
    /// One to five inputs. Results retain this order, including failures.
    #[cfg_attr(feature = "openapi", schema(min_items = 1, max_items = 5))]
    pub inputs: Vec<BatchReadInput>,
}

/// Options have the same meaning as ReadRequest. A missing URL is an input
/// failure, not a reason to discard other admitted inputs. Invalid field types
/// or unknown fields are request-level schema errors.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BatchReadInput {
    /// Absolute HTTP(S) URL without credentials, at most 8192 UTF-8 bytes.
    #[serde(default)] pub url: Option<String>,
    #[serde(default)] pub refresh: bool,
    /// auto, http, captions, lightpanda, chromium, or crw. Unknown strings are input errors.
    #[serde(default = "auto")] pub renderer: String,
    /// Language and each optional text field have a 4096-byte input bound.
    #[serde(default = "crate::default_language")] pub language: String,
    #[serde(default)] pub library: Option<String>,
    #[serde(default)] pub selector: Option<String>,
    #[serde(default)] pub actor: Option<String>,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchReadResponse {
    pub results: Vec<BatchReadOutcome>,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum BatchReadOutcome {
    Saved {
        /// Zero-based input index. Retrieve the saved document through its ordinary endpoint.
        index: usize,
        document_id: String,
        cached: bool,
        /// Details remain in the saved document. Do not ignore warnings.
        warning_count: usize,
    },
    Error {
        index: usize,
        /// Status the ordinary safe HTTP error boundary assigns to this failure.
        http_status: u16,
        problem: Problem,
    },
}
