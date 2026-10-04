# Document reading and source fidelity

Ordinary document reads use no language model. The CLI sends uploads and URL
reads to the same server. The CLI does not open the server's database.

The server retains original bytes before parsing. Use a saved ID for later reads,
`find`, extraction, and export. A parser change creates a new snapshot identity.
It does not rewrite older saved documents. Repeated acceptance of identical bytes
with the same parser and configuration keeps the same ID.

```sh
webtool ingest notes.md
webtool read SAVED_ID --details
webtool --format markdown read SAVED_ID
webtool extract SAVED_ID tables
webtool export SAVED_ID --kind original --output original.bin
```

`ingest --name` accepts a filename, not a URL or path. An upload has no public URL
base for relative links. Original export takes a file path. `--output -` does not
mean stdout. Select `--force` only when replacement of an existing file is intended.

## Markdown

The reader uses pinned `pulldown-cmark` 0.13.4. It handles CommonMark headings,
paragraphs, escapes, entities, tight and loose lists, nesting, block quotes,
reference links, images, thematic breaks, and fenced or indented code. Enabled
extensions add tables, task lists, strikethrough, footnotes, and dollar-delimited
mathematical notation. Smart punctuation is disabled. Notation is not calculated.

Blocks retain original line ranges. Semantic text can decode entities and normalize
code line endings to LF. Exact original bytes remain available. Optional metadata
retains link/style ranges, list positions, quote depth, and footnote labels.

Markdown reads and exports use exact source slices for complete, unchanged block
groups. These slices preserve syntax such as table alignment, link definitions,
image titles, hard breaks, and nested formatting. If a passage contains only part
of a group, rendering uses the selected blocks instead. It does not add omitted
blocks. Invalid style ranges also fall back to ordinary text.

Raw HTML stays source syntax and is not executed by this reader. Raw HTML blocks
produce a warning. More than 64 nested Markdown containers remain literal source
with a depth-limit warning. This is not a Markdown editor or a sandbox for rendering
exported HTML in another application.

## Small Office-format set

Build the server with `documents` for DOCX, PPTX, XLSX, and native-text PDF reading.
The standard server enables this feature. Pinned Xberg 1.1.1 supplies content and
structures. No LibreOffice process, workstation-specific converter, or second
extraction engine is used.

- DOCX: supplied headings, paragraphs, code, formula notation, tables, selected
  inline styles, and links. Unique source numbering matches preserve decimal
  starts and nested bullets. Unsupported labels, style-inherited numbering,
  ambiguous matches, and unavailable levels produce warnings, not guessed numbers.
  DOCX blocks remain `derived` locations. Upstream page fields are not exact Word
  pagination or XML positions.
- PPTX: supplied slide text and speaker notes. Internal presentation and notes
  relationships corroborate slide numbers. A complete source text match must be
  unique. If an upstream note/slide association conflicts with its source
  relationship, the source-confirmed slide is used with a correction warning.
  Uncorroborated notes keep their text, use a derived position, and show a warning.
  Exact shape layout and drawing export are not implemented.
- XLSX: one supplied table per extracted grid, with its sheet name. Hidden-sheet
  labels remain visible. Zero, empty text, cached formula values, and literal
  formula-looking strings remain separate supplied values. Formula metadata records
  whether a cache exists. No formula is calculated and no external workbook is
  fetched. Locations identify a sheet, not exact cell addresses. Formatting and
  number-display conventions are not a complete spreadsheet preview.

Header roles in Office tables are upstream estimates, not independently verified
source roles. Missing grid entries are not treated as explicit empty cells. A
missing, ambiguous, unsupported, or inconsistent structured result keeps supplied
page text and labeled table supplements with a warning. Full upstream results,
processing warnings, and original bytes remain available.

The small OOXML corroboration reader uses existing lockfile dependency `zip` 8.6.0
and the existing XML parser. It reads only internal archive members in memory.
It does not extract files, follow external relationships, or fetch link targets.
XML reads enforce the smaller of the application source-byte limit, the existing
25 MiB decoded limit, and the configured Xberg content/archive limits. Reads also
check actual decoded bytes, archive entry count, declared total size, and compression
ratio. Missing or unsupported XML produces an explicit corroboration warning.

## Native PDF and HTML

PDF page text keeps Xberg's reading order. Native tables are heuristic supplements.
Aligned prose can be misidentified as a table. A supplied native bounding box is
used only for a unique complete text match. No coordinate or reading-order repair
is guessed. `document_config` can request Xberg's native hierarchy and bounding
boxes, but those structures do not by themselves establish correct columns or tables.
Blank or unreadable page objects remain warnings, not successful page reads.

HTML uses the existing single content selector. Selected complete runs can recover
bold, italic, inline code, strikethrough, and underline ranges from a unique matching
original element. Recovery copies ranges, not missing paragraphs or source subtrees.
A selection-copy carrier preserves main/article image references only when that
carrier survives selection. References are not downloaded figures. Original HTML,
explicit selectors, all-page link discovery, and access gates remain unchanged.
Mathematical notation, script text, code, tables, captions, and article qualifications
continue to use the source-preserving rules in `AGENTS.md` and [ENCODING.md](ENCODING.md).

## Practical scope and limits

Authored small inputs are in `crates/engine/tests/fixtures/documents/`.
Run `make_inputs.py` in a new diagnostic directory to create deterministic Office
and native PDF inputs and a hash manifest. The bounded checks cover:

- `commonmark.md`: syntax, nesting, source slices, link definitions, styles, math,
  image references, zero/empty table cells, footnotes, and literal raw HTML.
- `field-notes.docx`: heading, bold/italic, start 3 with a nested bullet, source link,
  zero/empty cells, and a qualification.
- `field-notes.pptx`: two slides and a note linked from slide 2 to `notesSlide1.xml`.
- `field-notes.xlsx`: zero, empty, cached 42, literal `=SUM(A2,99)`, and a hidden sheet.
- `native-text.pdf`, `native-columns.pdf`, `native-partial.pdf`: exact text/original
  comparisons, page order, heuristic-table disclosure, and an unreadable second page.
- `fidelity.html`: selected styles, source math, image reference, caption, code,
  zero/empty cells, article qualification, and excluded page chrome.

These inputs are not a broad Office/PDF corpus or a universal layout test.
Image-only scans need separately prepared OCR backends and models. No OCR/model
readiness, scanned tables, general multicolumn accuracy, arbitrary Office files,
legacy Office, OpenDocument, or downloadable figures are accepted by these checks.
The existing optional format routes are not additional verified-format claims.
