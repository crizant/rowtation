//! Search regressions for sparse structured rows; fixtures are synthetic.
use nsh_app_rowtation::{
    operations::SearchOptions,
    session::{DocumentSession, ImportOptions, RowWindow},
};
fn session(input: &str) -> DocumentSession {
    let mut options = ImportOptions::default();
    options.format = Some("ndjson".into());
    DocumentSession::new(input.as_bytes().to_vec(), options).unwrap()
}
fn find(s: &mut DocumentSession, term: &str, column: Option<u32>) -> u32 {
    s.search(SearchOptions {
        term: term.into(),
        column_id: column,
        ..Default::default()
    })
    .unwrap()
    .count
}
#[test]
fn ten_thousand_unique_columns_search_only_present_values() {
    let input = (0..10_000)
        .map(|i| format!("{{\"field_{i}\":\"needle\"}}\n"))
        .collect::<String>();
    let mut s = session(&input);
    assert_eq!(s.metadata().column_count, 10_000);
    // Only 60,000 searchable bytes: the former schema walk falsely exhausted
    // its 64-MiB limit after counting millions of implicit Missing cells.
    assert_eq!(find(&mut s, "absent", None), 0);
    assert_eq!(find(&mut s, "needle", None), 10_000);
    let last = s.search_match(9999).unwrap();
    assert_eq!(
        (
            last.row_id,
            last.column_id,
            last.row_index,
            last.start,
            last.end
        ),
        (10_000, 10_000, 9999, 0, 6)
    );
    assert!(s.read_cell(10_000, 10_000).unwrap().matched);
    assert!(!s.read_cell(1, 10_000).unwrap().matched);
    assert_eq!(find(&mut s, "needle", Some(5001)), 1);
    assert_eq!(s.search_match(0).unwrap().row_id, 5001);
    assert!(!s.read_cell(1, 1).unwrap().matched);
    assert_eq!(find(&mut s, "needle", None), 10_000);
    let replaced = s.replace_matches(None, "changed".into(), 0).unwrap();
    assert_eq!(
        (replaced.replacements, replaced.affected_cells),
        (10_000, 10_000)
    );
    assert_eq!(find(&mut s, "changed", None), 10_000);
    s.history(false).unwrap();
    assert_eq!(find(&mut s, "needle", None), 10_000);
    assert!(!s.metadata().dirty);
}
#[test]
fn sparse_search_retains_view_schema_occurrence_order_and_scoped_replacement() {
    let mut s = session(
        "{\"a\":\"hit hit\",\"b\":\"hit\",\"rank\":2}\n{\"b\":\"hit\",\"a\":\"雪 hit\",\"rank\":1}",
    );
    s.sort(Some(3), false).unwrap();
    assert_eq!(find(&mut s, "hit", None), 5);
    let identities = (0..5)
        .map(|i| {
            let m = s.search_match(i).unwrap();
            assert_eq!(m.revision, 0);
            (m.row_id, m.column_id, m.row_index, m.start, m.end)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        identities,
        [
            (2, 1, 0, 4, 7),
            (2, 2, 0, 0, 3),
            (1, 1, 1, 0, 3),
            (1, 1, 1, 4, 7),
            (1, 2, 1, 0, 3)
        ]
    );
    assert_eq!(s.search_index(1, 1, 4, 0), Some(3));
    let mut window = RowWindow::default();
    window.limit = 2;
    let snapshot = s.rows(window).unwrap();
    assert_eq!(snapshot.rows[0].id, 2);
    assert!(
        snapshot
            .rows
            .iter()
            .all(|r| r.cells[0].matched && r.cells[1].matched && !r.cells[2].matched)
    );
    let result = s.replace_matches(Some(3), "one".into(), 0).unwrap();
    assert_eq!((result.replacements, result.affected_cells), (1, 1));
    assert_eq!(s.read_cell(1, 1).unwrap().text.as_deref(), Some("hit one"));
    assert_eq!(find(&mut s, "hit", Some(2)), 2);
    s.replace_matches(None, "two".into(), 1).unwrap();
    assert_eq!(s.read_cell(2, 2).unwrap().text.as_deref(), Some("two"));
    assert_eq!(s.read_cell(2, 1).unwrap().text.as_deref(), Some("雪 hit"));
    s.history(false).unwrap();
    s.history(false).unwrap();
    assert_eq!(find(&mut s, "hit", None), 5);
    assert!(!s.metadata().dirty);
}
#[test]
fn sparse_typed_empty_and_absent_cells_keep_search_semantics() {
    let mut s = session(
        "{\"t\":true,\"n\":9007199254740993,\"empty\":\"\",\"null\":null}\n{\"other\":\"true\"}\n{}",
    );
    assert_eq!(find(&mut s, "true", None), 2);
    assert_eq!(find(&mut s, "9007199254740993", None), 1);
    assert_eq!(find(&mut s, "null", None), 0);
    assert_eq!(
        s.search(SearchOptions {
            term: "^$".into(),
            regex: Some(true),
            ..Default::default()
        })
        .unwrap()
        .count,
        1
    );
    let empty = s.search_match(0).unwrap();
    assert_eq!(
        (empty.row_id, empty.column_id, empty.start, empty.end),
        (1, 3, 0, 0)
    );
    assert!(
        s.search(SearchOptions {
            term: "true".into(),
            column_id: Some(999),
            ..Default::default()
        })
        .is_err()
    );
    assert!(s.search_match(0).is_none());
}
