# Export and conversion policies

Five outputs: CSV, TSV, JSON, NDJSON and XML. Processing stays local in Rust/WASM.
The English export dialog supports filename, original/current sorted order, copy
and browser download. Every scope includes all rows; search only highlights.
No recent history or persistent document storage is added.

## Conversion matrix

| Source | CSV / TSV | JSON / NDJSON | XML |
| --- | --- | --- | --- |
| CSV / TSV | Literal text and field order; optional formula protection | Objects with literal header keys and string values; missing properties omitted | `rows/row` with named elements; invalid names rejected |
| JSON / NDJSON | Explicit flat path headers and scalar-to-text policy | Original objects/arrays and exact scalar types | Named elements; arrays contain `item` elements; explicit type/array warning |
| Tabular XML | Explicit flat path headers and text policy | Objects with `@name`, `#text`, child-name arrays and expanded namespace names | Supported semantic source tree with attributes, child order and repeated elements |

JSON preserves a single-object source envelope; other JSON outputs are arrays.
NDJSON always writes exactly one compact object per physical line plus a final
newline; zero records produce empty text. Output object entries retain source or
schema order, including pretty JSON. No numbers-as-numbers inference is offered.
Numbers retain their original lexical tokens: large integers, decimal zeros,
negative zero and exponents. Changed numbers/booleans become text under the existing
editor policy; undo restores their original types. Numeric-looking text and
leading-zero identifiers stay strings. Unicode and escaping follow each format.

## Explicit losses and rejected conversions

Warnings require the user to accept mapping changes before Copy or Download.
The registered `exportWarning(options)` API returns these same policies;
`serialize` rejects unaccepted warnings with `LOSSY_EXPORT`.

Delimited output uses literal paths, not reconstructed containers. Numbers and
booleans become their exact textual spellings. Null and interior missing cells
become empty text; trailing missing cells remain absent. Empty containers and
source envelopes have no delimited representation. Header omission also removes
column names from the file, as selected by the user. CSV separator is configurable;
TSV uses tab. Quoting is normalized, with LF records; source quoting/BOM style is
not retained. Structured output with headers disabled cannot retain paths.

JSON reconstruction walks immutable per-row metadata, referencing current cell
IDs. Empty containers and absent source properties are retained. Missing fields
remain absent unless edited. Edited absent JSON paths use retained shapes from
other records to insert only the edited leaves. New object entries append in schema
order; arrays can append contiguous elements. Scalar/container collisions and
array holes block reconstruction rather than inventing values or discarding edits.
Explicit flat JSON/NDJSON or delimited output remains available. Flat JSON/NDJSON requires acknowledgement and writes
literal column-path keys, omitting empty containers and the source envelope.
Duplicate headers block JSON/NDJSON and generic XML because object keys would collide.

XML reconstruction preserves the supported import subset semantically, not byte
for byte or arbitrary XML. In-scope namespace bindings are retained (equivalent prefixes may be chosen); CDATA, entity spelling, indentation and empty-element spelling are not
preserved. XML text/attribute escaping preserves CR and attribute whitespace via
character references. Readable formatting indents only element-only parents.
XML source leaves remain text. Unplaced edits block reconstruction.

Cross-format XML uses `rows/row`, object keys as element names, arrays as a named
parent containing repeated `item` children, and scalar text without inference.
Types and array identity are not recovered on XML reimport. Null and empty
containers block export; names that are not XML 1.0 Names, and characters
forbidden by XML 1.0, also block. JSON conversion from XML uses `@attribute`, `#text` and arrays of child
objects; expanded names preserve namespace identities but declarations and the
wrapper, record names and interleaved child order are omitted. These policies are shown before conversion.

## Formula-safe text

Enabled by default in the dialog for CSV/TSV. An apostrophe is prefixed to fields
and headers beginning with `=`, `+`, `-`, `@`, tab, CR or LF, including formula
characters after leading whitespace. This deliberately changes literal values
and requires acknowledgement. Turning it off emits literal text, which spreadsheet
applications may interpret. Rowtation never evaluates formulas, scripts or markup.
It does not claim to control every spreadsheet application's import behavior.

## Lifecycle, transport and resources

Export never mutates cells, revision, dirty status, history, search or sort. Copy,
cancellation and errors preserve the active document. A download request is not
proof that a file was saved; feedback asks the user to check browser downloads.
Closing/replacing a changed document still warns, including after export. There
is no persistent exported-revision state.

TSX initiates serialization and browser transport within the press handler before
any await, retaining user activation. Copy transfers only final UTF-8 text and uses
NSH Clipboard; downloads serialize in Rust and use a temporary browser Blob/link
through the registered session API. CSV uses `text/csv`, TSV
`text/tab-separated-values`, JSON `application/json`, NDJSON
`application/x-ndjson`, XML `application/xml`. Supported input extensions are
replaced with the chosen output extension; unsafe filename characters are replaced.
The temporary anchor is removed and object URLs are revoked after the click.

Serialization is synchronous. Rust builds complete output in memory, including
intermediate record strings for structured output; copy adds a JS string and the
browser Blob also needs memory. There is no JS dataset clone, streaming export,
server or upload. Recoverable engine/clipboard/download errors appear in the dialog;
WASM allocation failure or a browser tab being killed cannot be caught. Large
exports may pause or exhaust the tab. There are no claimed large-export benchmarks.
