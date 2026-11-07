# Delivery plan

All converter milestones are pending. Complete and test each slice before marking
its formats or operations as supported in the README.

1. **Mixed foundation:** wire TSX to Rust, align the NSH release, verify generated
   types, bridge tests and static web packaging.
2. **Data core and delimited text:** values, stable IDs and structured errors;
   CSV/TSV parsing and serialization with Rust fixtures and round trips.
3. **Import and inspection:** paste, drop and Open; parsing options, errors,
   row counts and a virtualized table. Verify keyboard and touch access.
4. **Transformations:** Rust-owned sort and search, cell edits, scoped find/replace
   and clear dirty/error states. Test identity under filtering and sorting.
5. **Structured formats:** JSON and NDJSON, then XML with explicit mapping rules
   and visible loss warnings. Test missing, null, nested and mixed-type values.
6. **Excel:** WASM-compatible readers/writers, worksheet selection, dates,
   precision and formulas. Ship `.xlsx` first; evaluate legacy `.xls` input.
7. **Export and release:** options, scope, loss preview and downloads; browser
   round trips, resource limits, accessibility and deployment smoke tests.

Remove the counter when the first real import-to-export slice replaces it.
