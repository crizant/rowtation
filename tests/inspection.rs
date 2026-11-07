use nsh_app_rowtation::{
    inspection::ImportDetector,
    session::{DocumentSession, ImportOptions},
};

#[test]
fn detection_ignores_quoted_separators_and_keeps_ambiguous_options_explicit() {
    let result = ImportDetector::inspect(
        b"name\tcity\n\"Smith, Mia\"\tBrisbane\n".to_vec(),
        "data.csv".into(),
    )
    .unwrap();
    assert_eq!(result.delimiter, "\t");
    assert!(!result.ambiguous);
    let result = ImportDetector::inspect(b"name\nMia".to_vec(), "data.TSV".into()).unwrap();
    assert_eq!(result.delimiter, "\t");
    assert!(result.ambiguous);
    let result = ImportDetector::inspect(b"a,b\nc,d".to_vec(), "data.tsv".into()).unwrap();
    assert_eq!(result.delimiter, ",");
    assert!(ImportDetector::inspect(vec![255], "data.csv".into()).is_err());
}

#[test]
fn detection_measures_the_whole_input_for_both_separators() {
    let result = ImportDetector::inspect(
        b"a,b\tc\r\n\"x\ny,z\",1\n\n\"q\"\"\"\tw".to_vec(),
        "data.csv".into(),
    )
    .unwrap();
    assert_eq!(result.byte_length, 25);
    let (comma, tab) = (&result.comma, &result.tab);
    assert_eq!((comma.records, tab.records), (4, 4));
    assert_eq!((comma.columns, comma.cells), (2, 6));
    assert_eq!((tab.columns, tab.cells), (2, 6));
    assert_eq!(comma.longest_field, 7);
    assert_eq!(tab.longest_field, 9);
    assert!(!comma.large && !tab.large);
    let empty = ImportDetector::inspect(vec![], "data.csv".into()).unwrap();
    assert_eq!((empty.comma.records, empty.comma.cells), (0, 0));
}

#[test]
fn large_inputs_are_accepted_and_flagged_for_confirmation() {
    let flagged = |input: Vec<u8>| {
        let result = ImportDetector::inspect(input, "data.csv".into()).unwrap();
        (result.comma.large, result.tab.large)
    };
    assert_eq!(flagged(b"a\n".repeat(100_000)), (false, false));
    assert_eq!(flagged(b"a\n".repeat(100_001)), (true, true));
    assert_eq!(flagged(vec![b'x'; 8 * 1024 * 1024 + 1]), (true, true));
    assert_eq!(flagged(vec![b','; 512]), (true, false));
    assert_eq!(flagged(vec![b'\t'; 511]), (false, false));
    assert_eq!(
        flagged(format!("{}\n", ",".repeat(99)).repeat(10_001).into_bytes()),
        (true, false)
    );
    assert_eq!(flagged(vec![b'x'; 1024 * 1024 + 1]), (true, true));
}

#[test]
fn profiles_distinguish_empty_missing_and_literal_text_and_are_cached() {
    let mut session = DocumentSession::new(
        b"id,city\n001,Sydney\n002,Brisbane\n003,Sydney\n004,\n005\n006,=1+1\n".to_vec(),
        ImportOptions::default(),
    )
    .unwrap();
    let column = session.columns()[1].id;
    let profile = session.profile(column).unwrap();
    assert_eq!(profile.unique_count, 4);
    assert_eq!(profile.empty_count, 1);
    assert_eq!(profile.missing_count, 1);
    assert_eq!(profile.kind, "Text");
    assert_eq!(profile.common_values[0].value, "Sydney");
    assert_eq!(profile.common_values[0].count, 2);
    assert_eq!(profile.common_values[1].value, "=1+1");
    assert_eq!(session.profile(column).unwrap().common_values[0].count, 2);
    assert!(session.profile(999).is_err());
    assert_eq!(session.metadata().revision, 0);
    assert!(!session.metadata().dirty);
}

#[test]
fn profiles_bound_frequencies_and_keep_whole_values() {
    let long = "🦀".repeat(100_000);
    let input = format!("value\n{long}\n{long}\na\nb\nc\nd\ne\nf\n");
    let mut session = DocumentSession::new(input.into_bytes(), ImportOptions::default()).unwrap();
    let profile = session.profile(session.columns()[0].id).unwrap();
    assert_eq!(profile.common_values.len(), 5);
    assert_eq!(profile.common_values[0].count, 2);
    assert_eq!(profile.common_values[0].value, long);
}

#[test]
fn profile_cache_eviction_keeps_results_correct() {
    let input = "a,b,c,d,e,f,g,h,i\nx,x,x,x,x,x,x,x,x\n";
    let mut session =
        DocumentSession::new(input.as_bytes().to_vec(), ImportOptions::default()).unwrap();
    for column in session.columns() {
        assert_eq!(session.profile(column.id).unwrap().unique_count, 1);
    }
    assert_eq!(
        session
            .profile(session.columns()[0].id)
            .unwrap()
            .common_values[0]
            .value,
        "x"
    );
}
