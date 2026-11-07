# Rowtation

A browser-based data converter built with **Native Shell (NSH) v0.4.0**, using
its **mixed template**: TSX renders the UI on the browser's JavaScript engine,
and Rust compiles to WebAssembly for all data logic. **Web is the only delivery
target.** There are no iOS, Android, native desktop or TUI builds.

## Product scope

Users will paste, drop or open CSV, TSV, JSON, NDJSON, XML and Excel files;
inspect them as a table; sort, search, edit cells and find and replace values;
then export and download the result.

**Status:** the Rust document core and CSV/TSV parsing/serialization are implemented,
with a retained NSH session API and bounded row retrieval. The screen remains a
placeholder. Import controls, the table, editing, other formats and browser
downloads are planned. All data processing stays local; no backend, cloud storage
or telemetry is added.

## Document engine

CSV/TSV input is UTF-8, with an optional initial BOM. Quoted fields, doubled quotes,
embedded newlines, CRLF/LF (and bare CR), Unicode, empty fields and blank records
are supported. Options explicitly select the delimiter and header mode; no format
or header detection runs. Every field stays text: `00123`, `true`, `null` and
formula-like values retain their literal values. Nothing is evaluated.

Headers retain order, duplicate names and empty names. No-header input generates
`Column 1`, `Column 2`, etc. Short rows expose missing trailing cells; longer rows
extend the schema with generated columns. A blank record is one empty text field,
with any remaining columns missing. Empty input has no rows or columns. A terminal
record separator ends the last record; each additional blank line adds a row.

Serialization returns UTF-8 bytes with configurable delimiter/header inclusion,
standard quoting and LF record separators. It preserves field text and row order,
including embedded CR/LF. It does not reproduce original quote style, BOM or record
separator style. Trailing missing fields remain absent. Null, interior missing
and non-text typed values require a future explicit conversion policy and currently
fail with `LOSSY_EXPORT`. Generated column names are included when headers are
requested. Spreadsheet formula protection/downloads belong to a later export UI;
this internal serializer emits literal text unchanged.

Limits reject excessive input without truncation: 8 MiB input, 100,000 data rows,
512 columns, 1,000,000 stored fields, and 32 MiB serialized output. Row windows are
limited to 256 rows, 16,384 logical cells and 1 MiB of text. These are conservative
resource guards, not measured large-file performance guarantees. Parsing remains
synchronous on the browser thread.

See [architecture and API contracts](docs/architecture.md) for Rust/TSX usage,
identity, error and disposal rules.

## Development

Prerequisites:

- NSH CLI **0.4.0** (`nsh --version`). Keep NSH crates aligned with the CLI.
- Rust with Cargo and the `wasm32-unknown-unknown` standard library. With rustup,
  install it using `rustup target add wasm32-unknown-unknown`.
- LLVM `clang` and `llvm-ar`, available on `PATH` for the mixed web build.
- Chrome, Edge or Chromium for browser tests. Select another compatible browser
  with `--browser /path/to/executable`.
- The `wasm-bindgen` CLI version requested by NSH if it reports that it is missing.

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
nsh test --target web                 # Test with real browser input
nsh package --target web -o dist      # Build the deployable static website
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
the website. Test imports and downloads against the deployed origin before release.

## Project layout

```text
App.tsx                TSX entry and placeholder screen
App.test.tsx           NSH bridge and app-runtime smoke tests
components/            Shared styled NSH controls
src/lib.rs             Rust API registered as rowtation/native
src/document/          Canonical values, identities and bounded storage access
src/formats/           CSV/TSV parser, validation and serializer
src/session.rs         Retained NSH document session and transport records
src/error.rs           Recoverable engine errors
tests/                 Synthetic Rust tests and test-only TSX bridge entry
Cargo.toml             Rust crate and NSH dependencies
Cargo.lock             Resolved application dependencies
nsh.json               App metadata and mixed-template configuration
docs/                  Product scope, architecture and delivery plan
DESIGN.md              UI standards
AGENTS.md              Contributor and coding-agent constraints
```

Read [product requirements](docs/product.md), [architecture](docs/architecture.md),
[delivery plan](docs/roadmap.md) and [contribution guidance](CONTRIBUTING.md) before
implementing converter features.
