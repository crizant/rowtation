# Data operations

All values, view ordering, matching, transactions and column statistics live in
Rust/WASM. The UI retains bounded row previews and transient drafts. Operations
are synchronous on the browser thread; search input is debounced, not moved to a
worker. No recent history or persistence is added. See [export](export.md) for downloads and conversion.

## Sorting

Ascending sorts by canonical type: Number, Boolean, nonempty Text, empty Text,
Null, Missing. Descending reverses that order, including the empty ranks. Numbers
compare as arbitrary-precision decimals, with keys parsed once per numeric cell per sort; equal numeric values retain imported row
order. Invalid future numeric lexical forms sort after valid numbers and compare lexically.
Booleans sort false before true. Text uses case-sensitive UTF-8 lexical order.
CSV/TSV imports contain Text only: `001`, `10`, `2` remain text and sort in that
order. No inference, locale collation, natural sorting or conversion occurs.
Sorting is stable relative to imported order, including after changing direction.
Reset restores imported order. Backing rows, IDs, schema, revision and dirty state
are unaffected. An edit to the active sort column reapplies the current sort.

## Search and replacement

Search visits all rows in current view order, then schema columns, then matches
left to right within a cell. It does not filter rows. Scope is all columns, or
the column selected when the find bar’s column control is pressed; later
navigation does not move that scope. Hidden columns participate only in the
all-columns scope. Headers and missing/null fields are not searched. Text, exact numeric tokens and boolean spellings are searchable.

Literal search escapes all regex metacharacters. Case-insensitive matching uses
the regex crate's Unicode simple case folding, without normalization, locale
rules or multi-character folds. Case-sensitive matching compares exact text.
Matches are nonoverlapping. Regex mode uses Rust `regex` syntax: Unicode classes,
anchors, groups and alternation; no lookaround or backreferences. Dot excludes
newline unless `(?s)` is used; multiline anchors require `(?m)`. Zero-width regex
matches are allowed, once per UTF-8 boundary as defined by `regex::find_iter`.
Empty search terms return zero matches, including in regex mode. Invalid regex
produces an inline error and clears old results.

Search is bounded to 4096 pattern bytes, a 1 MiB compiled regex size and DFA cache,
64 MiB of actual searchable UTF-8 payload (Text, numeric tokens and boolean spellings), and 100,000 matches. Missing and Null contribute no bytes; empty Text contributes zero bytes and still participates in zero-width regex matches. Sparse all-column search visits only stored entries in schema order; a scoped search reads only the selected column in each row. Dense CSV/TSV rows retain contiguous iteration.
Exceeding a bound returns an error with no partial result count. The regex engine
avoids catastrophic backtracking. Narrow the term/scope when a limit is reached.
Raising those bounds, returning a partial list, or moving the scan off the calling
thread is engine follow-up [issue #9](https://github.com/crizant/rowtation/issues/9),
not an import cap.
The retained match list stays in Rust. Finding the same occurrence again is one
lookup, and navigation transfers that match plus a bounded snippet, not the whole cell. After a new query the UI reveals
the first match. After replace, replace all, undo, redo, or a scope/case/regex
change it keeps the same occurrence when that byte range still matches, and
otherwise keeps the same index clamped to the new list. The counter and the grid
selection both follow that choice. An empty query has no count; a search in
flight says “Searching…”; no hits say “No matches”; an error shows the message
and no count. Multiple occurrences in a cell are separate results. Matching
cells are tinted. The active occurrence is marked in the preview, or named in
the find bar when its bytes sit past the 512-byte preview. Cmd/Ctrl+F focuses
the query and selects it. Enter and Shift+Enter move to the next and previous
match. Escape closes Find and returns focus to the grid.

Replacement always treats the replacement string literally, including `$1`, `$`,
backslashes, newlines and Unicode. There is no capture expansion. “Replace” changes
one occurrence; “Replace all” changes precisely the retained search scope in one
transaction. Ranges are applied in a single forward pass per cell. Counts report
matched occurrences and cells whose final values actually differ. Replacing text
with itself counts occurrences but creates no revision/history/dirty change.
Changed numbers and booleans become literal Text; replacements producing the same spelling preserve their type. CSV/TSV values already are Text. No formulas, markup or code are evaluated.

## Editing, consistency and history

Selection and mutation use document-scoped stable row/column IDs. Display row
positions are only for scrolling. Double-press, F2, or Edit cell in the cell
menu opens an editor. Clear cell is in that menu. Enter or Save cell
commits; Escape/Cancel drops the TSX draft. Tab commits and selects the next cell.
Clear cell commits empty Text, which differs from Missing or Null. Drafts never
modify the Rust document before commit. Re-entering identical text is a no-op.

Mutations check the expected revision and validate/bound all changes before
application. One successful mutation/undo/redo increments revision once and
invalidates profile caches and unfinished profile jobs. UI refresh happens once
per operation. Failed mutations leave document values/revision/history intact.
Selection is preserved by stable IDs and relocated after ordering changes.

Undo/redo stores per-cell before/after values, not dataset snapshots. Replace all
is one transaction. History retains at most 50 transactions and 32 MiB of estimated
payload including imported baseline values and 64 bytes per changed cell; older undo entries are evicted. Each
transaction is limited to 16 MiB by the same accounting and is rejected atomically
above that size. Undo moves a transaction to redo; a new value change clears redo.
Sorting, searching, navigation and same-value edits do not enter history.

A sparse map retains the imported value only for cells currently differing from
import. Its size is the modified-cell count. Restoring a value, directly or with
undo/redo, removes that difference; undo history eviction does not reset the dirty
baseline. Source bytes/quote style are not duplicated. Undo restores Missing tails
and serialization round trips as well as values. Export does not reset the imported baseline;
“Unsaved” means the active in-memory document differs from import. Closing or
replacing a modified document still asks first after export. Confirming discards the changes and releases the session.
An unmodified document closes immediately. See [structured import type and reconstruction rules](structured-imports.md).

Search and sort run synchronously on the calling thread, including in the browser,
so a large query blocks the tab until it returns. The limits above are search
limits only. Import has no size cap.

`ROWTATION_BENCH_ROWS=2000000 cargo test --release --test stress -- --ignored benchmark_engine --nocapture`
measures a generated CSV. One release run on 8 Nov 2026 loaded 2,000,000 rows
(113 MiB, 6 columns). Sorting the country column took 549 ms ascending and
533 ms descending; restoring imported order took 0.23 ms. A scoped search for
one order id took 29 ms and found that row. Searching the status column for
“paid” returned the 100,000-match error in 5 ms. Searching every column for an
absent token returned the 64 MiB error in 65 ms. Neither error changes the
document. These are single-run timings, not a performance budget.

Inspector statistics remain lazy, exact and bounded to five common-value previews.
They are invalidated after value changes and recalculated in slices when visible.
CSV/TSV display type remains Text; no numeric/date inference is introduced.
