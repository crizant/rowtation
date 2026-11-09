# Structured import review fixes: measurements

Measured on 8 November 2026 on an Apple M4 with Rust 1.99.1, native release
builds. The baseline is PR #11's original commit `74e3089`; both revisions ran
the same synthetic `tests/structured_performance.rs` benchmark. These timings
measure Rust on this machine, not browser responsiveness or a supported file-size
budget. Inputs contain no user data.

Numeric/delimited timings are medians of three separate process runs; sparse
scaling timings are single process runs. Peak RSS is the whole benchmark process
reported by macOS `/usr/bin/time -l`, including fixtures, source buffers, parser
metadata, allocator retention and sort scratch space. It is not WASM heap usage.
No elapsed-time threshold is enforced by a test.

| 100,000-row operation | Original PR | Review fixes |
| --- | ---: | ---: |
| Exact numeric import | 31.66 ms | 30.36 ms |
| Exact numeric ascending sort | 628.75 ms | 42.41 ms |
| Exact numeric descending sort | 631.45 ms | 40.47 ms |
| CSV import, two text columns | 8.15 ms | 7.38 ms |
| CSV text sort | 21.28 ms | 19.68 ms |
| TSV import, two text columns | 7.39 ms | 6.63 ms |
| TSV text sort | 21.22 ms | 19.61 ms |

Numeric values have a large integer coefficient and 21 decimal places. A fixed
permutation prevents an already-sorted input from hiding comparison costs.
Numeric parsing occurs once per numeric cell per sort, verified separately by a
unit-test counter. Stable equal-value order, negative zero, exact lexical tokens,
and reordering after edit/undo/redo have correctness tests.

The numeric benchmark's median peak RSS rises from 59.61 to 69.08 MiB because
exact sort keys are temporarily retained. They are released after each sort.
The dense CSV/TSV control's RSS rises from 24.36 to 28.33 MiB; the tagged row-storage
representation adds eight bytes per row on this 64-bit target, and process RSS
also includes allocator effects. Dense field vectors and direct indexing are
retained, with no numeric-key array allocated for text-only sorts. The measured
control timings show no runtime regression; they do not promise improvements on
other machines or workloads.

## Heterogeneous NDJSON

Each record introduces one unique property, so N records produce N columns and
exactly N present cells. The old dense row representation allocated N(N+1)/2
cell slots, mostly Missing. The sparse representation allocates entries only for
present cells and retains the same deterministic schema and reconstruction shape.

| Rows / unique columns | Original stored slots | New stored entries | Original peak RSS | New peak RSS |
| --- | ---: | ---: | ---: | ---: |
| 2,000 | 2,001,000 | 2,000 | 116.06 MiB | 3.91 MiB |
| 4,000 | 8,002,000 | 4,000 | 277.34 MiB | 5.28 MiB |

The baseline was deliberately not run with 100,000 unique columns: its storage
alone would require billions of cell slots. No import cap was added.

| New rows / unique columns | New stored entries | Parse | Peak RSS |
| --- | ---: | ---: | ---: |
| 25,000 | 25,000 | 9.19 ms | 21.41 MiB |
| 50,000 | 50,000 | 18.15 ms | 42.41 MiB |
| 100,000 | 100,000 | 39.22 ms | 77.45 MiB |

The 100,000-row fixture is 2,177,780 bytes. Review detection took 0.008 ms;
structured byte-size review rounded to 0.000 ms. Review never constructs a
canonical document. The 64-KiB sample is a detection-work bound, not an input cap.
Full validation and one complete parse start after Import. If an exact shape
warning is needed, the already-parsed candidate is retained for confirmation;
the browser integration test verifies that there is exactly one Parse measurement
across Import and Import anyway. Invalid data beyond the sample and a malformed
9-MiB source are regression fixtures for deferring validation until Import.

## Reproduce

```sh
cargo test --lib --test structured_import --test document_engine --test operations --test inspection
nsh test StructuredImport.test.tsx
nsh test StructuredImport.test.tsx --target web --browser /path/to/chromium

ROWTATION_BENCH_ROWS=100000 cargo test --release --test structured_performance -- --ignored --nocapture
```

For isolated RSS measurements, build with
`cargo test --release --test structured_performance --no-run`, then use the
executable path printed by Cargo:

```sh
ROWTATION_BENCH_ROWS=100000 /usr/bin/time -l target/release/deps/structured_performance-HASH --ignored --nocapture heterogeneous_ndjson
ROWTATION_BENCH_ROWS=100000 /usr/bin/time -l target/release/deps/structured_performance-HASH --ignored --nocapture numeric_sort
ROWTATION_BENCH_ROWS=100000 /usr/bin/time -l target/release/deps/structured_performance-HASH --ignored --nocapture delimited_control
```

Repeat heterogeneous runs with 25,000 and 50,000 rows. To compare the original
commit in a detached worktree, copy the same benchmark there, build it separately,
and set `ROWTATION_EXPECT_DENSE=1` when running that baseline. That flag disables
only the new sparse-entry-count assertion; it changes no parsing/sorting code.

## Sparse search follow-up

Search now iterates sparse stored entries in schema order instead of looking up
all schema columns in every row. A column-scoped search does one lookup per row;
dense CSV/TSV rows retain contiguous iteration. The 64-MiB budget counts only
searchable UTF-8 bytes. Missing/Null add nothing, and empty Text remains searchable
for zero-width regex matches while adding zero bytes. Match order, identities,
highlighting and replacement transactions are unchanged.

The focused benchmark is `tests/search_performance.rs`. On the same Apple M4 and
Rust/native release setup described above, compare the previous PR revision
`9b38f87` with this search fix. Each table entry is the median of 15 measurements
(five searches in each of three sequential process runs). No timing assertions
or new dependencies were added.

| Search | Before | After |
| --- | ---: | ---: |
| 10k sparse rows / 10k columns, absent term | 169.368 ms, false SEARCH_LIMIT | 0.270 ms, 0 matches |
| Same sparse data, matching term | 170.162 ms, false SEARCH_LIMIT | 0.523 ms, 10,000 matches |
| Same sparse data, one column | 0.206 ms, 1 match | 0.083 ms, 1 match |
| 100k-row CSV, all columns, absent term | 4.411 ms | 4.291 ms |
| 100k-row CSV, all columns, 100k matches | 8.108 ms | 8.171 ms |
| 100k-row CSV, scoped, 100k matches | 6.253 ms | 6.354 ms |
| 100k-row TSV, all columns, absent term | 4.373 ms | 4.218 ms |
| 100k-row TSV, all columns, 100k matches | 8.025 ms | 7.932 ms |
| 100k-row TSV, scoped, 100k matches | 6.181 ms | 6.050 ms |

The sparse fixture has only 60,000 searchable bytes, one six-byte value per row.
The baseline all-column searches incorrectly charged implicit Missing cells and
failed before completing; their timings do not represent successful searches of
the whole dataset. Dense control results show no material performance regression;
the small timing differences are within observed run variation. These native
measurements are not browser responsiveness guarantees.

```sh
cargo test --test sparse_search --test operations
cargo test --lib search_budget_counts_searchable_payload_only_at_the_boundary
cargo test --release --test search_performance -- --ignored --nocapture --test-threads=1
```

For comparison, copy the same benchmark into a detached `9b38f87` worktree and
set `ROWTATION_SEARCH_BASELINE=1` for that version. The flag records the known
SEARCH_LIMIT failure instead of failing the benchmark; production code is
unchanged. Fresh-process repeats can use Cargo's printed benchmark executable.
