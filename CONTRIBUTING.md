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
