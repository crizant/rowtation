# Data operations

All values, view ordering, matching, transactions and column statistics live in
Rust/WASM. The UI retains bounded row previews and transient drafts. Operations
are synchronous on the browser thread; search input is debounced, not moved to a
worker. No recent history, persistence, other format or download UI is added.

## Sorting

Ascending sorts by canonical type: Number, Boolean, nonempty Text, empty Text,
Null, Missing. Descending reverses that order, including the empty ranks. Numbers
compare as arbitrary-precision decimals; equal numeric values retain imported row
order. Invalid future numeric lexical forms sort after valid numbers and compare lexically.
Booleans sort false before true. Text uses case-sensitive UTF-8 lexical order.
CSV/TSV imports contain Text only: `001`, `10`, `2` remain text and sort in that
order. No inference, locale collation, natural sorting or conversion occurs.
Sorting is stable relative to imported order, including after changing direction.
Reset restores imported order. Backing rows, IDs, schema, revision and dirty state
are unaffected. An edit to the active sort column reapplies the current sort.

## Search and replacement

Search visits all rows in current view order, then schema columns, then matches
left to right within a cell. It does not filter rows. Scope is explicitly all
columns or the column selected when “Selected column only” is enabled; navigation
does not move that scope to another column. Hidden columns participate only in the
explicit “All columns” scope. Headers, missing/null fields and booleans are not
searched. Text and future canonical numbers are searchable.

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
64 MiB of scoped cell payload (every empty/absent scoped cell counts as one byte), and 100,000 matches.
Exceeding a bound returns an error with no partial result count. The regex engine
avoids catastrophic backtracking. Narrow the term/scope when a limit is reached.
The retained match list stays in Rust; navigation transfers one match with stable
row/column IDs, UTF-8 byte range and revision. Sorting/search edits invalidate the
list; the UI searches again and reveals the first result. Multiple occurrences in
a cell are separate navigation results; matching cells are highlighted in the grid.

Replacement always treats the replacement string literally, including `$1`, `$`,
backslashes, newlines and Unicode. There is no capture expansion. “Replace” changes
one occurrence; “Replace all” changes precisely the retained search scope in one
transaction. Ranges are applied in a single forward pass per cell. Counts report
matched occurrences and cells whose final values actually differ. Replacing text
with itself counts occurrences but creates no revision/history/dirty change.
Numbers replaced in a future typed document become literal Text; current CSV/TSV
values already are Text. No formulas, markup or code are evaluated.

## Editing, consistency and history

Selection and mutation use document-scoped stable row/column IDs. Display row
positions are only for scrolling. The grid supports double-click or F2 to
open an editor; the labeled Edit cell action supports touch. Enter or Save cell
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
and serialization round trips as well as values. There is no save/export workflow;
“Unsaved” means the active in-memory document differs from import. Closing or
replacing it releases its session and changes.

Inspector statistics remain lazy, exact and bounded to five common-value previews.
They are invalidated after value changes and recalculated in slices when visible.
CSV/TSV display type remains Text; no numeric/date inference is introduced.
