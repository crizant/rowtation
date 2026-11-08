# Rowtation

A browser-based data converter built with **Native Shell (NSH) v0.4.0**, using
its **mixed template**: TSX renders the UI on the browser's JavaScript engine,
and Rust compiles to WebAssembly for all data logic. **Web is the only delivery
target.** There are no iOS, Android, native desktop or TUI builds.

## Product scope

Users will paste, drop or open CSV, TSV, JSON, NDJSON and XML files;
inspect them as a table; sort, search, edit cells and find and replace values;
then export and download the result.

**Status:** CSV/TSV, JSON, NDJSON and tabular XML import and inspection are implemented: choose a local file,
drop one into the browser, or paste text; review the separator and header option;
then inspect a virtualized grid with keyboard/touch cell selection and column
statistics. Sorting, scoped search, literal/regex find and replace, cell editing,
bounded undo/redo and accurate modified-cell counts are implemented. Desktop uses
a right inspector; narrow browsers use a contextual panel. Five-format export, browser download and clipboard copy are implemented. Excel is a possible future feature only. See [export policies and conversion matrix](docs/export.md). See [structured import rules](docs/structured-imports.md) for nesting, exact types and XML limitations. See [operation semantics](docs/operations.md).

Data stays local. There is no backend, upload, telemetry, persistent storage or
session history. Closing an unmodified document releases its Rust session.
Closing or replacing a modified document asks first, even after export; confirming discards those changes. Downloads do not reset the imported baseline. Import failures preserve
the active document; a successful replacement releases the old session.

See [export release validation](docs/export-release.md) for production-download checks and export screenshots.

See [UI validation instructions](docs/ui-validation.md) for local desktop/mobile
screenshot generation, visual inspection guidance, and browser validation commands.

## Document engine

CSV/TSV input is UTF-8, with an optional initial BOM. Quoted fields, doubled quotes,
embedded newlines, CRLF/LF (and bare CR), Unicode, empty fields and blank records
are supported. Options explicitly select the delimiter and header mode; the low-level parser does no format
or header detection. The import UI suggests comma/tab using a quote-aware Rust
sample and lets the user correct it. Header mode is explicit, defaulting to on. Every field stays text: `00123`, `true`, `null` and
formula-like values retain their literal values. Nothing is evaluated.

Headers retain order, duplicate names and empty names. No-header input generates
`Column 1`, `Column 2`, etc. Short rows expose missing trailing cells; longer rows
extend the schema with generated columns. A blank record is one empty text field,
with any remaining columns missing. Empty input has no rows or columns. A terminal
record separator ends the last record; each additional blank line adds a row.

Export supports CSV, TSV, JSON, NDJSON and supported tabular XML with explicit
mapping policies, source reconstruction, formula-safe text and original/current
sorted ordering. See [the conversion matrix and limits](docs/export.md). Export
does not change document dirty state or revision.

There are no size caps on input, rows, columns, stored fields, value length or
serialized output; the practical ceiling is the browser tab's memory and WASM's
4 GiB address space. A picked or dropped file above 8 MiB is held unread until
the user confirms; it is then read in 16 MiB slices into one Rust buffer that JS
never mirrors. Delimited review measures the whole input in Rust. Structured review detects
format from a bounded prefix, then parses once after Import; exact shape warnings
retain the parsed candidate for confirmation. The workflow asks for confirmation ("Import anyway") above 8 MiB, 100,000 records, 512 columns,
1,000,000 fields or a 1 MiB field. These thresholds are advisory, not measured
performance limits. Row windows return at most 256 rows and can be narrowed to a
column range and to 512-byte previews. The grid keeps a bounded 256-row cache
with overscan for the visible columns, reusing overlapping rows. The inspector
reads whole values on demand. Column statistics are exact and are counted in
slices between frames. Parsing remains synchronous on the browser thread.

Press Ctrl+Shift+D to show local diagnostics: read, inspect, parse and profiling
times; row-window time with Rust's measured share and an estimated transfer share
from the same call; time from an update to the next animation frame (which excludes
paint); and WebAssembly memory. They appear in the panel and console only. Nothing is recorded while it is off or sent anywhere.
`cargo test --release --test stress -- --ignored --nocapture` runs the large-input
stress tests and a native benchmark (`ROWTATION_BENCH_MB`, default 64, or
`ROWTATION_BENCH_ROWS` for an exact row count). Sort and search in that
benchmark run synchronously on the calling thread. They are not an import-size cap.
Search also stops, with no partial matches, above 64 MiB of scoped text or
100,000 matches ([issue #9](https://github.com/crizant/rowtation/issues/9)).

See [architecture and API contracts](docs/architecture.md) for Rust/TSX usage,
identity, error and disposal rules.
See the [2-million-row production profile](docs/scroll-performance.md) for measured
cache/Inspector costs and the remaining NSH scaled-scroll limitation.

## Development

Prerequisites:

- NSH CLI **0.4.0** (`nsh --version`). Keep NSH crates aligned with the CLI.
- Rust with Cargo and the `wasm32-unknown-unknown` standard library. With rustup,
  install it using `rustup target add wasm32-unknown-unknown`.
- LLVM `clang` and `llvm-ar`, available on `PATH` for the mixed web build.
- Chrome, Edge or Chromium for browser tests. Select another compatible browser
  with `--browser /path/to/executable`.
- `wasm-bindgen` CLI **0.2.130**, matching the locked WASM bindings. The browser
  file adapter calls browser APIs through these bindings:
  `cargo install wasm-bindgen-cli --version 0.2.130 --locked`.

No npm dependencies are currently needed. See the
[NSH documentation](https://nsh.dev) for CLI setup.

### macOS toolchain setup

Use a [rustup-managed Rust installation](https://rust-lang.org/tools/install/)
so the browser target can be installed independently. Homebrew's `rust` formula
alone does not include the WASM standard library. An existing Homebrew Rust
installation can remain installed; put rustup's tools first on `PATH`.

```sh
brew install llvm
rustup target add wasm32-unknown-unknown
rustup component add rustfmt
```

On Apple Silicon, include `~/.cargo/bin` and `/opt/homebrew/opt/llvm/bin` before
the other compiler paths. For fish, persist these paths with:

```fish
fish_add_path --universal --move $HOME/.cargo/bin /opt/homebrew/opt/llvm/bin
```

For zsh, add this to `~/.zshrc`, then open a new terminal:

```sh
export PATH="$HOME/.cargo/bin:/opt/homebrew/opt/llvm/bin:$PATH"
```

Verify `rustc --print sysroot` points into `.rustup`,
`rustup target list --installed` lists `wasm32-unknown-unknown`, and
`clang` and `llvm-ar` resolve. If a mixed web build reports `can't find crate for
core`, check these paths and install the target for the active toolchain.

### Commands

```sh
nsh run --target web                 # Develop in the browser
nsh check                            # Check Rust and generated TSX types
nsh build                            # Validate and bundle TSX
nsh test                             # Rust tests and headless UI tests
scripts/test-browser.sh               # Shared tests with real browser input
nsh package --target web -o dist      # Build the deployable static website
node scripts/web-metadata.mjs dist/Rowtation/index.html
```

NSH may compile a local helper to describe Rust exports or run headless tests.
These are development tools; the shipped app targets the browser only.

NSH generates TypeScript declarations in `.nsh/types/rowtation/native.d.ts` from
the registered Rust API. Do not edit generated declarations. TSX edits hot update;
Rust changes rebuild the runtime.

## Preview and deploy

The web package is written to `dist/Rowtation/`. Serve the complete directory,
including WASM, JavaScript and other generated assets, over HTTP:

```sh
python3 -m http.server 8080 --directory dist/Rowtation
```

Open http://localhost:8080. Deploy that directory to a static host; a backend
service is not required by the planned architecture. Packaging does not publish
the website. Test imports against the deployed origin before release. Validate downloads and clipboard permissions on the deployed origin before release.

Cloudflare Pages builds from the GitHub integration. Its build command is
`sh scripts/cloudflare-pages-build.sh`, and its output directory is
`dist/Rowtation`. The script installs the Linux toolchain, NSH 0.4.0, and the
wasm-bindgen CLI version in `Cargo.lock`, runs `nsh package --target web -o dist`,
then writes the page title and meta description into `dist/Rowtation/index.html`.
NSH’s page template has no description field, so that stamp is what puts them in
the HTML before any script runs. `nsh run` still serves NSH’s development page.
It is for the Pages Linux image; local macOS builds use the commands above.

## Project layout

```text
App.tsx                Responsive import and inspection screens, TSX entry
App.test.tsx           NSH bridge and app-runtime smoke tests
components/            Styled controls, DataGrid and Inspector
src/lib.rs             Rust API registered as rowtation/native
src/document/          Canonical values, identities and storage access
src/formats/           CSV/TSV and structured adapters, multi-format export
src/session.rs         Retained NSH session, row windows and profile cache
src/inspection.rs      Rust delimiter suggestions, size measurement and profile records
src/browser_import.rs Browser file input/drop adapter through registered NSH API
src/browser_styles.rs Scoped browser presentation styles through registered API
src/error.rs           Recoverable engine errors
tests/                 Synthetic Rust tests and test-only TSX bridge entry
Cargo.toml             Rust crate and NSH dependencies
Cargo.lock             Resolved application dependencies
nsh.json               App metadata and mixed-template configuration
scripts/               Pages package command and local browser checks
docs/                  Product scope, architecture and delivery plan
DESIGN.md              UI standards
AGENTS.md              Contributor and coding-agent constraints
```

Read [product requirements](docs/product.md), [architecture](docs/architecture.md),
[delivery plan](docs/roadmap.md) and [contribution guidance](CONTRIBUTING.md) before
implementing converter features.
