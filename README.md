# Rowtation

A browser-based data converter built with **Native Shell (NSH) v0.4.0**, using
its **mixed template**: TSX renders the UI on the browser's JavaScript engine,
and Rust compiles to WebAssembly for all data logic. **Web is the only delivery
target.** There are no iOS, Android, native desktop or TUI builds.

## Product scope

Users will paste, drop or open CSV, TSV, JSON, NDJSON, XML and Excel files;
inspect them as a table; sort, search, edit cells and find and replace values;
then export and download the result.

**Status:** this repository is the mixed-project foundation. The current screen
is a counter that verifies the TSX-to-Rust bridge. File import, the data grid,
transformations and export are planned, not implemented.

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

Automatic deployment to Cloudflare Pages is configured through GitHub Actions.
See [deployment setup and activation](docs/deployment.md) for credentials, status
and manual deployment instructions.

## Project layout

```text
App.tsx                TSX entry and starter UI
App.test.tsx           UI bridge smoke test
components/            Shared styled NSH controls
src/lib.rs             Rust API registered as rowtation/native
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
