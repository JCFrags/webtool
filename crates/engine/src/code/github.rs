//! Public GitHub object snapshots. Lists are discovery, not accepted bodies.
use super::*;

pub(super) fn validate_page(page: &GitHubPage) -> Result<()> {
    if !(1..=1000).contains(&page.page) || !(1..=20).contains(&page.limit) { return Err(CodeError::Invalid.into()); }
    Ok(())
}
fn route(kind: GitHubKind) -> &'static str {
    match kind { GitHubKind::Issue => "issues", GitHubKind::PullRequest => "pulls", GitHubKind::Release => "releases" }
}
pub(super) fn next_page(response: &Response, current: &GitHubPage) -> Result<Option<usize>> {
    let Some(header) = &response.pagination else { return Ok(None); };
    for part in header.split(',') {
        let fields: Vec<_> = part.trim().split(';').map(str::trim).collect();
        if !fields.iter().skip(1).any(|v| *v == "rel=\"next\"") { continue; }
        let target = fields[0].strip_prefix('<').and_then(|v| v.strip_suffix('>')).ok_or(CodeError::Identity)?;
        let target = Url::parse(target).map_err(|_| CodeError::Identity)?;
        let original = Url::parse(&response.observation.url).map_err(|_| CodeError::Identity)?;
        if target.scheme() != "https" || target.host_str() != Some("api.github.com") || target.path() != original.path() ||
            !target.username().is_empty() || target.password().is_some() || target.port().is_some() || target.fragment().is_some() {
            return Err(CodeError::Identity.into());
        }
        let pages: Vec<_> = target.query_pairs().filter(|(k, _)| k == "page").map(|(_, v)| v.parse::<usize>()).collect();
        if pages.len() != 1 || pages[0].as_ref().ok() != Some(&(current.page + 1)) { return Err(CodeError::Identity.into()); }
        return Ok(Some(current.page + 1));
    }
    Ok(None)
}
fn display_url(repository: &str, kind: GitHubKind, number: Option<u64>, tag: Option<&str>) -> Result<String> {
    let (owner, repo) = super::repository(repository)?;
    let mut url = Url::parse("https://github.com/").expect("constant URL");
    let mut parts = url.path_segments_mut().expect("HTTP URL");
    parts.extend([owner, repo]);
    match kind {
        GitHubKind::Issue => { parts.push("issues").push(&number.ok_or(CodeError::Invalid)?.to_string()); },
        GitHubKind::PullRequest => { parts.push("pull").push(&number.ok_or(CodeError::Invalid)?.to_string()); },
        GitHubKind::Release => { parts.extend(["releases", "tag", tag.ok_or(CodeError::Invalid)?]); },
    }
    drop(parts);
    Ok(url.into())
}
fn identity(value: &Value, repository: &str, kind: GitHubKind, number: Option<u64>, tag: Option<&str>) -> Result<String> {
    let expected = display_url(repository, kind, number, tag)?;
    if !string(value, "html_url")?.eq_ignore_ascii_case(&expected) { return Err(CodeError::Identity.into()); }
    match kind {
        GitHubKind::Issue => {
            if value["number"].as_u64() != number || value.get("pull_request").is_some() { return Err(CodeError::Identity.into()); }
        },
        GitHubKind::PullRequest => {
            if value["number"].as_u64() != number || !value["base"]["repo"]["full_name"].as_str().is_some_and(|v| v.eq_ignore_ascii_case(repository)) ||
                value["base"]["repo"]["private"].as_bool() != Some(false) { return Err(CodeError::Identity.into()); }
            sha(string(&value["base"], "sha")?)?;
            sha(string(&value["head"], "sha")?)?;
        },
        GitHubKind::Release => {
            if value["tag_name"].as_str() != tag || value["draft"].as_bool() != Some(false) { return Err(CodeError::Identity.into()); }
        },
    }
    if value["id"].as_u64().is_none() { return Err(CodeError::Identity.into()); }
    Ok(expected)
}
fn body(parsed: &mut Parsed, value: &Value, pointer: &str) -> Result<()> {
    match value.pointer(pointer) {
        Some(Value::String(text)) => parsed.push(Content::Paragraph { text: text.clone() }, Locator::JsonPointer { pointer: pointer.into() }),
        Some(Value::Null) => parsed.warnings.push(Warning::new("github_body_null", "The provider supplied a null body. No body text was invented.")),
        None => parsed.warnings.push(Warning::new("github_body_missing", "The provider did not supply a body field. This is distinct from null.")),
        _ => return Err(CodeError::Upstream.into()),
    }
    Ok(())
}
fn rights() -> Value { json!({"status":"unknown","reason":"Public visibility does not establish reuse rights. Native notices and metadata remain in the original JSON."}) }
async fn save(e: &Engine, mut parsed: Parsed, response: &Response, resolved: String) -> Result<Document> {
    parsed.metadata["api_observation"] = json!(response.observation);
    e.finish(parsed, Source { requested: response.observation.url.clone(), resolved, retrieved_at: Utc::now().to_rfc3339(),
        status: Some(response.observation.status), version: Some(API_VERSION.into()), original: response.observation.artifact.clone() }, vec![]).await
}
impl Engine {
    pub async fn github_list(&self, request: GitHubListRequest) -> Result<GitHubListResponse> {
        self.code_run(async {
            let (owner, repo) = repository(&request.repository)?;
            validate_page(&request.pagination)?;
            if request.kind == GitHubKind::Release && request.state != GitHubState::All { return Err(CodeError::Invalid.into()); }
            let mut url = endpoint(&["repos", owner, repo, route(request.kind)]);
            url.query_pairs_mut().append_pair("page", &request.pagination.page.to_string()).append_pair("per_page", &request.pagination.limit.to_string());
            if request.kind != GitHubKind::Release {
                let state = match request.state { GitHubState::All => "all", GitHubState::Open => "open", GitHubState::Closed => "closed" };
                url.query_pairs_mut().append_pair("state", state).append_pair("sort", "created").append_pair("direction", "desc");
            }
            let mut budget = Budget::new(1, 2 * 1024 * 1024);
            let response = self.code_get(url, &mut budget).await?;
            let value = response.json()?;
            let native = value.as_array().ok_or(CodeError::Upstream)?;
            if native.len() > request.pagination.limit { return Err(CodeError::Identity.into()); }
            let next = next_page(&response, &request.pagination)?;
            budget.coverage.incomplete = request.pagination.page > 1 || next.is_some();
            budget.coverage.warnings.push(Warning::new("github_discovery_page", "One mutable provider page. Listed titles and metadata are discovery, not accepted issue, PR, release, code, or asset bodies. No pagination was followed."));
            if request.kind == GitHubKind::Issue {
                budget.coverage.warnings.push(Warning::new("github_issues_include_prs", "GitHub's issues page includes pull requests. Each admitted item identifies its actual kind. No hidden filtering or refill request is used."));
            }
            let mut parsed = Parsed::new(&format!("{} {:?} discovery", request.repository, request.kind), "github-object-list/1");
            parsed.push(Content::Paragraph { text: "Discovery metadata only. Select an object explicitly to read its body.".into() }, Locator::Derived { index: 1 });
            let mut items = Vec::new();
            for (index, item) in native.iter().enumerate() {
                let kind = if request.kind == GitHubKind::Issue && item.get("pull_request").is_some() { GitHubKind::PullRequest } else { request.kind };
                // Issue-list PR records have no base/head object. Check the exact public identity without treating them as fetched PR details.
                let expected = display_url(&request.repository, kind, item["number"].as_u64(), item["tag_name"].as_str())?;
                if !string(item, "html_url")?.eq_ignore_ascii_case(&expected) { return Err(CodeError::Identity.into()); }
                if kind == GitHubKind::Release && item["draft"].as_bool() != Some(false) { return Err(CodeError::Identity.into()); }
                let mut summary = serde_json::Map::new();
                for key in ["id", "number", "title", "name", "tag_name", "state", "html_url", "user", "author", "labels", "created_at", "updated_at", "published_at", "draft", "prerelease"] {
                    if let Some(value) = item.get(key) { summary.insert(key.into(), value.clone()); }
                }
                summary.insert("kind".into(), json!(kind));
                items.push(Value::Object(summary));
                let key = if kind == GitHubKind::Release { if item["name"].is_string() { "name" } else { "tag_name" } } else { "title" };
                parsed.push(Content::Paragraph { text: string(item, key)?.into() }, Locator::JsonPointer { pointer: format!("/{index}/{key}") });
                parsed.links.push(Link { url: expected, text: string(item, key)?.into() });
            }
            parsed.metadata = json!({"github_list":{"request":request,"next_page":next,"coverage":budget.coverage},"rights":rights()});
            parsed.warnings = budget.coverage.warnings.clone();
            let document = save(self, parsed, &response, response.observation.url.clone()).await?;
            Ok(GitHubListResponse { document_id: document.id, repository: request.repository, kind: request.kind, observed_at: document.source.retrieved_at,
                pagination: request.pagination, next_page: next, items, observation: response.observation, coverage: budget.coverage })
        }).await
    }
    pub async fn github_read(&self, request: GitHubReadRequest) -> Result<GitHubReadResponse> {
        self.code_run(async {
            let (owner, repo) = repository(&request.repository)?;
            if let Some(page) = &request.comments { validate_page(page)?; }
            let url = match request.kind {
                GitHubKind::Issue | GitHubKind::PullRequest => {
                    let number = request.number.filter(|n| *n > 0 && *n <= i64::MAX as u64).ok_or(CodeError::Invalid)?;
                    if request.tag.is_some() { return Err(CodeError::Invalid.into()); }
                    endpoint(&["repos", owner, repo, route(request.kind), &number.to_string()])
                },
                GitHubKind::Release => {
                    let tag = request.tag.as_deref().ok_or(CodeError::Invalid)?;
                    if request.number.is_some() || tag.is_empty() || tag.len() > 256 || tag.chars().any(char::is_control) || matches!(tag, "." | "..") { return Err(CodeError::Invalid.into()); }
                    if request.comments.is_some() { return Err(CodeError::Unsupported.into()); }
                    endpoint(&["repos", owner, repo, "releases", "tags", tag])
                },
            };
            let mut budget = Budget::new(2, 4 * 1024 * 1024);
            let response = self.code_get(url, &mut budget).await?;
            let value = response.json()?;
            let resolved = identity(&value, &request.repository, request.kind, request.number, request.tag.as_deref())?;
            let title_key = if request.kind == GitHubKind::Release { if value["name"].is_string() { "name" } else { "tag_name" } } else { "title" };
            let mut parsed = Parsed::new(string(&value, title_key)?, "github-object-body/1");
            parsed.push(Content::Heading { level: 1, text: string(&value, title_key)?.into() }, Locator::JsonPointer { pointer: format!("/{title_key}") });
            body(&mut parsed, &value, "/body")?;
            parsed.metadata = json!({"github_object":{"repository":request.repository,"kind":request.kind,"number":request.number,"tag":request.tag,
                "native":value,"snapshot":"Retained API JSON, not an immutable issue/PR/release or historical-state assertion."},"rights":rights()});
            parsed.warnings.push(Warning::new("github_mutable_snapshot", "Issue, PR, and release metadata can change. This saved API snapshot is not a commit-pinned conversation, complete timeline, review set, or asset download. PR base/head SHAs are observed metadata. Release target_commitish is not resolved to a commit."));
            if request.kind != GitHubKind::Release && request.comments.is_none() && value["comments"].as_u64().is_none_or(|n| n > 0) {
                budget.coverage.incomplete = true;
                budget.coverage.warnings.push(Warning::new("github_comments_not_read", "Conversation comments were not selected. Request one explicit comment page to read them."));
            }
            parsed.warnings.extend(budget.coverage.warnings.clone());
            let document = save(self, parsed, &response, resolved.clone()).await?;
            let mut comments = None;
            let mut next = None;
            if let Some(page) = &request.comments {
                let mut url = endpoint(&["repos", owner, repo, "issues", &request.number.expect("validated number").to_string(), "comments"]);
                url.query_pairs_mut().append_pair("page", &page.page.to_string()).append_pair("per_page", &page.limit.to_string());
                match self.code_get(url, &mut budget).await {
                    Ok(response) => {
                        let value = response.json()?;
                        let native = value.as_array().ok_or(CodeError::Upstream)?;
                        if native.len() > page.limit { return Err(CodeError::Identity.into()); }
                        next = next_page(&response, page)?;
                        budget.coverage.incomplete |= page.page > 1 || next.is_some();
                        let mut parsed = Parsed::new(&format!("{} conversation comments", document.title), "github-conversation-comments/1");
                        parsed.push(Content::Paragraph { text: format!("Selected conversation comment page {}. Reviews and timeline events were not read.", page.page) }, Locator::Derived { index: 1 });
                        for (index, comment) in native.iter().enumerate() {
                            let issue = endpoint(&["repos", owner, repo, "issues", &request.number.expect("validated number").to_string()]);
                            if !string(comment, "issue_url")?.eq_ignore_ascii_case(issue.as_str()) || comment["id"].as_u64().is_none() { return Err(CodeError::Identity.into()); }
                            body(&mut parsed, &value, &format!("/{index}/body"))?;
                        }
                        parsed.metadata = json!({"github_comments":{"parent_document_id":document.id,"repository":request.repository,"number":request.number,
                            "pagination":page,"next_page":next},"rights":rights()});
                        parsed.warnings.push(Warning::new("github_comment_scope", "Conversation comments are a separate mutable snapshot. They are not PR review comments, reviews, timeline events, or a transaction-consistent parent/comment view."));
                        comments = Some(save(self, parsed, &response, response.observation.url.clone()).await?);
                    },
                    Err(error) => {
                        let Some(domain) = error.downcast_ref::<CodeError>() else { return Err(error); };
                        budget.stop(*domain);
                        budget.coverage.warnings.push(Warning::new("github_comments_unavailable", "The selected comment page failed. The separately accepted object body remains available."));
                    },
                }
            }
            if request.kind == GitHubKind::PullRequest {
                budget.coverage.incomplete = true;
                budget.coverage.warnings.push(Warning::new("github_pr_scope", "PR body and optional issue-conversation comments only. Reviews, inline review comments, commits, checks, and changed files were not requested."));
            }
            Ok(GitHubReadResponse { document, comments, next_comment_page: next, coverage: budget.coverage })
        }).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn object_identity_and_missing_body_are_explicit() {
        let issue = json!({"id":1,"number":23,"html_url":"https://github.com/example/repo/issues/23","title":"Issue","body":null});
        assert!(identity(&issue, "example/repo", GitHubKind::Issue, Some(23), None).is_ok());
        assert!(identity(&issue, "example/repo", GitHubKind::Issue, Some(24), None).is_err());
        let mut parsed = Parsed::new("test", "test");
        body(&mut parsed, &issue, "/body").unwrap();
        body(&mut parsed, &issue, "/absent").unwrap();
        assert_eq!(parsed.warnings[0].code, "github_body_null");
        assert_eq!(parsed.warnings[1].code, "github_body_missing");
        assert!(validate_page(&GitHubPage { page: 0, limit: 5 }).is_err());
    }
}
