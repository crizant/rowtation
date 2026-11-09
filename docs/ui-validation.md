# Validation

## Checks and web release

Use NSH 0.4.0 and the toolchain in [README](../README.md). Run NSH commands
serially: tests, the development server and packaging share generated runtime
paths. Set `ROWTATION_BROWSER` to a Chromium executable when discovery fails.

```sh
cargo fmt --check
nsh check
nsh build
nsh test
node --test scripts/web-metadata.test.mjs
ROWTATION_BROWSER=/path/to/chrome scripts/test-browser.sh
ROWTATION_BROWSER=/path/to/chrome scripts/test-browser-files.sh
nsh package --target web -o dist
node scripts/web-metadata.mjs dist/Rowtation/index.html
node scripts/web-metadata.mjs --check dist/Rowtation/index.html
node scripts/test-export-package.mjs --build dist/Rowtation --browser /path/to/chrome
```

The package smoke needs Node 24+, uses no npm dependencies, and serves the static
build over loopback. It checks homepage scroll bounds at fitting and short
viewports, edits a cell, copies output, downloads all five formats, verifies
contents/MIME/extensions and object-URL cleanup, then checks undo and mobile
touch export. A download request alone is not proof that a file was saved.
CI runs this smoke after packaging; it does not publish the site.

## Test ownership

- Rust tests own parsing, normalization, types, invalid input, search/replace,
  ordering, stable identity, history, export policies and meaningful round trips.
- UI tests own import/edit/search/export workflows, focus, cancellation, session
  disposal and grid recycling. Search lifecycle tests count real bridge calls;
  responsiveness tests require an event-loop yield rather than a timing threshold.
- `scripts/test-browser.sh` runs shared tests with browser input and Rust/WASM.
  It excludes `*.headless.test.ts[x]`: NSH 0.4's web host rejects the iOS idiom
  used by the separate long-press test. This does not add a native delivery target.
- `scripts/test-browser-files.sh` covers File/FileList/DragEvent workflows with
  synthetic data, including delayed reads and failed-import preservation. It
  temporarily changes `nsh.json` and restores it with an exit trap; production
  never imports its test host.
- Stress and performance fixtures are opt-in. See [scroll](scroll-performance.md),
  [search](search-performance.md) and [structured import](structured-performance.md)
  measurements for commands, conditions and limitations.

## Visual checks

Generate only the captures relevant to the change:

| Area | Command or steps file |
| --- | --- |
| Home, import, inspector and grid | `scripts/capture-layouts.sh` |
| Search, replacement and editing | `tests/operations-layouts.steps.js` |
| Search scope and shared dialog actions | `tests/mobile-controls.steps.js` |
| Export formats, warnings and dark appearance | `tests/export-layouts.steps.js` |

Run a steps file with `nsh test App.tsx --target web --browser /path/to/chrome < tests/FILE.steps.js`.
Captures use synthetic data and go to ignored `docs/screenshots/`. Inspect them;
passing behavior tests do not establish visual correctness. CI retains export
captures as its `export-dialog-screenshots` artifact.

Check 320- and 390-point phones, an intermediate width, a wide desktop, short
viewports, enlarged text and light/dark appearance. Content that fits must not
scroll through empty space; content that overflows must keep its final action
reachable. Also check dialog action lanes and safe areas, long column names,
search expansion, grid selection and a single horizontal grid scrollbar.
Recheck the scoped NSH browser compatibility styles after an NSH upgrade.

Chromium is the automated browser target. Safari/Firefox, actual phone keyboards,
and deployed-origin clipboard/download permissions need separate validation.
Current data-format and memory limits belong in [product](product.md),
[operations](operations.md) and [export](export.md), rather than validation logs.
