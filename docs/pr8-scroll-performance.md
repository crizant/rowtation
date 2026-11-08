# PR #8 grid scrolling regression

This investigation compares `main` (`67a0344`) with PR #8 (`4d2dfff`), the same
PR with only `renderCell` removed temporarily, and the fixed PR. No NSH dependency,
Rust operation, cache size, row height, or scroll protocol changed.

## Root cause and isolation

PR #8 installed a custom TSX renderer for every visible cell even with Find
closed. Every cell created a View, outer Text and nested Text span, and multiple
text/style/test-ID bindings independently read `cache()` through `snap()` and
`parts()`. Thus every cache refill invalidated bindings across the visible grid,
including unchanged overlapping cells. `parts()` also repeated the UTF-8 split
and object creation for each binding. The native DataTable path updates retained
text directly without a renderer callback into JS per visible cell.

Temporarily deleting just the `renderCell` prop restored scrolling to the range
of `main`. In separate instrumented grid-only phases, all three variants made
five Rust row-window calls: 9.6 ms total on main, 8.6 ms on PR #8, and 10.2 ms
with the renderer disabled. Native component-prop application took 3.6, 2.8 and
3.2 ms respectively. These are inclusive bridge timings, not isolated conversion
or engine-layout timers. They do not explain the renderer-on regression. The A/B
isolates the aggregate custom-renderer path; it does not assign separate
percentages to callback, nested-text, layout and reactive-invalidation costs.

At the initial viewport PR #8 mounted 1,030 nodes, versus 599 with its renderer
disabled and in the fixed build (main had 564 because its surrounding UI predates operations). Separate
whole-protocol Chrome traces recorded 51.5 ms of minor/major GC on PR #8 versus
6.0 ms with the renderer disabled. These profiling runs include Inspector work
and instrumentation overhead; they are supporting evidence, not the timing table.

## Fix

- Use native DataTable text on pointer desktops when the bounded cached window
  has no matching cells. A memoized boolean and stable callback avoid changing
  the renderer prop on every refill.
- In custom cells, memoize the cell snapshot. Overlapping snapshots retain their
  identity, preventing downstream bindings from updating for unrelated refills.
  Memoize the active occurrence and check coordinates before reading its preview.
- Plain custom cells have one Text. Only the active occurrence creates nested
  Text, in a separate complete Text branch so switching does not replace the
  children of an existing Text with an incompatible string/fragment shape.
- Keep the existing touch long-press wrapper. Table-level selection, double-press,
  context-menu handling and plain clipboard data remain unchanged.

## Measurement protocol

Use the deterministic fixture and production probe described in
[the earlier scroll investigation](scroll-performance.md). The same real file is
imported for every run: 2,000,000 rows × six columns, 127,566,724 bytes, SHA-256
`96c01cb4c135f546a86ae805e456d8f0d837f7ce9f90ab02e6caf5de6e469f97`.

All variants use NSH 0.4.0 (`e626a8d`), release WASM, macOS ARM64 and headless
Brave/Chrome 156, 1280 × 800, DPR 2, desktop idiom. The three descending CDP wheel
sequences are identical. Import/parse, startup, and the later unique-ID Inspector
phase are excluded from the grid-only timing table. Samples measure wall time
between animation callbacks, including the same lightweight movement observation.

Each variant has three fresh-browser timing runs without method wrappers, CPU
sampling or trace recording, alternating main → PR → disabled → fixed per round.
The additional `--search` workload selects the ID column, enables regex, and
searches `00$`: exactly 20,000 matches, one every 100 rows. Its 14 MB scope is
below the existing search limit. Query execution and initial column profiling
finish before measurement. Search reduces the grid viewport; compare search
runs only with each other. Do not use these host-specific numbers as CI FPS gates.

| Grid only, median of three runs | main | PR #8 | Renderer disabled | Fixed |
| --- | ---: | ---: | ---: | ---: |
| Frame interval p50, ms | 16.7 | 16.6 | 16.7 | 16.6 |
| Frame interval p95, ms | 20.0 | 26.6 | 19.1 | 19.7 |
| Largest frame, ms | 31.9 | 57.8 | 36.0 | 36.8 |
| Frames over 25 ms | 2 | 19 | 2 | 3 |

The fix reduces p95 by about 26% and frames over 25 ms from 19 to 3 on this
protocol. It restores performance to the range of main and the isolation build;
the small differences among those three do not establish a further improvement.
Occasional missed frames remain. Raw recordings are `{variant}-timing-{1,2,3}.json`.

| Highlighted ID search, median of three runs | PR #8 | Fixed |
| --- | ---: | ---: |
| Frame interval p95, ms | 25.3 | 24.3 |
| Largest frame, ms | 54.1 | 52.2 |
| Frames over 25 ms | 17 | 16 |

The highlighted path still invokes JS renderers for visible cells. These small
changes do not establish a substantial scrolling improvement while highlights
are active. The confirmed win is avoiding that path when highlighting is not
needed. Raw search recordings are `{pr8,fixed}-search-timing-{1,2,3}.json`.

## Reproduction

```sh
python3 scripts/generate-scroll-fixture.py /tmp/rowtation-pr8-scroll/input.csv
nsh package --target web -o dist
node scripts/profile-scroll.mjs \
  --build dist/Rowtation --csv /tmp/rowtation-pr8-scroll/input.csv \
  --out /tmp/rowtation-pr8-scroll/fixed.json --label fixed
# Repeat with --search for the scoped-highlight workload.
# Add --instrument --trace --cpu for a separate diagnostic recording.
```

Package detached worktrees at the SHAs above for the baselines. For the isolation
variant remove only the DataTable `renderCell` prop from the PR, package it, and
restore the source. Do not use that variant as a feature-complete build. Local
recordings, traces, screenshots and input data are kept outside tracked files
under `/tmp/rowtation-pr8-scroll/`.

## Focused validation and limits

The new `components/DataGrid.test.tsx` covers native rendering before searching,
a query with no matches, switching highlights on/off, first/second occurrence
underlines, plain clipboard copy, keyboard selection, double-click editing,
distant cache jumps with Unicode highlights, and touch long-press editing with
and without highlights. The existing nine grid-cache tests remain unchanged.

`cargo fmt --check`, `nsh check`, `nsh build`, the nine cache tests, and three new
headless grid tests pass. `nsh test components/DataGrid.test.tsx --target web
--filter 'grid pointer'` passes both browser grid tests. Desktop and mobile
screenshots were visually inspected, including the second occurrence, selection,
closed Find/native rendering, and the mobile editor. `nsh package --target web
-o dist` passes. No stress tests or unrelated suites were run; Rust data logic
is unchanged.

Pre-existing browser limitation: `app.longPress` cannot open the cell menu
through NSH 0.4's physical hit layer. The same focused test fails on unchanged
PR #8. A separate production check with mobile viewport, touch emulation and
CDP touch-start → 1,000 ms hold → touch-end also opened no menu on either the
unchanged PR or the fixed package (`touch-pr8.png` / `touch-fixed.png`). The
body cells have pointer events disabled and the hit layer owns pointer input;
headless long-press dispatch reaches the custom cell directly and passes. The
existing mobile wrapper is retained, rather than claiming a fix to NSH's hit
layer or masking that failure as a passing browser test.

The pre-existing NSH scaled-scroll tiny-delta holds remain; see the earlier
investigation. This fixes the app's renderer overhead, not that motion mapping.
