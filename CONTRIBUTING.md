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
nsh test --target web
nsh package --target web -o dist
```

Test meaningful data behavior and malformed input in Rust with synthetic
fixtures. Add UI tests for complete workflows when implemented. Inspect visible
changes in a real browser; behavior tests do not prove layout. Report unavailable
verification and its reason.

Commit Cargo.lock. Keep generated `.nsh/`, `target/` and `dist/` out of source
control; do not edit generated declarations or commit private user datasets.

Follow AGENTS.md for npm dependencies. Check Rust parser/writer dependencies for
browser WASM compatibility and document native-only feature exclusions.


## Continuous integration

[CI](.github/workflows/ci.yml) runs one `Validate web` job on pull requests targeting
`main`, pushes to `main`, and manual workflow runs. It checks Rust formatting,
Rust/TSX types, bundler diagnostics, headless tests and browser/WASM integration.
It uses NSH 0.4.0 and Ubuntu 24.04's installed Chrome and LLVM tools.

Two caches reduce setup and compilation time:

- NSH's web-only distribution and downloaded TypeScript compiler, keyed by the
  NSH release, operating system and architecture.
- Cargo registry/git dependencies, `target/` (native and WASM builds), and NSH's
  generated runner workspace in `.nsh/rust/`. Keys include the compiler identity,
  NSH version, Cargo manifests/lockfile and source fingerprint. Compatible prefix
  restores reuse dependencies after source or lockfile changes.

Successful runs save new cache entries; an exact hit reuses the existing entry.
Development debug symbols and incremental compilation are disabled in CI to keep
archives smaller. The first run is expected to be slower; the job summary reports
exact cache hits. Update the cache key's `v1` prefix if its storage policy changes.
Superseded runs for the same branch or pull request are canceled automatically.

The workflow uses read-only repository access and does not publish the website.
Cloudflare continues to build and deploy independently when `main` changes.
