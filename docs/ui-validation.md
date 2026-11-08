# Import and inspection validation

## Generate and inspect screenshots locally

Run from the repository root with NSH 0.4.0 and Chromium installed:

```sh
scripts/capture-layouts.sh
# When Chromium is outside NSH's default search paths:
ROWTATION_BROWSER=/path/to/chromium scripts/capture-layouts.sh
```

The script uses `tests/capture-layouts.steps.js` to capture the production entry
with synthetic orders from `tests/data/orders.csv`. Generated PNGs are written
to `docs/screenshots/`, which is ignored by Git. Captures are local verification
artifacts; do not commit them or use repository image links in PR descriptions.

Open the generated PNGs in an image viewer. Inspect desktop and mobile import,
the data grid, the contextual inspector, the scrolled desktop sidebar, and dark
appearance. Check centered desktop dialogs, mobile safe-area spacing, readable
selection, one horizontal grid scrollbar, and content clipping inside the
stationary inspector frame. Re-run after visible changes; automated workflow
tests alone do not establish visual correctness. Delete the local output when
it is no longer needed.

## Focused workflow checks

`nsh test` runs the Rust fixtures and production UI workflows. These cover UTF-8
CSV/TSV parsing, explicit headers, failed import preservation, successful
replacement, disposal, exact profile counts, previews with whole-value view and copy, diagnostics, cached summaries,
virtualized scrolling, keyboard selection and responsive layouts. The wide-table
case uses 512 columns and a 2160-point viewport, then narrows the viewport and
navigates to an offscreen column. Two workflows cover the large-import warning:
100,001 records require "Import anyway", and small input shows no warning. One mobile workflow covers TSV/no-header import and the contextual inspector. There
are eight UI/bridge workflows in total; parser and statistics cases stay in Rust.
Appearance is verified through screenshots rather than a separate DOM geometry
test harness.

`nsh test --target web --browser <Chromium>` runs those UI workflows through actual
browser input and Rust/WASM, independently from the native headless checks.

`scripts/test-browser-files.sh` adds browser File/FileList/DragEvent coverage:
file picker selection, a valid TSV drop, malformed CSV, unsupported extension,
multiple-file rejection, invalid UTF-8 after a valid draft, cancellation and
newer workflows during delayed reads, preservation/replacement, and session
closing/reopening. It uses synthetic File objects to supply a picker selection
instead of driving the operating system's chooser. The fixture host supplies file
selections/drops only; it contains no layout observer or product test controls.
The script restores nsh.json after its temporary test-host configuration,
including on failure. Run NSH commands serially; 0.4's generated runtime paths
are shared with the development server and package builder.

## Verification result

Verified on 2026-11-08 with NSH 0.4.0 and Chrome for Testing:

- `cargo fmt --check`, `nsh check`, and `nsh build`: passed.
- `nsh test`: 63 passed, with 2 ignored stress tests (41 Rust tests and 22 UI/cache workflows).
- `nsh test --target web --browser <Chromium>`: 22 passed, including removal of search highlights after closing Find.
- `scripts/test-browser-files.sh`: local picker/drop workflow passed.
- Native and wasm32 `cargo clippy` with warnings denied: passed.
- `tests/operations-layouts.steps.js`: passed. Match highlights are visible while Find is open. Closing Find leaves the grid without them, on both the 1440-point and 390-point layouts, and the mobile status line shows the full unsaved state.
- `nsh package --target web -o dist`: passed.
- The test browser does not report a desktop pointer, so the 34-point desktop grid density and compact controls are not shown in these captures. Import-layout captures were not regenerated for this operations pass.

## Limits

CSV/TSV values remain literal Text. There is no numeric/date inference, automatic
header inference, document history, persistence, or export/download UI. Sorting,
search, cell editing, literal replacement and bounded undo/redo stay in the
active in-memory document. The existing internal serializer remains available to
Rust callers.

Parsing and first-time column profiling are synchronous on the browser thread.
There are no size caps. Above 8 MiB, 100,000 records, 512 columns, 1,000,000
fields or a 1 MiB field, the review step asks for confirmation; files above 8 MiB
are confirmed before the browser reads them. These thresholds
are advisory, not large-file responsiveness benchmarks; very large input can make
the tab unresponsive or exhaust its memory. The grid shows 512-byte previews; the
inspector's full-value dialog renders 32,768 characters at a time and copy always
takes the whole value. They are not full-text editors. File selection/drop requires a modern browser. Clipboard
permission failures leave a manual paste field available.

The scoped duplicate-scrollbar CSS depends on NSH 0.4's browser DOM classes,
including the desktop bar's stacking. Visually check that horizontal scrolling
still works and that the bar stays under Diagnostics when upgrading NSH. Web is
the only deliverable.

## Editor captures

`tests/operations-layouts.steps.js` imports synthetic orders, sorts and replaces in
a selected column, verifies a visible highlight, checks that closing Find clears
it, and captures desktop, mobile, closed-find and
touch-editor states. Run `nsh test App.tsx --target web --browser /path/to/chrome < tests/operations-layouts.steps.js`. The captures are
`docs/screenshots/operations-{desktop,mobile,find-closed,find-closed-desktop,mobile-editor}.png`.
