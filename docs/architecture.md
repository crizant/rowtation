# Architecture

## Execution and ownership

Rowtation uses NSH **0.4.0**, mixed template, with `rust: "."` and no `root`
in `nsh.json`. TSX runs on the browser JavaScript engine; Rust compiles to WASM.
No backend, cloud storage, persistent document storage or telemetry is introduced.

Rust owns the document, all format parsing/serialization and resource validation.
TSX owns presentation and transient focus, selection, dialogs and future drafts.
Returned snapshots may be mirrored in UI signals; they are not a second dataset.
The import UI and virtualized inspection table are implemented. Sorting/searching,
editing, replacement, other format adapters and browser downloads remain planned.

`#[native_module("rowtation/native")]` registers `DocumentSession` using
`Registry::class`. `#[class]`, `#[derive(JsObject)]`, `#[derive(PropEnum)]` and
synchronous `Result<T, E>` use actual NSH 0.4.0 APIs, checked against resolved
`nsh-core`/`nsh-macros` sources and generated `.nsh/types/rowtation/native.d.ts`.
No JSON-string protocol or separate WASM bridge is used.

## Modules and canonical model

- `src/document/`: canonical cells, ordered columns, retained rows, stable identity,
  counts, revision/dirty metadata and borrowed windows limited by row/cell counts.
- `src/formats/delimited.rs`: UTF-8 import, schema normalization, serialization and
  input/output limits. Uses `csv` 1.4.0 with an in-memory byte slice and byte writer;
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
creates Text; it never infers scalars or evaluates formulas/scripts/markup. Future
JSON/NDJSON/XML/Excel adapters can add mapping rules and composite values without
changing document ownership or cell identity. No adapter registry is introduced.

Columns have stable IDs, names and optional original header names. Empty and
duplicate headers are preserved. Absent headers and extra fields generate names.
Rows store only their present fields: an empty string is Text(""), while an absent
trailing field reads as Missing. Null is a distinct canonical value reserved for
future adapters. Ragged rows extend the schema; nothing is dropped or padded into
empty strings. Blank records are one empty text field. Empty input is zero-by-zero.

Document IDs are runtime-unique strings, not persisted IDs. Row and column IDs are
opaque `u32` identities scoped to a document. The session supplies document context;
UI keys and selection must retain document ID as well as row/column IDs. Current
immutable backing storage assigns monotonically increasing IDs and never reorders
it. Future sort/filter operations should order row IDs in a view, preserving backing
rows and IDs. Never reuse deleted IDs when mutation is added. Reads are O(1) by ID.

Imported documents start clean at revision zero. Reads, schema queries and export
do not change either. Future edits must update revision/dirty in Rust and define
how the UI rejects stale windows; no mutation or undo system is implemented here.

## Public Rust API

`formats::delimited::parse(&[u8], &str, bool) -> Result<Document, EngineError>`
imports bytes with explicit delimiter and headers. `serialize(&Document, &str,
bool) -> Result<Vec<u8>, EngineError>` serializes without changing the document.

`Document` exposes `id`, `columns`, `row_count`, `column_count`, `source_delimiter`,
`source_headers`, `revision`, `dirty`, `window(offset, limit)` and
`cell(row_id, column_id)`. Columns expose ID/name/original name. Rows expose ID and
stored cells. Dataset mutation and construction from arbitrary external cells are
not exposed yet. `window` and `cell` borrow storage without cloning; text-size caps
apply only to the session's copied previews. See Rust doc comments for exact
signatures. The `session` module exposes the NSH transport API
for Rust callers too; its records implement Default and are non-exhaustive.

## TSX API contract

NSH generates camelCase names and interfaces from Rust. Methods are synchronous;
long work blocks the browser thread. There is no worker or async-work claim.

```tsx
import { cleanup } from "nsh";
import { DocumentSession } from "rowtation/native";

const session = new DocumentSession(bytes, { delimiter: "\t", headers: true });
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
| `new DocumentSession(input: Uint8Array, options: ImportOptions)` | Atomic import; options `{ delimiter?: string, headers?: boolean }`, defaults comma/true. Failed creation does not affect another session. |
| `metadata(): DocumentMetadata` | `{ id, rowCount, columnCount, revision, dirty, delimiter, headers }`; no row copies. |
| `columns(): ColumnMetadata[]` | Ordered `{ id, name, originalName }`; originalName is null for generated columns. |
| `rows({ offset, limit }): RowsSnapshot` | `{ documentId, revision, offset, rows: [{ id, cells }] }`; cells follow schema order, including missing trailing values. Text/number payloads are bounded previews with explicit `truncated` and original `byteLength`. Offset is a position, not an ID. |
| `readCell(rowId: number, columnId: number): CellSnapshot` | Full value using document-scoped IDs obtained from `rows()`/`columns()`. `{ kind, text, boolean, number, truncated, byteLength }`; `truncated` is always false. Only the payload for kind is populated, others null. `kind` is `missing`, `null`, `text`, `boolean` or `number`. |
| `serialize(options: ExportOptions): Uint8Array` | Optional delimiter/headers default to source settings. Whole document only; no filtered export or download. |
| `dispose(): void` | NSH frees the Rust value immediately. Calls after disposal throw TypeError. Garbage collection is a fallback; explicitly dispose when closing/replacing a document. |

The inspection viewport keeps only its requested windows, key rows by stable
identity, and read changing signals inside JSX/closures. Render cell payloads as
previews: when `truncated` is true, the payload is a prefix, never a replacement
value or an editable draft. Use the row/column IDs to call `readCell()` when the
full value is needed, including cells larger than the window byte budget. A
preview's `byteLength` reports the full UTF-8 text/number length (zero for other
kinds). Prefixes do not include synthetic ellipses; the UI can indicate truncation.
Import UI must keep the old session until a new constructor succeeds, then dispose
the old one. PR #3 operations should accept stable IDs and revision expectations
rather than JS-owned row arrays. These are ownership contracts, not implemented editing methods.

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

Codes: INVALID_DELIMITER, INVALID_UTF8, INVALID_CSV, INPUT_LIMIT, COLUMN_LIMIT,
DATA_LIMIT, INVALID_WINDOW, WINDOW_LIMIT, UNKNOWN_ROW, UNKNOWN_COLUMN, LOSSY_EXPORT,
SERIALIZE_FAILED (including output-cap failures), and ID_EXHAUSTED. Failed operations
return no partial dataset/output and do not change a live document. No untrusted
input path uses unsafe code, unwrap or expect.

| Guard | Limit |
| --- | --- |
| Input, including BOM | 8 MiB |
| Data rows / columns | 100,000 / 512 |
| Stored fields, excluding implicit Missing | 1,000,000 |
| Rows per window | 256 |
| Logical cells per window | 16,384 |
| Preview text bytes per session window | 1 MiB |
| UTF-8 bytes per cell preview | At most 256, reduced for wide windows |
| Serialized bytes | 32 MiB |

End-of-document windows contain the remaining rows; offset equal to rowCount and
zero limits are valid empty windows. Larger offsets or requests above row/cell
limits fail; request fewer rows after WINDOW_LIMIT. Borrowed Rust windows never
copy text and have no text-size rejection. For session copies, divide the 1 MiB
budget equally across all logical cells in the returned window, capped at 256
bytes per cell, then shorten each text/number prefix at a valid UTF-8 character
boundary. This guarantees the payload budget while keeping every requested row's
identity available, even if a single cell exceeds 1 MiB. `truncated` marks every
shortened payload explicitly; storage, `readCell()` and serialization retain the
full values. `readCell()` can transfer a whole large cell on explicit request.
NSH copies input bytes once into Rust and copies returned snapshots/bytes across
the bridge. The engine retains decoded fields, not the original file or a dataset
clone. Only requested windows are copied; no whole-dataset JS cache or worker architecture is added.
Dispose drops the document and all retained rows/strings. WASM's allocator may keep
freed linear-memory pages for reuse; disposal does not promise the browser shrinks
its heap. Multiple sessions can coexist; per-session caps are not a global quota.

## Verification and remaining work

Rust tests cover parsing, invalid syntax/encoding, headers, empty/missing values,
leading zeros, Unicode, round trips, identity, windows, lifecycle and resource
limits, including large-cell discoverability, UTF-8 preview boundaries and the
aggregate preview byte budget. The NSH smoke tests cover typed results/arguments,
failed constructors, serialization, and explicit disposal, with a test-only app
entry to exercise the actual browser/WASM bridge, including a 2 MiB cell preview followed by its full
stable-ID read. Production UI tests open `App.tsx` and cover paste import, replacement after failure,
responsive inspection, keyboard selection and offscreen row-window recycling.

Verify `cargo fmt --check`, `nsh check`, `nsh build`, `nsh test`,
`nsh test --target web` and `nsh package --target web -o dist` for changes. A web
package verifies the in-memory csv dependency path compiles to wasm32; browser
smoke tests verify runtime compatibility. Resource caps have synthetic tests but
are not large-file responsiveness benchmarks. Measure representative browsers
before raising them or adding workers. Editing, structured-format mapping, safe download policy and undo/redo remain
later work. Browser file and layout regressions run through the synthetic host
used by scripts/test-browser-files.sh.

## Import and inspection UI

`App.tsx` mounts the responsive screen. TSX owns the current document handle, import draft,
dialog visibility, loading/errors and selection. The production entry imports no
test host. No document history is retained. A document view is keyed by document
ID, and its owner disposes the Rust session when closed/replaced. Cleanup is
idempotent so root disposal and view disposal cannot free it twice.

`ImportDetector.inspect(bytes, filename)` scans up to 32 logical records, counting
commas/tabs outside quoted text. The filename breaks ties only. Mixed separators
or no separators are marked ambiguous. It validates UTF-8 and the 8 MiB input cap;
the existing strict parser validates the complete file on import. The review step
always exposes comma/tab and the header toggle. No scalar or header inference is
added. A failed constructor leaves the old session untouched.

`BrowserImport.onFile(callback, onDrag)` owns a hidden local file input and document
drag/drop listeners. Its returned stop removes the input, handlers and closures;
in-flight reads check lifetime/generation before delivery. `pick()` clicks that
input synchronously in the user's press. Bytes are read through browser File APIs
in Rust/WASM, with the size cap checked before allocating a read buffer. Only one
.csv/.tsv file is accepted. These APIs use registered NSH modules and generated
interfaces. Target-specific web-sys/wasm-bindgen dependencies add no native target.

`components/DataGrid.tsx` styles NSH's DataTable. It supplies `rowCount`, `dataStart`
and only the visible row range plus four rows on each side. Wide windows are split
into requests respecting 256 rows, 16,384 logical cells and the engine preview-byte
budget. The table virtualizes both rows and columns; sticky headers, two-axis
scrolling, hit testing, focus and keyboard navigation belong to NSH. TSX translates
only the current cell previews to display text. Selection retains document, row
and column IDs independently of rendered cells. Keyboard jumps can read a single
row before the visible-range event arrives. Preview truncation is indicated with
an ellipsis and original byte count in the inspector; the grid is read-only.

`DocumentSession.profile(columnId)` computes exact distinct/empty/missing counts
and the top five nonempty values in Rust. Empty text is a distinct present value;
missing fields are excluded from distinct counts. Frequency maps borrow the
stored strings and are dropped after computation. The session caches at most
eight small summaries and recomputes an evicted column on demand. Selection within
a column does not rescan it. Common-value previews are bounded to 256 UTF-8 bytes,
and ties use lexical text order. Current imported values remain Text.

The desktop inspector uses a stationary rounded frame around an independent
scroll view. Dialogs have explicit centered desktop positioning and bottom panel
positioning below 640 points, with a scrollable content region and safe-area
padding. The inspector moves to a contextual panel below 900 points.

NSH 0.4's web desktop backend draws a horizontal indicator even when the inner
DataTable scroller disables it, alongside DataTable's bottom viewport scrollbar.
`BrowserStyles.install(css)` installs/removes a scoped app stylesheet suppressing
only the disabled inner indicator beneath `data-grid`. TSX owns that CSS; Rust
only attaches it. This compatibility rule is tied to the resolved 0.4 DOM classes
and should be checked visually when upgrading NSH. It does not modify the NSH runtime or disable
scrolling. File contents never enter CSS, markup or executable code.
