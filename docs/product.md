# Product requirements

## Purpose and current status

Rowtation converts structured data in the browser. The workflow is import →
inspect → transform → export. The Rust document core and CSV/TSV
parser/serializer and CSV/TSV import/inspection are implemented. Sorting, search, editing, find/replace and bounded undo/redo are implemented;
JSON, NDJSON and tabular XML imports are also implemented; five-format export, download and copy are implemented. See [structured import rules](structured-imports.md). See the
architecture for the implemented value, header, delimiter and resource policies.

## Implemented import and inspection

- Local CSV/TSV, JSON, NDJSON/JSONL and XML by picker/drop or UTF-8 paste.
- Rust delimiter suggestion, explicit header/separator review, actionable parsing
  errors, loading and empty states, atomic document replacement.
- Virtualized two-axis table with row numbers, sticky headers, cell selection and
  keyboard navigation. Values keep canonical types; long values use grid previews.
- Desktop column inspector and narrow-browser contextual panel: exact unique,
  empty, null and missing counts, five common values, source settings and selection.
- In-memory active document only. No recent history or persistence; closing/replacing it
  disposes its retained Rust session. No upload or backend.

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

## Implemented export

CSV, TSV, JSON, NDJSON and supported XML output, original/current sorted order,
explicit mapping warnings, formula-safe delimited text, filename, download and
copy. Search is highlighting, not filtering. Export keeps dirty state and history.
See [conversion policies and limitations](export.md).

Excel remains a possible future feature only.

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

## Future scope

Excel requires a separate compatibility and product decision. Persistent storage,
session history, automatic header/type inference, additional encodings and
malformed-input recovery are not current commitments. Specify behavior and add
synthetic fixtures before extending support.
