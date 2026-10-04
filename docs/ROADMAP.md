# Roadmap

## Active work: two sprints with a Ketch gate

PR #24 is merged and the research core is delivered. The next work is split into
[two sprints](SPRINTS.md). Keep the Rust service, thin text CLI, shared store,
source originals, and LLM-free ordinary operations.

Sprint 1 covers Documents, GitHub/code, Backend, and Linux/Docker product
readiness. Installation and operation must work for other people without this
workstation's configuration. Use scoped parallel work.

Sprint 2 covers optional answers, scholarly/bibliography extensions, search
extensions, and media follow-ons. Sprint 2 and GitHub merges wait for a strict
same-input comparison against pinned Ketch. Webtool must meet or exceed Ketch
quality and input-to-output wall clock in overlapping workflows. Slightly higher
system resources are allowed only with time and quality parity or better. Use a
brand-new product-blinded subagent for subjective evaluation.

The earlier milestones below retain historical evidence and source-fidelity
lessons. Their merge holds are not current Sprint 1 instructions. Preserve
published alpha assets, the running installation, data/configuration, helpers,
snapshots, historical worktrees, and rollback assets. Credentials, paid calls,
models, live media, and exposure changes remain separate decisions. Plans and
source changes are not proof of installed activation.

## Preserved baseline: live reading quality

The user approved continued live testing and correction across page types. The
product is not finished. Keep PR #24 unmerged and published alpha assets unchanged.
Preserve prior useful behavior, configuration, helpers, libraries, and snapshots.
The narrower limits in the historical milestones below do not prohibit this pass.

Use actual CLI reads and compare retained sources with text and Markdown. Fix
content loss and poor presentation as well as failed retrieval. Current confirmed
work covers selected link destinations, mathematical notation, citation titles,
list hierarchy and numbering, code-language labels, source spacing, custom modal
exclusion, and article-footer qualifications. Do not add access bypasses, a hidden
extractor chain, automatic clicks, or unbounded retries.

Live testing found that Britannica can currently return a readable HTTP response
but includes tool dialogs. Amazon still has no accepted product content after one
bounded browser attempt. The older Amazon snapshot contained inactive markup, not
a useful product inventory. One news article lost an editorial qualification.
These are separate mechanisms, not evidence of one universal fetch regression.
The current correction is installed and active from clean source
`44f6a45e2acef20dfda215555349da236c3ee1fd`, with CI passed. Installed ordinary
Chemistry, MDN, Britannica, and news reads passed their focused checks without
browser recovery. This is a usable improvement, not product completion. Do not
reinstall for documentation alone. See [STATUS.md](STATUS.md) for evidence and
remaining limits.

## Preserved follow-up: truthful, fast page reads

The reported cases have a bounded correction, installed and verified from clean
build `f99fb0acbb914d7146a2dd799535b2caae3839ce`. Keep PR #24 ready and unmerged.
Do not publish, repeat installation for documentation, or start another backlog
priority. Preserve the prior read and search-ad work, helpers, settings, schemas,
libraries and historical snapshots.

The selection copy now excludes inactive noscript payloads, supplementary
recently viewed UI, and narrowly identified inert filter controls. Reported
Lightpanda root navigation failures are rejected before content selection.
Existing single-attempt recovery and timeout settings are unchanged. No blanket
hidden-state rule or additional extraction engine was added.

Installed ASUS reading kept product sections and 108 links in 0.201 seconds.
Installed search returned five results from both providers in 0.785 seconds.
These are bounded observations, not speed guarantees. Amazon remains unavailable
after one fresh attempt. Website denials and robots restrictions are not bypassed.
Small ASUS UI labels remain. See [STATUS.md](STATUS.md) for evidence and rollback.
Further performance or extraction work needs a reproduced case and focused scope.

## Preserved follow-up: clean disclosure and overlay reading

Improve focused main-content reads on the existing branch and PR #24. Preserve
substantive collapsed sections and clean section boundaries. Exclude newsletter
and modal UI before the extractor loses the identifying wrappers. Use source
structure, not blanket visibility changes or prose keyword deletion. Keep
originals, explicit CSS, links, code/table values and historical snapshots intact.

Verify a bounded diagnostic article and retained real inputs, then practical
ordinary reads. Use the locked build and focused checks, followed by one supported
local installation with rollback. Keep the PR unmerged and do not publish. No
automatic clicking, access-control bypass, lazy-panel fetching, parser ladder,
new dependencies, search changes or broad corpus is approved.

The bounded proof and installation are complete from clean build
`c9ad49b6ecbbd9afde300cf24f335f8ccf2b9800`. The installed ordinary diagnostic and
FDA reads used HTTP without recovery. Remote acceptance remains pending. Do not
repeat installation for final documentation or start another backlog priority.

## Preserved follow-up: exclude search ads

The user approved paid/sponsored search-result exclusion on the existing branch
and PR #24. Check provider HTML context before result conversion and ranking.
Keep organic results, saved-library search, existing provider selection, ranking
rules, configuration, dependency pins and read-quality improvements. Keep the
PR unmerged. See [SEARCH.md](SEARCH.md) for the filtering boundary and limits.
The focused checks and installed ordinary search passed. Both configured default
providers returned results. This follow-up was verified from clean build
`51b42fffd958eda9f988244290ca7e690c7aef67`. No other deferred search interface or
backlog feature is approved. Do not repeat installation for final documentation.

## Preserved work: reliable, clean read

Issue [#23](https://github.com/JCFrags/webtool/issues/23), PR
[#24](https://github.com/JCFrags/webtool/pull/24), branch `feat/read-quality`.
The initial two-page workflow and the user-approved table/footer follow-up are
verified and installed locally. The follow-up used retained Chemistry input and
one fresh installed ordinary read. Remote acceptance is pending. The later
follow-ups above preserve this work.

- Select coherent main content without losing substantive sections, code, tables, references, captions, or discussion replies.
- Make ordinary read compact and readable. Keep provenance behind `--details`, full JSON, and retained originals.
- Recover likely JavaScript shells with one bounded configured Lightpanda attempt after HTTP. Keep explicit renderer choices and native routes.
- Preserve the Rust Book and JavaScript proof. Verify the table/footer follow-up with retained sources, then install the update with rollback.

Provider selection, ranking rules and configuration stay unchanged. No packaging,
release, broad test corpus, benchmark or other deferred feature work is approved.

## Additional backlog

The delivery plan now owns academic discovery, crawl resume/sitemaps, search
improvements, media capabilities, and the final optional-LLM milestone. The
following extensions are not prerequisites for that core delivery:

- Additional document formats, broad OCR coverage, and figure-asset export beyond the measured HTML/PDF paths.
- Full CommonMark support and broader notation/style handling beyond reproduced source-fidelity needs.
- GitHub issues/PRs/releases, revision comparison, and semantic repository navigation.
- Citation graphs, additional bibliography tools, and reference-manager integration.
- Provider-specific search interfaces beyond the explicitly supported filters.
- Portability and platform/linkage support beyond the verified environment.

## Completed baseline

The published `v0.1.0-alpha.1` retains the existing search, source readers,
libraries, crawling, citations, and shared-client setup. See [STATUS.md](STATUS.md)
for bounded evidence and limitations. Published tags and assets are immutable.
