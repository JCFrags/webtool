//! Compare admitted map entries offline. Provider patches require explicit opt-in.
use std::collections::{BTreeMap, BTreeSet};
use super::*;

fn changes(base: &RepositoryMap, head: &RepositoryMap) -> Vec<CodeMapChange> {
    let old: BTreeMap<_, _> = base.entries.iter().map(|v| (v.path.as_str(), v)).collect();
    let new: BTreeMap<_, _> = head.entries.iter().map(|v| (v.path.as_str(), v)).collect();
    let paths: BTreeSet<_> = old.keys().chain(new.keys()).copied().collect();
    paths.into_iter().filter_map(|path| {
        let a = old.get(path).copied();
        let b = new.get(path).copied();
        let status = match (a, b) {
            (Some(a), Some(b)) if a.object_sha == b.object_sha && a.mode == b.mode && a.kind == b.kind => return None,
            (Some(_), Some(_)) => "modified",
            (None, Some(_)) if base.coverage.incomplete => "not_admitted_in_base",
            (None, Some(_)) => "added",
            (Some(_), None) if head.coverage.incomplete => "not_admitted_in_head",
            (Some(_), None) => "removed",
            (None, None) => unreachable!(),
        };
        Some(CodeMapChange { path: path.into(), base: a.cloned(), head: b.cloned(), status: status.into() })
    }).collect()
}
fn provider_identity(value: &Value, expected: &Url, base: &str) -> Result<()> {
    if value["base_commit"]["sha"].as_str() != Some(base) { return Err(CodeError::Identity.into()); }
    let actual = Url::parse(string(value, "url")?).map_err(|_| CodeError::Identity)?;
    if actual.scheme() != expected.scheme() || actual.host_str() != expected.host_str() || !actual.path().eq_ignore_ascii_case(expected.path()) ||
        !actual.username().is_empty() || actual.password().is_some() || actual.port().is_some() { return Err(CodeError::Identity.into()); }
    sha(string(&value["merge_base_commit"], "sha")?)?;
    if value["total_commits"].as_u64().is_none() || !matches!(value["status"].as_str(), Some("ahead" | "behind" | "diverged" | "identical")) { return Err(CodeError::Upstream.into()); }
    Ok(())
}
impl Engine {
    pub async fn code_compare(&self, request: CodeCompareRequest) -> Result<CodeCompareResponse> {
        self.code_run(async {
            github::validate_page(&request.pagination)?;
            if !request.provider && request.pagination.page != 1 { return Err(CodeError::Invalid.into()); }
            let base = saved_map(self, &request.base_map_id).await?;
            let head = saved_map(self, &request.head_map_id).await?;
            if !base.repository.eq_ignore_ascii_case(&head.repository) || base.path != head.path { return Err(CodeError::Invalid.into()); }
            let changes = changes(&base, &head);
            let mut coverage = CodeCoverage { incomplete: base.coverage.incomplete || head.coverage.incomplete, ..Default::default() };
            coverage.warnings.push(Warning::new("admitted_map_comparison", "Only admitted entries in two saved maps were compared. Missing entries in an incomplete map are unknown, not confirmed additions or removals. No rename inference or whole-repository content scan was made."));
            let manifest = json!({"base_map_id":request.base_map_id,"head_map_id":request.head_map_id,"repository":base.repository,
                "base_requested_ref":base.requested_ref,"head_requested_ref":head.requested_ref,"base_commit":base.resolved_commit,"head_commit":head.resolved_commit,
                "base_scope":{"path":base.path,"coverage":base.coverage},"head_scope":{"path":head.path,"coverage":head.coverage},"changes":changes});
            let original = self.store.put_bytes(&serde_json::to_vec(&manifest)?, "application/json", "derived_repository_comparison").await?;
            let (owner, repo) = repository(&base.repository)?;
            let comparison = format!("{}...{}", base.resolved_commit, head.resolved_commit);
            let mut display = Url::parse("https://github.com/").expect("constant URL");
            display.path_segments_mut().expect("HTTP URL").extend([owner, repo, "compare", &comparison]);
            let mut parsed = Parsed::new(&format!("{} admitted map comparison", base.repository), "github-map-comparison/1");
            parsed.push(Content::Paragraph { text: format!("Admitted entries at {} and {}. File bodies were not compared.", base.resolved_commit, head.resolved_commit) }, Locator::Derived { index: 1 });
            for change in &changes {
                parsed.push(Content::Paragraph { text: format!("{}: {}", change.status, change.path) }, Locator::Derived { index: parsed.blocks.len() + 1 });
            }
            parsed.metadata = json!({"code_comparison":manifest,"rights":{"status":"unknown"}});
            let document = self.finish(parsed, Source { requested: display.to_string(), resolved: display.to_string(), retrieved_at: Utc::now().to_rfc3339(),
                status: None, version: Some(comparison.clone()), original }, coverage.warnings.clone()).await?;
            let mut provider_document = None;
            let mut next = None;
            if request.provider {
                let mut url = endpoint(&["repos", owner, repo, "compare", &comparison]);
                url.query_pairs_mut().append_pair("page", &request.pagination.page.to_string()).append_pair("per_page", &request.pagination.limit.to_string());
                let mut budget = Budget::new(1, 4 * 1024 * 1024);
                match self.code_get(url.clone(), &mut budget).await {
                    Ok(response) => {
                        let value = response.json()?;
                        provider_identity(&value, &url, &base.resolved_commit)?;
                        let commits = value["commits"].as_array().ok_or(CodeError::Upstream)?;
                        if commits.len() > request.pagination.limit { return Err(CodeError::Identity.into()); }
                        for commit in commits { sha(string(commit, "sha")?)?; }
                        next = github::next_page(&response, &request.pagination)?;
                        let files = if request.pagination.page == 1 { value["files"].as_array().ok_or(CodeError::Upstream)? } else {
                            // Later pages can omit files completely. Never label that as zero changed files.
                            value["files"].as_array().unwrap_or_else(|| { static EMPTY: Vec<Value> = Vec::new(); &EMPTY })
                        };
                        if files.len() > 300 { return Err(CodeError::Identity.into()); }
                        let total = value["total_commits"].as_u64().expect("validated total");
                        budget.coverage.incomplete = request.pagination.page > 1 || next.is_some() || total > commits.len() as u64 || files.len() >= 300;
                        if !files.is_empty() && request.pagination.page > 1 { budget.coverage.incomplete = true; }
                        let mut parsed = Parsed::new(&format!("{} provider comparison", base.repository), "github-provider-comparison/1");
                        parsed.push(Content::Paragraph { text: string(&value, "status")?.into() }, Locator::JsonPointer { pointer: "/status".into() });
                        for (index, commit) in commits.iter().enumerate() {
                            if let Some(message) = commit["commit"]["message"].as_str() {
                                parsed.push(Content::Paragraph { text: message.into() }, Locator::JsonPointer { pointer: format!("/commits/{index}/commit/message") });
                            }
                        }
                        for (index, file) in files.iter().enumerate() {
                            path(string(file, "filename")?, false)?;
                            if let Some(previous) = file["previous_filename"].as_str() { path(previous, false)?; }
                            parsed.push(Content::Heading { level: 2, text: string(file, "filename")?.into() }, Locator::JsonPointer { pointer: format!("/files/{index}/filename") });
                            if let Some(patch) = file["patch"].as_str() {
                                parsed.push(Content::Code { language: Some("diff".into()), text: patch.into() }, Locator::JsonPointer { pointer: format!("/files/{index}/patch") });
                            } else {
                                budget.coverage.incomplete = true;
                                budget.coverage.warnings.push(Warning::new("github_patch_unavailable", "A changed file has no supplied patch. Binary or omitted patch content was not invented or fetched."));
                            }
                        }
                        budget.coverage.warnings.push(Warning::new("github_comparison_scope", "One exact-SHA comparison page. GitHub supplies changed files only on page one, up to 300. Patches are provider-supplied excerpts, not independently verified complete file bodies. The diff uses GitHub's merge-base semantics, not necessarily a direct base-to-head tree diff."));
                        parsed.metadata = json!({"github_comparison":{"base_map_id":request.base_map_id,"head_map_id":request.head_map_id,
                            "repository":base.repository,"base_commit":base.resolved_commit,"head_commit":head.resolved_commit,
                            "pagination":request.pagination,"next_page":next,"pagination_link_header":response.pagination,"native":value,"coverage":budget.coverage},"api_observation":response.observation,"rights":{"status":"unknown"}});
                        parsed.warnings = budget.coverage.warnings.clone();
                        provider_document = Some(self.finish(parsed, Source { requested: response.observation.url.clone(), resolved: display.to_string(),
                            retrieved_at: Utc::now().to_rfc3339(), status: Some(response.observation.status), version: Some(comparison), original: response.observation.artifact }, vec![]).await?);
                    },
                    Err(error) => {
                        let Some(domain) = error.downcast_ref::<CodeError>() else { return Err(error); };
                        budget.stop(*domain);
                    },
                }
                budget.coverage.incomplete |= coverage.incomplete;
                budget.coverage.warnings.extend(coverage.warnings);
                coverage = budget.coverage;
            }
            Ok(CodeCompareResponse { document_id: document.id, base_map_id: request.base_map_id, head_map_id: request.head_map_id, repository: base.repository,
                base_requested_ref: base.requested_ref, head_requested_ref: head.requested_ref, base_commit: base.resolved_commit, head_commit: head.resolved_commit,
                changes, provider_document, next_page: next, coverage })
        }).await
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_admission_is_not_a_confirmed_addition() {
        let map = |incomplete| RepositoryMap { repository: "example/repo".into(), requested_ref: "main".into(), resolved_commit: "a".repeat(40), root_tree: "b".repeat(40),
            path: "".into(), limits: MapLimits::default(), entries: vec![], observations: vec![], coverage: CodeCoverage { incomplete, ..Default::default() } };
        let mut head = map(false);
        head.entries.push(RepositoryEntry { path: "a.rs".into(), kind: "blob".into(), mode: "100644".into(), object_sha: "c".repeat(40), size: Some(3), url: "https://github.com/example/repo/blob/commit/a.rs".into() });
        assert_eq!(changes(&map(true), &head)[0].status, "not_admitted_in_base");
        assert_eq!(changes(&map(false), &head)[0].status, "added");
    }
}
