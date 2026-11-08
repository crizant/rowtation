# Product requirements

## Purpose and current status

Rowtation converts structured data in the browser. The workflow is import →
inspect → transform → export. The Rust document core and CSV/TSV
parser/serializer and CSV/TSV import/inspection are implemented. Sorting, search, editing, find/replace and bounded undo/redo are implemented;
other formats and download workflows below remain planned. See the
architecture for the implemented value, header, delimiter and resource policies.

## Implemented import and inspection

- Local .csv/.tsv files by picker/drop, or pasted UTF-8 CSV/TSV text.
- Rust delimiter suggestion, explicit header/separator review, actionable parsing
  errors, loading and empty states, atomic document replacement.
- Virtualized two-axis table with row numbers, sticky headers, cell selection and
  keyboard navigation. Values remain literal text; long values use grid previews.
- Desktop column inspector and narrow-browser contextual panel: exact unique,
  empty and missing counts, five common values, source settings and selection.
- In-memory active document only. No recent history or persistence; closing/replacing it
  disposes its retained Rust session. No upload or backend.

## Planned import extensions

- Accept pasted text, dropped files and an explicit Open action.
- Support CSV, TSV, JSON, NDJSON, XML and Excel. Proposed Excel input scope is
  `.xlsx` and `.xls`; verify crate and WASM support before promising either.
- Show the detected format and relevant parsing options. Allow correction when
  detection is ambiguous; avoid silently changing the data.
- Report malformed input with actionable location details when available.
- Offer worksheet selection for workbooks with multiple sheets.

## Implemented table operations

- Show headers, row counts and literal text values in a virtualized table.
- Sort columns with a visible direction and deterministic ordering.
- Search data with visible matching rows or cells and a result count.
- Edit cells without losing row identity when sorting or searching changes.
- Find and replace with visible match scope/count and separate Replace and Replace all
  actions, with exact affected-cell and occurrence counts after replacement.
- Show unsaved edits and preserve the document after a failed operation.
- Bounded transaction undo/redo and lazy revision-aware column statistics.

See [operation semantics and limits](operations.md) for exact matching, sorting,
replacement and history behavior.

## Planned export

- Choose output format, filename and options; download the serialized bytes.
- Target CSV, TSV, JSON, NDJSON, XML and Excel output. Excel output starts with
  `.xlsx`; legacy `.xls` export is undecided and must not be advertised yet.
- Make export scope explicit: complete dataset or current filtered view.
- Explain lossy mappings before export; do not silently discard nested values,
  additional worksheets, types or unsupported fields.
- Keep numeric precision and text such as leading-zero identifiers intact unless
  the user explicitly requests conversion.

## Data policy and constraints

- Process data locally in Rust/WASM; no upload or backend is required.
- Do not add telemetry, external data transmission or persistent document storage
  without a product decision. Local processing is a design requirement, not yet
  an audited privacy guarantee for a finished application.
- Never execute imported scripts, XML external entities, macros, workbook formulas
  or HTML. Treat imported contents as data.
- Handle formula-like spreadsheet text explicitly on export. A safe text mode
  must explain any escaping that changes output values.
- Imports have no size caps. Large input (above 8 MiB, 100,000 records, 512
  columns, 1,000,000 fields or a 1 MiB field) requires explicit confirmation;
  files above 8 MiB are confirmed before they are read.
  Measure representative browsers before claiming large-file support.

## Decisions needed during implementation

- JSON/XML nesting and attribute mapping, flattening and reconstruction rules.
- Null versus empty string versus absent field; mixed-type columns and coercion.
- Header detection, encodings, delimiters and malformed-row recovery.
- Excel dates, formulas versus cached values, multiple sheets and precision.
- Export ordering for formats that are not implemented yet.
- Persistence is not a current commitment. Search, sort, replacement and undo
  behavior is specified in operations.md.

Record these decisions with fixtures and tests before extending format support.
