# Structured imports

JSON, NDJSON (`.ndjson` or `.jsonl`) and XML are supported by picker, drop and
paste. Everything runs locally in Rust/WASM; embedded content is never executed.
Review suggests a format using content first, filename hints second. Every format
can be selected explicitly. CSV/TSV retain their separator/header controls and
all-text semantics; structured formats hide those controls. Bad input leaves the
active document, edits and undo history intact.

## JSON and NDJSON

JSON accepts a single object or an array of objects, including `{}` and `[]`.
Top-level scalars, arrays of scalars and duplicate object keys are rejected with
an actionable error. Nested objects and arrays can contain any JSON value.
NDJSON accepts one object per nonblank physical line, using LF or CRLF. Empty and
whitespace-only lines are skipped, including at the start/end. Errors identify
the physical line and nonblank record. It parses each line independently, without
creating an intermediate JSON array. JSON syntax and nested duplicate-key errors report positions in the original
document, including physical lines in CRLF input; substring decoder positions
are translated before being reported.

Leaves become columns: `user.name`, `user.address.city`, `items[0].sku`.
Simple nonempty alphanumeric/underscore keys use dot notation; other keys use a
JSON-quoted bracket segment, e.g. `["a.b"]` and `user[""]`. This avoids aliasing
literal property names with paths. Columns follow first encounter order, using
source object key order and array index order. Heterogeneous records form a union
of paths; absent values are Missing. A path that is an object in one record and
a scalar in another has separate leaf columns and separate per-row shapes.

Numbers retain their exact validated token, including large integers, decimal
trailing zeros, negative zero and exponent spelling. They never pass through f64.
Strings remain Text without inference: `"00123"`, `"true"`, and `"null"` stay
strings. JSON booleans and null become Boolean and Null. Null, empty Text and
Missing remain distinct. Empty containers have no leaf cells; reconstruction
metadata preserves them, even in zero-column records.

## XML subset

Use a single wrapper containing zero or more record elements with the same
expanded name, e.g. `<rows><row id="1"><name>Mia</name></row></rows>`.
The wrapper may declare namespaces, but cannot have attributes or non-whitespace
text. Each record and nested element can have attributes, child elements, or
text. A leaf's text (including CDATA) becomes `#text`; attributes become `@name`.
For example: `@id`, `name#text`, `address.city#text`, `address@country`.
Empty leaf elements become empty Text, distinct from absent elements.
All XML values are Text; no scalar inference is applied.

Repeated sibling elements use zero-based indexed paths such as `tag[0]#text`
and `tag[1]#text`. Singleton children use an unindexed path. This decision is per
record/parent, so heterogeneous records may have both `tag#text` and
`tag[0]#text` columns. Ordered reconstruction trees retain the exact child order,
element names, attribute order, in-scope namespace bindings and cell identities.
Names use expanded `{namespace-URI}local` identities in paths (quoted bracket
segments where needed), preventing collisions between different namespaces.
A reconstructed document can choose different equivalent namespace prefixes.

DTD processing is explicitly disabled. External entities have no resolver and
cannot load files or network resources. Undefined entities, malformed XML,
comments, processing instructions and mixed non-whitespace text with child
elements are rejected. XML declarations are accepted only for UTF-8 input.
The parser normalizes XML line endings, attribute whitespace and entity escapes;
CDATA boundaries and empty-element spelling are not retained. Whitespace between
child elements is treated as formatting and discarded. This is a semantic tabular
subset, not a promise of arbitrary or byte-for-byte lossless XML conversion.

## Model, operations and reconstruction

The existing canonical document remains authoritative. Row and column identities
are assigned once, and the source format is reported by session metadata. A Rust
`Structure` stores the original envelope and one `Shape` per row. Object entries
retain keys/order, arrays retain length/order (including empty arrays), and leaves
reference stable column IDs; values are stored only in canonical cells. XML also
retains its wrapper. Column paths and original per-row shapes together describe
where newly edited Missing cells belong. Reconstruction must explicitly
resolve incompatible insertions, e.g. a new child below an existing scalar; it
must never silently discard a cell. Multi-format reconstruction and explicit conversion policies are implemented in
the export milestone; see [export policies](export.md).

Sorting uses existing canonical type order and exact decimal comparison. Search
and replacement cover Text, numeric tokens and boolean spellings (`true`/`false`);
Missing and Null are excluded. Profiles distinguish types and count Null, empty
Text and Missing separately. Distinctness includes canonical type, so Number 1
and Text "1" count separately even if their common-value labels look alike.
Unchanged numeric/boolean edits and identity replacements preserve their types
and do not create history. Changed values become literal Text; no typed value is
inferred from the new text. The editor explains this rule. Opening and saving an
unchanged Missing/Null draft is also a UI no-op; explicit Clear cell creates empty
Text. Undo restores the exact original canonical type. Shapes are immutable
source metadata; editing leaves does not rewrite them. Dirty state, replacement
transactions, history bounds, stable selection and grid windows are shared with
CSV/TSV.

## Resource limits

Structured nesting is limited to 64 levels. JSON parsing uses borrowed raw tokens
before decoding individual leaves, avoiding an extra owned copy of the full JSON
text; NDJSON processes one line at a time. XML uses a WASM-compatible in-memory
`roxmltree` parser with DTD disabled, then checks depth before mapping. Its parsed
XML tree is transient and released after import. Deep XML is parsed by the XML
library before the mapping-depth validation; extremely large input can still
exhaust browser memory.

Structured review detects format from at most 64 KiB and reads byte-size metadata;
it does not validate or parse the document. The dialog explicitly says row/column
counts are checked on Import. Files over 8 MiB still require confirmation before
reading; large pasted input shows the byte-size warning before parsing. CSV/TSV
retain their existing whole-input delimiter measurements and dense row storage.

Only after the user presses Import (or Import anyway) does Rust parse the complete
structured document. Exact row/column/field/value warnings are then measured from
that candidate. If a shape threshold is exceeded without an earlier confirmation,
the dialog retains that Rust session until Import anyway, cancellation, or format
correction. Confirming publishes the same session without reparsing; cancellation
or changing settings disposes it. The active document and undo history stay intact
until successful publication. TSX holds session handles, not a second dataset.

Structured rows store sorted (column index, value) pairs for present leaves only.
Missing properties do not allocate cells, and editing an absent property inserts
only that entry. Undo removes it again. Storage grows with present cells and
schema/shape metadata rather than rows multiplied by schema width. Sparse reads
use binary search; CSV/TSV keep contiguous O(1) lookup. Numeric sorting computes
exact BigDecimal keys once per numeric cell per sort, then drops those temporary
keys. Text-only sorts allocate no numeric-key array. Edits and undo rebuild keys
when the active sort is reapplied, retaining original lexical numbers and stable
equal-value ordering.

The same 8 MiB / 100,000 records / 512 columns / 1,000,000 fields / 1 MiB field
warnings apply. No record is silently truncated. Full parsing and sorting still
run synchronously after Import confirmation; large inputs can block the tab, and
memory exhaustion cannot be recovered as a normal parsing error. Windows remain
bounded to 256 rows and existing column/byte previews. Native scaling benchmarks
and their commands/results are recorded in [structured performance](structured-performance.md).
