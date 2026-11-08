# Delivery plan

The mixed foundation, data core, and CSV/TSV import/inspection milestones are
implemented, along with sorting, search, editing, find/replace and undo/redo.
Additional formats and downloads remain pending. Complete and test each slice before marking
its formats or operations as supported in the README.

1. **Mixed foundation (implemented):** wire TSX to Rust, align the NSH release, verify generated
   types, bridge tests and static web packaging.
2. **Data core and delimited text (implemented):** values, stable IDs and structured errors;
   CSV/TSV parsing and serialization with Rust fixtures and round trips.
3. **Import and inspection (implemented):** paste, drop and Open; parsing options, errors,
   row counts and a virtualized table. Verify keyboard and touch access.
4. **Transformations (implemented):** Rust-owned sort and search, cell edits, scoped find/replace
   and clear dirty/error states. Test identity under searching and sorting.
   Search still rejects a scoped payload above 64 MiB or more than 100,000 matches,
   with no partial list, and runs on the calling thread. Widening that is engine
   follow-up [issue #9](https://github.com/crizant/rowtation/issues/9), not an import cap.
5. **Structured formats:** JSON and NDJSON, then XML with explicit mapping rules
   and visible loss warnings. Test missing, null, nested and mixed-type values.
6. **Excel:** WASM-compatible readers/writers, worksheet selection, dates,
   precision and formulas. Ship `.xlsx` first; evaluate legacy `.xls` input.
7. **Export and release:** options, scope, loss preview and downloads; browser
   round trips, large-input measurements, accessibility and deployment smoke tests.

Import and inspection retain only the active in-memory document; session history
and persistent document storage are outside this milestone.
