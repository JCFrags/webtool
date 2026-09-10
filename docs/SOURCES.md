# Dependency and implementation references

These references supported API inspection, not runtime or quality certification.
No upstream repository was merged wholesale into this snapshot.
Dependencies are declared through Cargo, and helper executables remain externally installed tools.

| Component | Selected dependency or reference |
|---|---|
| Rust HTTP application | https://docs.rs/axum/latest/axum/ |
| Runtime and subprocesses | https://docs.rs/tokio/latest/tokio/ |
| Command parsing | https://docs.rs/clap/latest/clap/ |
| HTTP client | https://docs.rs/reqwest/latest/reqwest/ |
| SQLite binding | https://docs.rs/rusqlite/latest/rusqlite/ |
| SQLite full-text search | https://www.sqlite.org/fts5.html |
| Native web content selection | https://github.com/Murrough-Foley/rs-trafilatura |
| Native metasearch adapters | https://github.com/MikeLuu99/metasearch-rust |
| Optional fastCRW renderer | https://github.com/us/crw |
| fastCRW renderer contract | https://github.com/us/crw/blob/main/crates/crw-renderer/src/traits.rs |
| Xberg public API | https://github.com/xberg-io/xberg/blob/main/plugin/skills/xberg/references/rust-api.md |
| Xberg page structures | https://github.com/xberg-io/xberg/blob/main/crates/xberg/src/types/page.rs |
| Xberg table structures | https://github.com/xberg-io/xberg/blob/main/crates/xberg/src/types/tables.rs |
| Xberg feature selection | https://github.com/xberg-io/xberg/blob/main/crates/xberg/Cargo.toml |
| Lightpanda helper | https://github.com/lightpanda-io/browser |
| Media helper | https://github.com/yt-dlp/yt-dlp |
| Feed parsing | https://docs.rs/feed-rs/latest/feed_rs/ |
| CSV processing | https://docs.rs/csv/latest/csv/ |
| GitHub repository metadata | https://docs.github.com/en/rest/repos/repos |
| GitHub repository contents | https://docs.github.com/en/rest/repos/contents |
| DOI content negotiation | https://www.crossref.org/documentation/retrieve-metadata/content-negotiation/ |

Selected direct integration pins are recorded in `crates/engine/Cargo.toml`.
The remaining dependency ranges are not transitively locked until Cargo generates a real lockfile.
The Xberg reference text described version 1.1.0, while the selected crate pin is 1.1.1.
Verify source compatibility during the first integration build.

fastCRW's engine is AGPL-licensed.
The application snapshot carries an AGPL-3.0-or-later notice.
Other dependencies retain their respective licenses.
Complete license texts and model licenses need inclusion in a packaged binary release.
