# Architecture

## Execution model

Rowtation uses NSH 0.4.0's mixed template. `App.tsx` is the entry; `nsh.json`
points `rust` at the root crate. On web, the browser runs TSX's compiled
JavaScript and loads Rust as WebAssembly. QuickJS is not the browser's
application JavaScript engine. There is no server-side data engine.

`#[native_module("rowtation/native")]` in `src/lib.rs` registers the Rust API.
NSH generates corresponding TypeScript declarations. Keep NSH crates aligned
with CLI 0.4.0 and commit Cargo.lock for the resolved application build.

## Ownership boundary

| Rust / WASM | TSX / browser integration |
| --- | --- |
| Decode, detect, parse and validate formats | Collect pasted text and opened/dropped file bytes |
| Own datasets, schemas, stable row/column IDs | Compose screens and styled NSH components |
| Sort, search, edit and replace | Own focus, selection, dialogs and editing drafts |
| Serialize exports and report conversion loss | Request export and trigger browser download |
| Return structured errors and operation results | Display errors, progress and confirmations |

The counter is a temporary integration smoke test, not the future dataset API.
No converter methods or parser dependencies exist yet.

## Planned data flow

1. Browser integration passes bytes or pasted text and parsing options to Rust.
2. Rust validates input and creates a document with stable row and column IDs.
3. TSX requests schema and bounded row windows for the visible table. Avoid copying
   the complete dataset across the bridge on every interaction.
4. UI commands call Rust with document and cell identities. Rust applies changes
   and returns revised snapshots, counts or errors for UI signals to mirror.
5. Rust serializes the chosen dataset/view into bytes. Browser integration supplies
   a filename and MIME type and starts the download.

Use a registered class for a retained document session; release it on close.
Do not keep duplicate authoritative arrays in TSX. Define the exact API, value
model and error types during the data-core milestone.

## Planned modules

- `src/document/`: canonical values, schema, IDs and document lifecycle.
- `src/formats/`: detection, parser/serializer adapters and conversion options.
- `src/operations/`: sorting, search, cell edits and find/replace.
- `pages/`: import and document screens composed from shared controls.
- `components/`: styled controls and virtualized table presentation.
- `tests/fixtures/`: small synthetic inputs and expected conversion outputs.

Create modules when needed; these paths describe intended boundaries.

## Browser constraints

Choose crates with verified `wasm32-unknown-unknown` support. Keep filesystem,
native threading and native networking assumptions out of the data core. Use
NSH's supported web integration for file access and downloads; verify exact APIs
against generated types before implementation.

WASM alone does not make long computations non-blocking. Measure parsing and
replacement work; decide on chunking or a compatible worker boundary before
promising large-file responsiveness. An async signature alone is insufficient.
Avoid interpreting imported XML/HTML as UI or executing imported formulas.

## Verification

Rust tests cover parser edge cases, stable identity, sorting, replacement and
export correctness. Bridge tests cover argument/result contracts and errors.
Browser tests cover paste/drop/open → table → edit → export/download. Visually
inspect focus, both appearances, narrow viewports and scrolling. Local helpers
may be compiled for tests and type generation; only web artifacts are distributed.
