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

Focused validation after the fixes: 53 affected Rust tests, seven headless
structured UI tests and the same seven Chromium tests passed. Formatting,
`nsh check`, `nsh build`, desktop/mobile visual inspection and web packaging
passed. The whole application test suite was not rerun for this review follow-up.
