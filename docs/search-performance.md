# Incremental search validation (issue #9)

Search uses the registered Rust `startSearch` / `searchStep` / `cancelSearch`
methods. The browser yields with `setTimeout` between calls; Rust owns all
matching state. A slice targets 8 ms and checks its deadline every 64 work
units, including cells, matches and highlight-index entries. Sparse rows visit
stored entries, scoped queries use a single column lookup per row, and dense
rows retain contiguous iteration. Ordinary cells use `regex::find_iter`;
suspended cells resume at their saved byte offset with full haystack context.
Completed match/index buffers are reused on subsequent searches.

The UI clears stale matches immediately when a query changes, before the
180 ms debounce expires. Options, sort, edits, undo/redo, closing Find, document
replacement/disposal and explicit cancellation invalidate unfinished work.
No partial counts, highlights or replacements are exposed. The existing 64 MiB
payload and 100,000-match limits remain unchanged; there is no new dataset cap.

Individual regex engine calls and query compilation cannot yield. The deadline
is cooperative, not a guaranteed maximum pause for a very long cell or expensive
pattern. Splitting the haystack into independent chunks would change anchors,
word boundaries and greedy matches, so this implementation does not do so.

## Native before/after measurements

Measured locally on 8 Nov 2026, release builds, five repetitions per case,
with the first repetition excluded below. The baseline is commit `a68b30b`,
compiled in a separate worktree/target directory. These are observational
measurements, not timing assertions. The synchronous controls use the same
fixtures and terms before and after; the new large-input benchmark additionally
measures the incremental API. Native sliced timings exclude browser scheduling.

| Synthetic fixture / search | Before, median ms | After, median ms |
| --- | ---: | ---: |
| CSV, 100k rows, absent term, all columns | 4.20 | 4.14 |
| CSV, 100k rows, 100k matches, all columns | 8.04 | 7.52 |
| CSV, 100k rows, 100k matches, one column | 6.18 | 5.91 |
| TSV, 100k rows, absent term, all columns | 4.11 | 4.19 |
| TSV, 100k rows, 100k matches, all columns | 7.81 | 7.99 |
| TSV, 100k rows, 100k matches, one column | 6.01 | 6.17 |
| Sparse NDJSON, 10k rows × 10k schema columns, absent | 0.26 | 0.25 |
| Sparse NDJSON, 10k matches | 0.51 | 0.50 |
| CSV, 500k rows, 40,000,000 searchable bytes, absent | 9.02 | 8.60 |

The 40 MB incremental scan took a median 9.01 ms over two slices, with a
longest slice of 8.00–8.001 ms. Small control differences are within ordinary
run-to-run variation; there is no material throughput regression in these cases.
The browser can take longer overall because timers yield to input/rendering and
are subject to timer clamping. The purpose is shorter uninterrupted work.

Reproduce:

```sh
cargo test --release --test search_performance -- --ignored --nocapture --test-threads=1
nsh test SearchResponsiveness.test.tsx
nsh test SearchResponsiveness.test.tsx --target web --browser /path/to/chromium
```

The responsiveness fixture uses 500,001 rows and 40 MB of searchable text.
A separate event-loop task must run while search remains unfinished, and a
second run cancels after a real slice and changes the query. It asserts the
single result's stable row identity. It reports slice count, longest call,
active scan time and total elapsed time. It deliberately avoids fragile
wall-clock pass/fail thresholds. Rust tests force a yield every 64 work units
with a zero-time budget, checking cancellation, invalidation, exact UTF-8 and
zero-width regex ranges against `regex::find_iter`, byte accounting across
resumes, and atomic rejection at the match limit.

## Browser verification

Chromium (Playwright's cached Chrome for Testing, macOS arm64) passed the large
search/restart test and all 25 production app workflows, including pending
cancellation, stale-query suppression, closing Find, replacement/undo/redo,
match selection, long-cell previews and mobile find. A browser probe of the
40 MB fixture reported 6 slices, 10.0 ms longest call, 43.0 ms active scanning
and 48.1 ms elapsed. Another run reported 5 slices, 9.2 ms longest call and
41.4 ms elapsed. Those are single observations, not a hard latency guarantee.

Final local checks passed: `cargo fmt --check`, `nsh check`, `nsh build`,
`nsh test` (120 passed, 8 benchmark/stress tests skipped), all three ignored
search benchmarks in release, and `nsh package --target web -o dist` (13.5 MB).
The focused Rust suite was re-run after avoiding an unused regex iterator/cache
on suspended cells. Mobile pending/cancelled states and desktop dark matching
were visually inspected; mobile status keeps enough width for the full message.
Generated screenshots, build output and synthetic inputs are not committed.
