# Architecture

## Execution and ownership

Rowtation uses NSH **0.4.0**, mixed template, with `rust: "."` and no `root`
in `nsh.json`. TSX runs on the browser JavaScript engine; Rust compiles to WASM.
No backend, cloud storage, persistent document storage or telemetry is introduced.

Rust owns the document, all format parsing/serialization and resource validation.
TSX owns presentation and transient focus, selection, dialogs and editing drafts.
Returned snapshots may be mirrored in UI signals; they are not a second dataset.
The import UI, virtualized inspection table and Rust data operations are implemented.
JSON, NDJSON and tabular XML adapters are implemented; Excel and browser downloads remain planned. See [structured mapping and reconstruction metadata](structured-imports.md).

See the [data operations contract](operations.md) for ordering, matching,
revisions, dirty state, mutation methods and resource limits.
`src/operations.rs` owns stable ordering, retained matches, sparse baseline values
and bounded before/after transactions. `DocumentSession` exposes `sort`, `search`,
`searchMatch`, `searchIndex`, `searchSnippet`, `rowPosition`, `editStatus`, `editCell`, `replaceMatches` and `history`
through the registered API. Mutations invalidate cached and in-progress profiles.

`#[native_module("rowtation/native")]` registers `DocumentSession` using
`Registry::class`. `#[class]`, `#[derive(JsObject)]`, `#[derive(PropEnum)]` and
synchronous `Result<T, E>` use actual NSH 0.4.0 APIs, checked against resolved
`nsh-core`/`nsh-macros` sources and generated `.nsh/types/rowtation/native.d.ts`.
No JSON-string protocol or separate WASM bridge is used.

## Modules and canonical model

- `src/document/`: canonical cells, ordered columns, retained rows, stable identity,
  counts, revision/dirty metadata and borrowed windows of at most 256 rows.
- `src/formats/delimited.rs`: UTF-8 import, schema normalization and serialization,
  without size caps. Uses `csv` 1.4.0 with an in-memory byte slice and byte writer;
  no filesystem, networking or threading APIs are used by the adapters.
- `src/formats/records.rs`: an allocation-free quote/boundary scanner. The mature
  `csv` crate decodes/encodes fields; the scanner supplies strict quote validation
  and retains physical blank records that the crate normally skips.
- `src/error.rs`: recoverable code/message/location errors.
- `src/session.rs`: the retained NSH class and small typed transport records.
- `tests/document_engine.rs`: synthetic data fixtures; `tests/fixtures/DocumentSmoke.tsx`
  calls the bridge inside the app runtime, including WASM in browser tests.

`CellValue` is `Missing | Null | Text(String) | Boolean(bool) | Number(String)`.
Numbers store exact lexical text to avoid floating-point precision loss. CSV only
creates Text; it never infers scalars or evaluates formulas/scripts/markup. Structured adapters add per-row shape metadata referencing leaf column IDs without
changing document ownership or cell identity. No adapter registry is introduced.

Columns have stable IDs, names and optional original header names. Empty and
duplicate headers are preserved. Absent headers and extra fields generate names.
Rows store only their present fields: an empty string is Text(""), while an absent
trailing field reads as Missing. Null is a distinct canonical value produced by JSON and NDJSON. Ragged rows extend the schema; nothing is dropped or padded into
empty strings. Blank records are one empty text field. Empty input is zero-by-zero.

Document IDs are runtime-unique strings, not persisted IDs. Row and column IDs are
opaque `u32` identities scoped to a document. The session supplies document context;
UI keys and selection must retain document ID as well as row/column IDs. Backing storage assigns monotonically increasing IDs and never reorders
rows. Sorting orders row identities in a Rust view. Reads are O(1) by ID.

Imported documents start clean at revision zero. Reads, sorting, search and export
do not change either. Mutations update revision/dirty in Rust; a UI generation
invalidates bounded grid windows after mutations and ordering/search changes.

## Public Rust API

`formats::delimited::parse(&[u8], &str, bool) -> Result<Document, EngineError>`
imports bytes with explicit delimiter and headers. `serialize(&Document, &str,
bool) -> Result<Vec<u8>, EngineError>` serializes without changing the document.

`Document` exposes `id`, `columns`, `row_count`, `column_count`, `source_delimiter`,
`source_headers`, `revision`, `dirty`, `window(offset, limit)` and
`cell(row_id, column_id)`. Columns expose ID/name/original name. Rows expose ID and
stored cells. Dataset mutations are exposed by the retained session; construction
from arbitrary external cells is not exposed. `window` and `cell` borrow storage without cloning. See Rust doc comments for exact
signatures. The `session` module exposes the NSH transport API
for Rust callers too; its records implement Default.

## TSX API contract

NSH generates camelCase names and interfaces from Rust. Methods are synchronous;
long work blocks the browser thread. There is no worker or async-work claim.

```tsx
import { cleanup } from "nsh";
import { DocumentSession, ImportSource } from "rowtation/native";

const source = ImportSource.fromText(text, "Pasted data"); // or BrowserImport.take(requestId)
const analysis = source.inspect();
const session = DocumentSession.fromSource(source, { delimiter: analysis.delimiter, headers: true });
source.dispose(); // the session keeps its own data
cleanup(() => session.dispose()); // inside the owning component
const metadata = session.metadata();
const columns = session.columns();
const window = session.rows({ offset: 0, limit: 64 });
if (window.rows.length && columns.length) {
  const cell = session.readCell(window.rows[0].id, columns[0].id);
}
const output = session.serialize({ delimiter: ",", headers: true });
// output is Uint8Array; download UI is a later milestone.
```

| API | Contract |
| --- | --- |
| `new DocumentSession(input: Uint8Array, options: ImportOptions)` | Atomic import; options `{ format?: string, delimiter?: string, headers?: boolean }`, defaults comma/true. Failed creation does not affect another session. |
| `DocumentSession.fromSource(source: ImportSource, options)` | Same import from Rust-held bytes, without copying them through JS. The source is kept, so a failed import can be retried. |
| `ImportSource.fromText(text, name)`, `source.inspect()`, `source.name`, `source.byteLength` | Bytes of one import candidate held in Rust. `inspect` equals `ImportDetector.inspect`. Dispose when review ends. |
| `BrowserImport.take(requestId)` / `readDeferred(requestId)` | Take the `ImportSource` a delivered `LocalImport` read, once; or read a file delivered with `deferred: true` (above 8 MiB, unread) after confirmation. `cancelPending()` drops both and stops a read at its next 16 MiB slice. |
| `profileStep(columnId, rows): ProfileProgress` | Count up to `rows` more rows of a column's exact statistics; `profile` is set when done. `profile(columnId)` runs to completion. |
| `cancelProfile(): void` | Drop an unfinished `profileStep` scan and its distinct-value table; finished profiles stay cached. The view calls it when the selected column changes, and disposal frees it too. |
| `metadata(): DocumentMetadata` | `{ id, format, rowCount, columnCount, revision, dirty, delimiter, headers }`; no row copies. |
| `columns(): ColumnMetadata[]` | Ordered `{ id, name, originalName }`; originalName is null for generated columns. |
| `rows({ offset, limit, columnOffset?, columnLimit?, previewBytes?, measure? }): RowsSnapshot` | `{ documentId, revision, offset, columnOffset, rows: [{ id, cells }], rustMs }`; `rustMs` is set only when `measure` is true: Rust's own build time, excluding the copy into JS; cells follow schema order from columnOffset, including missing trailing values. Without previewBytes whole text/number values are copied; with it each value is cut to that many UTF-8 bytes on a character boundary and marked `truncated`. `byteLength` is always the whole length. `matched` marks cells in the active search. At most 256 rows; offsets are positions, not IDs. |
| `readCell(rowId: number, columnId: number): CellSnapshot` | Full value using document-scoped IDs obtained from `rows()`/`columns()`. `{ kind, text, boolean, number, byteLength, truncated, matched }` (never truncated). Only the payload for kind is populated, others null. `kind` is `missing`, `null`, `text`, `boolean` or `number`. |
| `serialize(options: ExportOptions): Uint8Array` | Optional delimiter/headers default to source settings. Whole document only; no filtered export or download. |
| `sort(columnId, descending): void` | Stable view order; null column resets. Does not change data revision. |
| `search(options): SearchResult` / `searchMatch(index)` / `searchIndex(rowId, columnId, start, hint)` / `searchSnippet(index, previewBytes)` | Retain scoped occurrence IDs/ranges in Rust. Return the count and one navigable match. `searchIndex` finds a retained occurrence in one call. `searchSnippet` returns that slice, or empty when it sits inside the preview, at most 512 bytes. |
| `editCell(rowId, columnId, text, revision)` | Literal text commit using stable IDs and expected revision; same-value commits are no-ops. |
| `replaceMatches(index, text, revision)` | Replace one occurrence, or all retained matches when index is null; one transaction and exact counts. |
| `history(redo): MutationResult` / `editStatus()` | Undo/redo one bounded transaction; report dirty cell count and history availability. |
| `rowPosition(rowId): number` | Locate an existing identity in the current view for selection/scrolling. |
| `dispose(): void` | NSH frees the Rust value immediately. Calls after disposal throw TypeError. Garbage collection is a fallback; explicitly dispose when closing/replacing a document. |

The inspection viewport keeps only its requested windows, keys rows by stable
identity, and reads changing signals inside JSX/closures. The grid caches up to
256 rows with overscan for the columns in view, with 512-byte previews. Scrolling
away from cache boundaries avoids bridge calls; refills reuse overlapping snapshots
and formatted row objects. Document identity, revision and column-window changes
invalidate reuse. The inspector copies or shows the whole value through `readCell`. Profile common values are 256-byte previews
with exact counts and whole byte lengths.
Import UI must keep the old session until a new constructor succeeds, then dispose
the old one. Operations accept stable IDs and revision expectations
rather than JS-owned row arrays. The implemented mutation methods enforce this.

## CSV/TSV and serialization policies

Low-level parser: UTF-8 only, optional single initial BOM; explicit delimiter and
headers, without encoding or format detection. Accept
single ASCII delimiters except NUL, quote, CR and LF. A tab selects TSV. Header
mode is explicit. Preserve whitespace, text, Unicode, doubled quotes, embedded
newlines and delimiters. Accept LF, CRLF and bare CR record separators. Reject
unclosed quotes, quotes inside unquoted fields and characters after closing quotes;
spaces after closing quotes are errors. Backslash is not an escape character.

Serialization uses standard necessary quoting (or always quotes when protecting a
literal leading BOM), doubled quotes, LF separators and no transport BOM. It preserves cell values and row/column order, not source bytes or quote
style. Trailing Missing fields are omitted to preserve short rows; interior Missing,
Null and typed scalars fail with LOSSY_EXPORT until explicit conversion policies
exist. With headers enabled, generated names become actual headers; original-name
provenance is not encoded in CSV. Empty documents serialize to empty bytes.
Formula-like text stays literal: the engine never evaluates it, and does not add
spreadsheet escaping that would change field values. A future download/conversion
UI must explain spreadsheet safety and lossy mappings before altering text.

## Errors and resource lifecycle

Rust returns `EngineError` with `code()`, `message()`, `line()` and `record()`.
Locations, where available, are one-based physical lines and logical records,
including the header and blank rows. Invalid UTF-8 reports the byte offset after
initial BOM removal. NSH synchronous Result converts failures to JS Error whose
message starts with `CODE:`. This version does **not** attach a structured JS
`error.code`; consumers may match the prefix. Binding argument errors and use after
disposal are NSH TypeErrors, distinct from engine errors.

Codes: INVALID_FORMAT, INVALID_JSON, INVALID_XML, INVALID_STRUCTURE, INVALID_DELIMITER, INVALID_UTF8, INVALID_CSV, INVALID_WINDOW, UNKNOWN_ROW,
UNKNOWN_COLUMN, LOSSY_EXPORT, SERIALIZE_FAILED, ID_EXHAUSTED, INVALID_REGEX,
SEARCH_LIMIT, EDIT_LIMIT, STALE_REVISION, UNKNOWN_MATCH and REVISION_LIMIT. Failed operations
return no partial dataset/output and do not change a live document. No untrusted
input path uses unsafe code, unwrap or expect.

There are no caps on import size, rows, columns, stored fields, value length or
serialized output. Search/edit requests have [operation limits](operations.md).
Row windows are limited to 256 rows, which keeps
virtualized reads incremental. End-of-document windows contain the remaining rows;
offset equal to rowCount and zero limits are valid empty windows. Larger offsets
or limits fail with INVALID_WINDOW. Memory exhaustion is not a recoverable engine
error: a WASM allocation failure aborts the module, and the browser may close the tab.

`ImportDetector.inspect` measures the whole input before import and reports, for
comma and tab separately, records, widest record, total fields, the longest raw
field and a `large` flag. The flag is set above 8 MiB of input, 100,000 records,
512 columns, 1,000,000 fields or a 1 MiB field (`inspection::LARGE_*`). The review
dialog then shows a warning and renames its action to "Import anyway"; nothing is
rejected.
A picked file is read in 16 MiB slices directly into one Rust buffer, and pasted
text is encoded once in Rust; `ImportSource` keeps those bytes until import, so JS
never holds a second copy. Returned snapshots/bytes are copied across the bridge. The engine retains decoded fields, not the original file or a dataset
clone. Only requested windows are copied; no whole-dataset JS cache or worker architecture is added.
Dispose drops the document and all retained rows/strings. WASM's allocator may keep
freed linear-memory pages for reuse; disposal does not promise the browser shrinks
its heap. Multiple sessions can coexist.

## Verification and remaining work

Rust tests cover parsing, invalid syntax/encoding, headers, empty/missing values,
leading zeros, Unicode, round trips, identity, windows and lifecycle, imports past
former caps (1 MiB values, 513 columns, 100,001 rows), previews on character
boundaries, column ranges, sliced profiles, import sources, and the large-input
measurements and flags. Inputs of tens of megabytes (a 32 MiB value, over a
million fields, 20,000-column windows) and the benchmark live in the ignored
`tests/stress.rs`. The NSH smoke tests cover typed results/arguments,
failed constructors, serialization, and explicit disposal, with a test-only app
entry to exercise the actual browser/WASM bridge, including a value over 1 MiB returned whole, as a preview
and by its stable-ID read. Production UI tests open `App.tsx` and cover paste import, replacement after failure,
responsive inspection, keyboard selection and offscreen row-window recycling.

Verify `cargo fmt --check`, `nsh check`, `nsh build`, `nsh test`,
`scripts/test-browser.sh` and `nsh package --target web -o dist` for changes. A web
package verifies the in-memory csv dependency path compiles to wasm32; browser
smoke tests verify runtime compatibility. The large-input thresholds are not
large-file responsiveness benchmarks. Measure representative browsers before
adding workers. Safe download policy and structured export remain later work. Browser file and layout regressions run through the synthetic host
used by scripts/test-browser-files.sh.

## Import and inspection UI

`App.tsx` mounts the responsive screen. TSX owns the current document handle, import draft,
dialog visibility, loading/errors and selection. The production entry imports no
test host. No document history is retained. A document view is keyed by document
ID, and its owner disposes the Rust session when closed/replaced. Cleanup is
idempotent so root disposal and view disposal cannot free it twice.

`ImportSource.inspect()` (or `ImportDetector.inspect(bytes, filename)`) scans up to 32 logical records, counting
commas/tabs outside quoted text. The filename breaks ties only. Mixed separators
or no separators are marked ambiguous. It validates UTF-8 and measures the whole
input for the large-import warning; the existing strict parser validates the complete file on import. The review step exposes a format selector; comma/tab and the header toggle appear only for delimited input. No scalar or header inference is
added. A failed constructor leaves the old session untouched.

`BrowserImport.onFile(callback, onDrag)` owns a hidden local file input and document
drag/drop listeners. Its returned stop removes the input, handlers and closures;
in-flight reads check lifetime/generation before delivery. `cancelPending()`
invalidates that same generation on UI cancellation or workflow changes. Each
file result carries an opaque `requestId`; TSX checks `isCurrent(requestId)` again
when a queued callback arrives. Rejected files clear obsolete review drafts and
report the failing filename; parser errors keep only the named current draft so
its separator/header options can be corrected. `pick()` clicks that
input synchronously in the user's press. Bytes are read through browser File APIs
in Rust/WASM without a size cap. Only one supported CSV/TSV, JSON, NDJSON/JSONL or XML file is accepted. These APIs use registered NSH modules and generated
interfaces. Target-specific web-sys/wasm-bindgen dependencies add no native target.

`components/DataGrid.tsx` styles NSH's DataTable. It supplies `rowCount`, `dataStart`
and only the visible row range plus four rows on each side, split into requests of
at most 256 rows. The table virtualizes both rows and columns; sticky headers, two-axis
scrolling, hit testing, focus and keyboard navigation belong to NSH. TSX translates
only the current window's cells to display text. Selection retains document, row
and column IDs independently of rendered cells. Keyboard jumps can read a single
row before the visible-range event arrives. The inspector shows the selected
value with its byte size; the grid opens a draft editor by stable ID. Rows are 34 points on pointer
desktops at least 900 points wide and 44 points elsewhere.

`DocumentSession.profile(columnId)` computes exact distinct/empty/missing counts
and the top five nonempty values in Rust. Empty text is a distinct present value;
missing fields are excluded from distinct counts. Frequency maps borrow the
stored strings and are dropped after computation. The session caches at most
eight small summaries and recomputes an evicted column on demand. Selection within
a column does not rescan it. Common values are returned whole, and ties use
lexical text order. CSV/TSV and XML values remain Text; JSON/NDJSON profiles include canonical types.

The desktop inspector uses a stationary rounded frame around an independent
scroll view. Dialogs have explicit centered desktop positioning and bottom panel
positioning below 640 points, with a scrollable content region and safe-area
padding. The inspector moves to a contextual panel below 900 points.

NSH 0.4's web desktop backend draws a horizontal indicator even when the inner
DataTable scroller disables it, alongside DataTable's bottom viewport scrollbar.
That indicator is a DOM node with a very high `z-index`, and the open document
is painted after Diagnostics, so the bar shows through the panel.
`BrowserStyles.install(css)` installs/removes a scoped app stylesheet that gives
the grid and the inspector scroller their own stacking context, places
Diagnostics above the document, and suppresses only the disabled inner indicator
beneath `data-grid`. TSX owns that CSS; Rust only attaches it. This compatibility rule
is tied to the resolved 0.4 DOM classes and should be checked visually when
upgrading NSH. It does not modify the NSH runtime or disable scrolling. File
contents never enter CSS, markup or executable code.
