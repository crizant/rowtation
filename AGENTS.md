# Project instructions

## Stack and targets

- Use Native Shell (NSH) **v0.5.0**, **mixed template**.
- Write UI in TSX; on web it runs on the browser's JavaScript engine.
- Put all data logic in Rust compiled to WASM: parsing, format detection,
  normalization, validation, sorting, searching, editing, replacement and export.
- Web only: use `nsh run --target web` and `nsh package --target web`.
  Do not add iOS, Android, native desktop or TUI deliverables.
- Keep `nsh.json` configured with `"rust": "."` and no `root` key.
- Use NSH's registered Rust API and generated types, not a separate ad hoc bridge.

## Implementation

- Read README.md, docs/product.md, docs/architecture.md and DESIGN.md first.
- TSX owns presentation and transient UI state; Rust owns datasets and operations.
  UI signals may mirror Rust results but must not become a second data engine.
- Components run once. Read changing props and signals inside JSX bindings or
  closures; do not destructure reactive props.
- Use semantic theme tokens and app-owned styled controls built on `nsh/base`.
- Verify APIs against `.nsh/types` or resolved Rust sources. Do not invent APIs.
- Treat file contents as untrusted data; never evaluate formulas, scripts or markup.
- Keep documentation honest about implemented and planned behavior.
- Do not commit `.nsh/`, `target/`, `dist/`, credentials or user datasets.

## Validation

- Run `cargo fmt --check`, `nsh check`, `nsh build` and `nsh test` for relevant changes.
- Verify browser interactions with `nsh test --target web` and visually inspect UI
  changes. Record unavailable checks and their concrete blockers.
- Package with `nsh package --target web -o dist` before claiming a web build works.
- Test data logic in Rust with synthetic fixtures, errors and meaningful round trips.

## When adding new npm dependencies

- Install from the npm registry without specifying versions.
- Do not choose historical versions unless there is a documented compatibility requirement.
- If a package cannot use the latest stable release, explain why.
