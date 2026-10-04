//! Deterministic selected-file matching and retained-source context. No index or LLM.
use super::*;
use regex::{Regex, RegexBuilder};

pub(super) fn pattern(query: &str) -> Result<Regex> {
    RegexBuilder::new(query).size_limit(256 * 1024).dfa_size_limit(1024 * 1024).build().map_err(|_| CodeError::Invalid.into())
}
pub(super) fn glob(query: &str) -> Result<Regex> {
    let mut pattern = String::from("^");
    let mut chars = query.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '*' if chars.peek() == Some(&'*') => {
                chars.next();
                if chars.peek() == Some(&'/') { chars.next(); pattern.push_str("(?:.*/)?"); }
                else { pattern.push_str(".*"); }
            },
            '*' => pattern.push_str("[^/]*"), '?' => pattern.push_str("[^/]"),
            '[' | ']' | '\\' => return Err(CodeError::Invalid.into()),
            c => pattern.push_str(&regex::escape(&c.to_string())),
        }
    }
    pattern.push('$');
    self::pattern(&pattern)
}
fn found(text: &str, start: usize, end: usize, entry: &RepositoryEntry, id: &str) -> CodeMatch {
    let mut a = start.saturating_sub(100);
    let mut b = (end + 200).min(text.len());
    while !text.is_char_boundary(a) { a += 1; }
    while !text.is_char_boundary(b) { b -= 1; }
    let line = text.as_bytes()[..start].iter().filter(|v| **v == b'\n').count() + 1;
    CodeMatch { path: entry.path.clone(), blob_sha: entry.object_sha.clone(), url: format!("{}#L{line}", entry.url), document_id: id.into(),
        line, byte_range: [start, end], excerpt: text[a..b].into() }
}
pub(super) fn language(path: &str) -> Result<&'static str> {
    match path.rsplit('.').next().unwrap_or("") {
        "rs" => Ok("rust"), "py" | "pyi" => Ok("python"), "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" => Ok("javascript"), "go" => Ok("go"),
        _ => Err(CodeError::Unsupported.into()),
    }
}
/// Mask comments and quoted strings with spaces while retaining every byte offset
/// and newline. This is lexical filtering, not a language parser or name resolver.
fn masked(text: &str, language: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = bytes.to_vec();
    let mut i = 0;
    while i < bytes.len() {
        let start = i;
        if (language == "python" && bytes[i] == b'#') || (language != "python" && bytes[i..].starts_with(b"//")) {
            while i < bytes.len() && bytes[i] != b'\n' { i += 1; }
        } else if language != "python" && bytes[i..].starts_with(b"/*") {
            i += 2; let mut depth = 1;
            while i < bytes.len() && depth > 0 {
                if bytes[i..].starts_with(b"/*") && language == "rust" { depth += 1; i += 2; }
                else if bytes[i..].starts_with(b"*/") { depth -= 1; i += 2; }
                else { i += 1; }
            }
        } else if language == "rust" && bytes[i] == b'r' && {
            let mut j = i + 1;
            while j < bytes.len() && bytes[j] == b'#' { j += 1; }
            j < bytes.len() && bytes[j] == b'"'
        } {
            let mut j = i + 1;
            while bytes[j] == b'#' { j += 1; }
            let hashes = j - i - 1;
            i = j + 1;
            while i < bytes.len() {
                if bytes[i] == b'"' && bytes.get(i + 1..i + 1 + hashes).is_some_and(|v| v.iter().all(|b| *b == b'#')) {
                    i += 1 + hashes; break;
                }
                i += 1;
            }
        } else if matches!(bytes[i], b'"' | b'\'' | b'`') && (bytes[i] != b'\'' || language != "rust" || {
            // A Rust lifetime is not a character literal.
            let end = (i + 9).min(bytes.len());
            bytes[i + 1..end].iter().position(|v| *v == b'\'' || *v == b'\n').is_some_and(|n| bytes[i + 1 + n] == b'\'')
        }) {
            let quote = bytes[i];
            let triple = language == "python" && bytes.get(i..i + 3).is_some_and(|v| v == [quote; 3]);
            i += if triple { 3 } else { 1 };
            while i < bytes.len() {
                if bytes[i] == b'\\' && quote != b'`' { i = (i + 2).min(bytes.len()); }
                else if triple && bytes.get(i..i + 3).is_some_and(|v| v == [quote; 3]) { i += 3; break; }
                else if !triple && bytes[i] == quote { i += 1; break; }
                else { i += 1; }
            }
        } else { i += 1; continue; }
        for byte in &mut out[start..i] { if *byte != b'\n' && *byte != b'\r' { *byte = b' '; } }
    }
    // All non-ASCII bytes in masked regions were replaced, not split. Unmasked
    // text remains unchanged, so the result stays valid UTF-8 with exact length.
    String::from_utf8(out).expect("mask preserves UTF-8")
}
fn declarations(language: &str) -> Regex {
    let pattern = match language {
        "rust" => r"(?m)^[ \t]*(?:(?:pub(?:\([^\n)]*\))?|unsafe|async|const|default|extern)[ \t]+)*(?:fn|struct|enum|trait|type|mod|const|static)[ \t]+([A-Za-z_][A-Za-z_0-9]*)",
        "python" => r"(?m)^[ \t]*(?:async[ \t]+)?(?:def|class)[ \t]+([A-Za-z_][A-Za-z_0-9]*)",
        "javascript" => r"(?m)^[ \t]*(?:(?:export|default|declare|async|abstract)[ \t]+)*(?:function|class|interface|type|enum|const|let|var)[ \t]+([A-Za-z_$][A-Za-z_$0-9]*)",
        "go" => r"(?m)^[ \t]*(?:func(?:[ \t]*\([^\n)]*\))?|type|const|var)[ \t]+([A-Za-z_][A-Za-z_0-9]*)",
        _ => unreachable!(),
    };
    Regex::new(pattern).expect("constant bounded declaration pattern")
}
pub(super) fn matches(text: &str, query: &str, mode: CodeSearchMode, entry: &RepositoryEntry, id: &str, limit: usize) -> Result<(Vec<CodeMatch>, bool)> {
    let mut result = Vec::new();
    match mode {
        CodeSearchMode::Regex => {
            for m in pattern(query)?.find_iter(text).filter(|m| !m.is_empty()) {
                if result.len() == limit { return Ok((result, true)); }
                result.push(found(text, m.start(), m.end(), entry, id));
            }
        },
        CodeSearchMode::Symbols => {
            let language = language(&entry.path)?;
            let masked = masked(text, language);
            for capture in declarations(language).captures_iter(&masked) {
                let name = capture.get(1).expect("declaration capture");
                if !name.as_str().contains(query) { continue; }
                if result.len() == limit { return Ok((result, true)); }
                result.push(found(text, name.start(), name.end(), entry, id));
            }
        },
        _ => return Err(CodeError::Invalid.into()),
    }
    Ok((result, false))
}
fn context(text: &str, line: usize, before: usize, after: usize) -> Result<(usize, usize, [usize; 2], String)> {
    if line == 0 || before > 20 || after > 20 { return Err(CodeError::Invalid.into()); }
    let lines: Vec<_> = text.split_inclusive('\n').collect();
    if line > lines.len() { return Err(CodeError::Unavailable.into()); }
    let start_line = line.saturating_sub(before).max(1);
    let end_line = line.saturating_add(after).min(lines.len());
    let start = lines[..start_line - 1].iter().map(|v| v.len()).sum();
    let end = start + lines[start_line - 1..end_line].iter().map(|v| v.len()).sum::<usize>();
    if end - start > 64 * 1024 { return Err(CodeError::Limit.into()); }
    Ok((start_line, end_line, [start, end], text[start..end].into()))
}
impl Engine {
    pub async fn code_context(&self, request: CodeContextRequest) -> Result<CodeContextResponse> {
        self.code_run(async {
            let map = saved_map(self, &request.map_id).await?;
            let document = self.store.document(&request.file_id).await?;
            let code = &document.metadata["code"];
            if document.parser != FILE_PARSER || !code["repository"].as_str().is_some_and(|v| v.eq_ignore_ascii_case(&map.repository)) { return Err(CodeError::Identity.into()); }
            let entry = admitted(&map, string(code, "path")?)?.clone();
            if code["blob_sha"].as_str() != Some(&entry.object_sha) || entry.size != Some(document.source.original.size) { return Err(CodeError::Identity.into()); }
            let bytes = self.store.bytes(&document.source.original).await?;
            let text = std::str::from_utf8(&bytes).map_err(|_| CodeError::Unsupported)?;
            let (start_line, end_line, byte_range, text) = context(text, request.line, request.before, request.after)?;
            Ok(CodeContextResponse { map_id: request.map_id, file_id: request.file_id, repository: map.repository, requested_ref: map.requested_ref,
                resolved_commit: map.resolved_commit, path: entry.path.clone(), blob_sha: entry.object_sha.clone(), url: format!("{}#L{start_line}-L{end_line}", entry.url),
                start_line, end_line, byte_range, text, coverage: CodeCoverage { incomplete: map.coverage.incomplete, warnings: map.coverage.warnings, ..Default::default() } })
        }).await
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn declarations_skip_quoted_and_commented_source_and_keep_original_ranges() {
        let text = "// struct Fake {}\n/* fn fake() {} */\npub struct Buffer;\nlet x = r#\"\nfn fake() {}\n\"#;\nimpl Buffer {\n    pub fn new() -> Self { todo!() }\n}\n";
        let entry = RepositoryEntry { path: "src/lib.rs".into(), kind: "blob".into(), mode: "100644".into(), object_sha: "b".repeat(40), size: None, url: "https://github.com/example/repo/blob/commit/src/lib.rs".into() };
        let (found, _) = matches(text, "Buffer", CodeSearchMode::Symbols, &entry, "saved", 5).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(&text[found[0].byte_range[0]..found[0].byte_range[1]], "Buffer");
        assert!(matches(text, "fake", CodeSearchMode::Symbols, &entry, "saved", 5).unwrap().0.is_empty());
        let (start, end, range, value) = context(text, 3, 1, 1).unwrap();
        assert_eq!((start, end), (2, 4)); assert_eq!(value, text[range[0]..range[1]]);
        assert!(glob("**/*.rs").unwrap().is_match("lib.rs"));
        assert!(!glob("src/*.rs").unwrap().is_match("src/sub/lib.rs"));
        assert!(pattern("[").is_err());
    }
}
