# Contributing

Read [AGENTS.md](AGENTS.md), [product scope](docs/product.md),
[architecture](docs/architecture.md) and [UI standards](DESIGN.md) before editing.
Keep implemented behavior distinct from plans.

Use NSH CLI 0.4.0 and Rust dependencies aligned with that release. Install the
WASM target and web tools described in README.md. Do not add native packaging
or a second JavaScript data-processing implementation.

For changes affecting Rust or TSX, run:

```sh
cargo fmt --check
nsh check
nsh build
nsh test
scripts/test-browser.sh
scripts/test-browser-files.sh
nsh package --target web -o dist
```

Test meaningful data behavior and malformed input in Rust with synthetic
fixtures. Keep UI tests focused on meaningful workflows; avoid copying parser
assertions into browser tests or adding a matrix of cosmetic layout assertions. Inspect visible
changes in a real browser; behavior tests do not prove layout. Report unavailable
verification and its reason.

Commit Cargo.lock. Keep generated `.nsh/`, `target/` and `dist/` out of source
control; do not edit generated declarations or commit private user datasets.

Follow AGENTS.md for npm dependencies. Check Rust parser/writer dependencies for
browser WASM compatibility and document native-only feature exclusions.


## Continuous integration

[CI](.github/workflows/ci.yml) runs three independent jobs concurrently on pull
requests targeting `main`, pushes to `main`, and manual workflow runs:

- `Rust formatting`: `cargo fmt --check`, with only Rust and rustfmt setup.
- `Static checks and headless tests`: `nsh check`, `nsh build`, then `nsh test`,
  sharing compilation within the job.
- `Browser and WASM integration`: `scripts/test-browser.sh`, local file workflow checks, then `nsh package --target web -o dist`, production-package downloads/copy and export dialog captures, using installed Chrome. Screenshots are retained as the `export-dialog-screenshots` workflow artifact.
  This job installs/caches the wasm-bindgen CLI matching Cargo.lock.

The two NSH jobs share a matrix definition with fail-fast disabled, so a failure
in one suite does not cancel the other. Each gets its own runner and uses NSH
0.4.0 and Ubuntu 24.04's installed Chrome and LLVM tools.

Two caches reduce setup and compilation time:

- NSH's web-only distribution and downloaded TypeScript compiler, keyed by the
  NSH release, operating system and architecture.
- Cargo registry/git dependencies, `target/` (native and WASM builds), and NSH's
  generated runner workspace in `.nsh/rust/`. Keys include the compiler identity,
  NSH version, validation suite, Cargo manifests/lockfile and source fingerprint.
  Suite-specific keys let both parallel jobs save their own build results without
  competing for the same cache entry. Compatible prefix restores stay within the
  same suite and compiler, reusing dependencies after source or lockfile changes.

Successful runs save new cache entries; an exact hit reuses the existing entry.
Development debug symbols and incremental compilation are disabled in CI to keep
archives smaller. The first run is expected to be slower; each NSH job summary
reports exact cache hits. Update the relevant cache key's version prefix if its
storage policy changes (`nsh-tools-v1` and `rust-web-v2` currently).
Superseded runs for the same branch or pull request are canceled automatically.

The workflow uses read-only repository access and does not publish the website.
Cloudflare Pages builds and deploys independently when `main` changes. Its
GitHub integration build command is `sh scripts/cloudflare-pages-build.sh`, and
its output directory is `dist/Rowtation`. That script installs the wasm-bindgen
CLI version from `Cargo.lock` before packaging.


The local file/layout script uses a test-only browser host with synthetic File and
DragEvent objects. It temporarily configures that host in nsh.json and restores
the exact original file with an exit trap, including on failure. Run it serially
with other NSH commands and development servers. Production App.tsx never imports
the test host, and packaging runs after restoration.

Set ROWTATION_BROWSER to the executable when NSH cannot discover Chromium. Use
scripts/capture-layouts.sh to regenerate synthetic desktop/mobile screenshots.
