use nsh_app_rowtation::{
    operations::SearchOptions,
    session::{DocumentSession, ImportOptions, RowWindow},
};
fn session(text: &str) -> DocumentSession {
    DocumentSession::new(text.as_bytes().to_vec(), ImportOptions::default()).unwrap()
}
fn find(s: &mut DocumentSession, term: &str, column: Option<u32>, case: bool, regex: bool) -> u32 {
    s.search(SearchOptions {
        term: term.into(),
        column_id: column,
        case_sensitive: Some(case),
        regex: Some(regex),
    })
    .unwrap()
    .count
}
fn ids(s: &DocumentSession) -> Vec<u32> {
    let mut w = RowWindow::default();
    w.limit = 256;
    s.rows(w).unwrap().rows.iter().map(|r| r.id).collect()
}
fn value(s: &DocumentSession, row: u32, col: u32) -> String {
    s.read_cell(row, col).unwrap().text.unwrap_or_default()
}
#[test]
fn stable_text_sort_and_reset_preserve_raw_values() {
    let mut s = session("v\n2\n10\n2\n001\n\n");
    s.sort(Some(1), false).unwrap();
    assert_eq!(ids(&s), [4, 2, 1, 3, 5]);
    assert_eq!(value(&s, 4, 1), "001");
    assert_eq!(s.metadata().revision, 0);
    s.sort(Some(1), true).unwrap();
    assert_eq!(ids(&s), [5, 1, 3, 2, 4]);
    s.sort(None, false).unwrap();
    assert_eq!(ids(&s), [1, 2, 3, 4, 5]);
}
#[test]
fn scoped_unicode_and_case_search() {
    let mut s = session("a,b\nÉté,été été\n雪,ÉTÉ");
    assert_eq!(find(&mut s, "été", None, false, false), 4);
    let mut window = RowWindow::default();
    window.limit = 2;
    assert!(s.rows(window).unwrap().rows[0].cells[0].matched);
    assert!(s.read_cell(1, 1).unwrap().matched);
    assert_eq!(find(&mut s, "été", Some(2), true, false), 2);
    assert_eq!(find(&mut s, "雪", Some(1), true, false), 1);
    let m = s.search_match(0).unwrap();
    assert_eq!((m.row_id, m.column_id, m.start, m.end), (2, 1, 0, 3));
    assert_eq!(find(&mut s, "", None, false, false), 0);
    assert_eq!(find(&mut s, "absent", None, false, false), 0);
}
#[test]
fn regex_errors_zero_width_and_literal_replacement() {
    let mut s = session("a\n雪雪");
    assert!(
        s.search(SearchOptions {
            term: "(".into(),
            regex: Some(true),
            ..Default::default()
        })
        .is_err()
    );
    assert!(
        s.search(SearchOptions {
            term: "(a)\\1".into(),
            regex: Some(true),
            ..Default::default()
        })
        .is_err()
    );
    assert_eq!(find(&mut s, "^|$", None, true, true), 2);
    let r = s.replace_matches(None, "$1\\".into(), 0).unwrap();
    assert_eq!(r.replacements, 2);
    assert_eq!(value(&s, 1, 1), "$1\\雪雪$1\\");
}
#[test]
fn replace_one_occurrence_and_all_are_single_transactions() {
    let mut s = session("a,b\ncat cat,cat\ncat,dog");
    assert_eq!(find(&mut s, "cat", Some(1), true, false), 3);
    let r = s.replace_matches(Some(1), "kitten".into(), 0).unwrap();
    assert_eq!((r.replacements, r.affected_cells, r.revision), (1, 1, 1));
    assert_eq!(value(&s, 1, 1), "cat kitten");
    assert_eq!(value(&s, 1, 2), "cat");
    assert_eq!(find(&mut s, "cat", Some(1), true, false), 2);
    let r = s.replace_matches(None, "$".into(), 1).unwrap();
    assert_eq!((r.replacements, r.affected_cells, r.revision), (2, 2, 2));
    assert_eq!(s.edit_status().modified_cells, 2);
    s.history(false).unwrap();
    assert_eq!(value(&s, 1, 1), "cat kitten");
    assert_eq!(value(&s, 2, 1), "cat");
    s.history(false).unwrap();
    assert!(!s.metadata().dirty);
    assert_eq!(s.edit_status().modified_cells, 0);
    s.history(true).unwrap();
    assert_eq!(value(&s, 1, 1), "cat kitten");
}
#[test]
fn sorted_and_searched_edit_uses_stable_identity() {
    let mut s = session("id,name\nb,Bob\na,Alice");
    s.sort(Some(1), false).unwrap();
    assert_eq!(ids(&s), [2, 1]);
    assert_eq!(find(&mut s, "Alice", None, false, false), 1);
    let m = s.search_match(0).unwrap();
    assert_eq!(m.row_index, 0);
    s.edit_cell(m.row_id, m.column_id, "Ann".into(), m.revision)
        .unwrap();
    assert_eq!(value(&s, 2, 2), "Ann");
    assert_eq!(value(&s, 1, 2), "Bob");
    assert!(s.edit_cell(1, 2, "bad".into(), 0).is_err());
    assert_eq!(value(&s, 1, 2), "Bob");
}
#[test]
fn dirty_restoration_noop_redo_branch_and_missing_roundtrip() {
    let mut s = session("a,b\n1\n2,x");
    s.edit_cell(1, 1, "1".into(), 0).unwrap();
    assert_eq!(s.metadata().revision, 0);
    assert!(!s.edit_status().can_undo);
    let original = s.serialize(Default::default()).unwrap();
    s.edit_cell(1, 2, "".into(), 0).unwrap();
    assert!(s.metadata().dirty);
    s.history(false).unwrap();
    assert!(!s.metadata().dirty);
    assert_eq!(s.serialize(Default::default()).unwrap(), original);
    s.history(true).unwrap();
    s.edit_cell(1, 2, "new".into(), 3).unwrap();
    assert!(!s.edit_status().can_redo);
    s.edit_cell(1, 1, "other".into(), 4).unwrap();
    s.edit_cell(1, 1, "1".into(), 5).unwrap();
    assert_eq!(s.edit_status().modified_cells, 1);
}
#[test]
fn profile_cache_and_partial_job_invalidated_by_edit_and_undo() {
    let mut s = session("a\nx\nx");
    assert_eq!(s.profile(1).unwrap().unique_count, 1);
    s.edit_cell(1, 1, "y".into(), 0).unwrap();
    assert_eq!(s.profile(1).unwrap().unique_count, 2);
    s.history(false).unwrap();
    assert_eq!(s.profile(1).unwrap().unique_count, 1);
}
#[test]
fn bounds_are_atomic_and_history_is_bounded() {
    let mut s = session("a\nx");
    assert!(s.edit_cell(1, 1, "x".repeat(16 * 1024 * 1024), 0).is_err());
    assert_eq!(s.metadata().revision, 0);
    for i in 0..60 {
        s.edit_cell(1, 1, i.to_string(), s.metadata().revision)
            .unwrap();
    }
    for _ in 0..60 {
        s.history(false).unwrap();
    }
    assert_eq!(value(&s, 1, 1), "9");
    assert!(!s.edit_status().can_undo);
}
#[test]
fn replace_counts_occurrences_even_when_value_unchanged() {
    let mut s = session("a\nx x");
    find(&mut s, "x", None, true, false);
    let r = s.replace_matches(None, "x".into(), 0).unwrap();
    assert_eq!((r.replacements, r.affected_cells, r.revision), (2, 0, 0));
    assert!(!s.metadata().dirty);
}

#[test]
fn undo_and_redo_of_direct_baseline_restoration_track_original() {
    let mut s = session("a\nx");
    s.edit_cell(1, 1, "y".into(), 0).unwrap();
    s.edit_cell(1, 1, "x".into(), 1).unwrap();
    assert!(!s.metadata().dirty);
    s.history(false).unwrap();
    assert!(s.metadata().dirty);
    assert_eq!(s.edit_status().modified_cells, 1);
    s.history(false).unwrap();
    assert!(!s.metadata().dirty);
    s.history(true).unwrap();
    assert!(s.metadata().dirty);
    s.history(true).unwrap();
    assert!(!s.metadata().dirty);
}

#[test]
fn search_index_and_snippet_stay_in_rust() {
    let mut s = session("a\nBrisbane Brisbane");
    assert_eq!(find(&mut s, "Brisbane", None, true, false), 2);
    assert_eq!(s.search_index(1, 1, 9, 0), Some(1));
    assert_eq!(s.search_index(1, 1, 0, 1), Some(0));
    assert_eq!(s.search_index(1, 1, 4, 99), None);
    assert_eq!(s.search_snippet(0, 512).as_deref(), Some(""));
    let mut s = session(&format!("a\n{}{}", "x".repeat(520), "N".repeat(800)));
    assert_eq!(find(&mut s, &"N".repeat(800), None, true, false), 1);
    let snippet = s.search_snippet(0, 512).unwrap();
    assert_eq!(snippet.len(), 512);
    assert!(snippet.chars().all(|c| c == 'N'));
    assert!(s.search_snippet(3, 512).is_none());
}

#[test]
fn search_limits_clear_old_results_and_reject_partial_replace() {
    let mut s = session(&format!("a\n{}", "x".repeat(100_001)));
    assert_eq!(find(&mut s, "xx", None, true, false), 50_000);
    assert!(
        s.search(SearchOptions {
            term: "x".into(),
            ..Default::default()
        })
        .is_err()
    );
    assert!(s.search_match(0).is_none());
    assert_eq!(
        s.replace_matches(None, "y".into(), 0)
            .unwrap()
            .affected_cells,
        0
    );
    assert!(!s.metadata().dirty);
    assert!(
        s.search(SearchOptions {
            term: "x".repeat(4097),
            ..Default::default()
        })
        .is_err()
    );
}

#[test]
fn partial_profile_is_restarted_after_mutation() {
    let mut s = session("a\nx\nx\nx");
    assert!(s.profile_step(1, 1).unwrap().profile.is_none());
    s.edit_cell(1, 1, "y".into(), 0).unwrap();
    assert_eq!(s.profile(1).unwrap().unique_count, 2);
    s.history(false).unwrap();
    assert_eq!(s.profile(1).unwrap().unique_count, 1);
}

#[test]
fn highlight_windows_follow_view_order_and_hide_unfinished_or_cancelled_searches() {
    let mut s = session("id,a,b\nz,hit,other\na,other,hit hit");
    let window = || {
        let mut w = RowWindow::default();
        w.limit = 2;
        w.column_offset = Some(1);
        w.column_limit = Some(2);
        w
    };
    assert_eq!(find(&mut s, "hit", None, false, false), 3);
    assert_eq!(
        s.search_highlights(window()).unwrap(),
        [vec![true, false], vec![false, true]]
    );
    s.sort(Some(1), false).unwrap();
    find(&mut s, "hit", None, false, false);
    assert_eq!(
        s.search_highlights(window()).unwrap(),
        [vec![false, true], vec![true, false]]
    );
    s.start_search(SearchOptions {
        term: "hit".into(),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(
        s.search_highlights(window()).unwrap(),
        [vec![false, false], vec![false, false]]
    );
    s.cancel_search();
    assert_eq!(
        s.search_highlights(window()).unwrap(),
        [vec![false, false], vec![false, false]]
    );
    let mut invalid = window();
    invalid.limit = 257;
    assert_eq!(
        s.search_highlights(invalid).err().unwrap().code(),
        "INVALID_WINDOW"
    );
    let mut invalid = window();
    invalid.column_offset = Some(4);
    assert_eq!(
        s.search_highlights(invalid).err().unwrap().code(),
        "INVALID_WINDOW"
    );
}
