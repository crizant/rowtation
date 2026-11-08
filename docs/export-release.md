# Export milestone validation

Validated locally on 2026-11-08 with NSH 0.4.0, Rust/WASM,
Chrome for Testing 149 and Node 24 on macOS. All fixtures are synthetic.

Local results: `cargo fmt --check`, `nsh check` and `nsh build` passed.
`nsh test`: 114 passed, 7 existing benchmark/stress tests skipped. The shared
browser suite passed 44 checks; the final expanded export suite passed 3.
Local-file browser workflows, web packaging (13.4 MB), and the production
five-download/clipboard/cleanup/mobile-touch smoke passed.

## Conversion coverage

See [the supported conversion matrix and policies](export.md). Rust integration
fixtures cover CSV → edit → JSON, CSV → replace all → TSV, JSON → edit → JSON,
JSON → NDJSON, NDJSON → CSV, XML → JSON, supported JSON → XML and CSV round trips.
Additional cases cover reconstruction of nested objects/arrays and edited absent
paths, rejected scalar collisions/array holes, XML attributes/repeats/namespaces,
exact large integers and decimal tokens, leading zeros, null/missing/empty fields,
CSV quoting/Unicode/formula protection, original/current sort order, undo/redo,
filename sanitation/extensions and export errors without data loss.

UI workflows cover contextual controls, explicit warning acceptance, export copy,
keyboard guarding while the dialog is open, cancel/Escape, mobile layout and a
rejected XML conversion that leaves the document unchanged.

## Browser and release checks

Run the project checks from the repository root:

```sh
cargo fmt --check
nsh check
nsh build
nsh test
scripts/test-browser.sh
scripts/test-browser-files.sh
nsh package --target web -o dist
node scripts/test-export-package.mjs --build dist/Rowtation --browser "$ROWTATION_BROWSER"
```

The production smoke serves the unmodified static package locally, performs real
browser input, edits a cell, copies serialized output, downloads all five formats,
and verifies file contents, extensions, MIME types, temporary anchor removal and
object URL revocation. It then undoes the edit and uses emulated mobile touch to
open export, choose NDJSON, copy the restored data and cancel. Node 24 and an
explicit Chromium executable are required; there are no npm dependencies.

Cloudflare Pages still builds with `sh scripts/cloudflare-pages-build.sh` and
publishes `dist/Rowtation`. The deployable package is verified locally; the Linux
installer and an actual Pages deployment are outside this local macOS run.
CI runs the production smoke after packaging. No deployment or server has been
introduced into the product.

## Visual review

Generate the six export captures with:

```sh
mkdir -p docs/screenshots
nsh test App.tsx --target web --browser "$ROWTATION_BROWSER" < tests/export-layouts.steps.js
```

Reviewed centered desktop dialogs at 1280×800, the bottom sheet at 390×844, and
scrollable options with stationary actions at 320×568, including light/dark
appearance. Five format cards, readable NDJSON marks, 44-point touch controls,
primary Download, focus styling and safe-area padding follow the app controls.
The narrow primary spans the sheet width. Longer warnings scroll in the body.

Captures stay in ignored `docs/screenshots/` as required by the repository's
visual-validation policy. CI attaches them to its run as the
`export-dialog-screenshots` artifact, so PR reviewers can download them without
committing generated PNGs. Files: `export-desktop.png`, `export-desktop-dark.png`,
`export-mobile.png`, `export-mobile-warning.png`, `export-small.png` and
`export-small-dark.png`.

## Known limits

- Export is synchronous and holds full serialized output in memory. Very large
  output can pause or exhaust the tab; allocation aborts cannot be recovered.
- Supported XML is the documented tabular subset. Generic JSON → XML rejects
  null, empty containers and unsupported names/characters, with explicit type
  and array-identity warnings. XML → JSON warns about omitted envelope, namespace
  bindings, record names and interleaved child order.
- Conflicting added paths, array holes and unplaced XML edits block reconstruction;
  acknowledged flat JSON/NDJSON or delimited output is available.
- Browser download feedback reports a request, not a guaranteed saved file. Export
  never resets imported-baseline dirty state. Clipboard permissions can deny copy.
- Chromium was exercised locally; Safari/Firefox and deployed-origin permissions
  have not been independently validated. No arbitrary XML lossless claim or
  large-export performance claim is made.
- Excel remains future-only. No recent history, persistent storage or backend.
