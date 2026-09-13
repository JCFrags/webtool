# Roadmap

## Active follow-up: exclude search ads

The user approved paid/sponsored search-result exclusion on the existing branch
and PR #24. Check provider HTML context before result conversion and ranking.
Keep organic results, saved-library search, existing provider selection, ranking
rules, configuration, dependency pins and read-quality improvements. Keep the
PR unmerged. See [SEARCH.md](SEARCH.md) for the filtering boundary and limits.
No other deferred search interface or backlog feature is approved.

## Preserved work: reliable, clean read

Issue [#23](https://github.com/JCFrags/webtool/issues/23), PR
[#24](https://github.com/JCFrags/webtool/pull/24), branch `feat/read-quality`.
The initial two-page workflow and the user-approved table/footer follow-up are
verified and installed locally. The follow-up used retained Chemistry input and
one fresh installed ordinary read. Remote acceptance is pending. The search-ad
follow-up above is the only newly approved work.

- Select coherent main content without losing substantive sections, code, tables, references, captions, or discussion replies.
- Make ordinary read compact and readable. Keep provenance behind `--details`, full JSON, and retained originals.
- Recover likely JavaScript shells with one bounded configured Lightpanda attempt after HTTP. Keep explicit renderer choices and native routes.
- Preserve the Rust Book and JavaScript proof. Verify the table/footer follow-up with retained sources, then install the update with rollback.

Provider selection, ranking rules and configuration stay unchanged. No packaging,
release, broad test corpus, benchmark or other deferred feature work is approved.

## Deferred work

These areas remain backlog, not parallel priorities:

- Format coverage, OCR readiness, figure extraction, and figure assets.
- Markdown, mathematical notation, list hierarchy, and broader source fidelity.
- GitHub issues/PRs/releases and wider media/caption coverage.
- Crawl resume, persisted frontiers, and sitemap expansion.
- Academic search, citation graphs, bibliography tools, and reference-manager integration.
- Optional explicit LLM jobs. No mandatory runtime or silent rewriting.
- Search filters, categories, dates, languages, and other search interfaces.
- Portability and platform/linkage support beyond the verified environment.
- Performance improvements supported by measurements, not assumed gains.

## Completed baseline

The published `v0.1.0-alpha.1` retains the existing search, source readers,
libraries, crawling, citations, and shared-client setup. See [STATUS.md](STATUS.md)
for bounded evidence and limitations. Published tags and assets are immutable.
