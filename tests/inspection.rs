use nsh_app_rowtation::{
    ImportSource,
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
fn profiles_count_whole_values_and_return_bounded_previews() {
    let long = "🦀".repeat(100_000);
    // Differs from long only in its last character, past the preview.
    let near = format!("{}x", "🦀".repeat(99_999));
    let input = format!("value\n{long}\n{near}\n{long}\na\nb\nc\nd\ne\nf\n");
    let mut session = DocumentSession::new(input.into_bytes(), ImportOptions::default()).unwrap();
    let profile = session.profile(session.columns()[0].id).unwrap();
    assert_eq!(profile.unique_count, 8);
    assert_eq!(profile.common_values.len(), 5);
    let top = &profile.common_values[0];
    assert_eq!((top.count, top.byte_length as usize), (2, long.len()));
    assert_eq!(
        (top.value.as_str(), top.truncated),
        ("🦀".repeat(64).as_str(), true)
    );
    assert_eq!(
        (
            profile.common_values[1].value.as_str(),
            profile.common_values[1].truncated
        ),
        ("a", false)
    );
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

#[test]
fn sliced_profiles_match_whole_profiles_and_restart_on_another_column() {
    let input = format!(
        "id,city\n{}",
        (0..1000)
            .map(|i| format!("{i},{}\n", ["Sydney", "Perth", ""][i % 3]))
            .collect::<String>()
    );
    let mut whole =
        DocumentSession::new(input.clone().into_bytes(), ImportOptions::default()).unwrap();
    let mut sliced = DocumentSession::new(input.into_bytes(), ImportOptions::default()).unwrap();
    let (id, city) = (sliced.columns()[0].id, sliced.columns()[1].id);

    let first = sliced.profile_step(city, 400).unwrap();
    assert_eq!((first.scanned, first.total), (400, 1000));
    assert!(first.profile.is_none());
    // Another column discards the unfinished city scan.
    assert_eq!(sliced.profile_step(id, 10).unwrap().scanned, 10);
    assert_eq!(sliced.profile_step(city, 400).unwrap().scanned, 400);
    assert_eq!(sliced.profile_step(city, 400).unwrap().scanned, 800);
    let done = sliced.profile_step(city, 400).unwrap();
    assert_eq!(done.scanned, 1000);
    let profile = done.profile.unwrap();
    let expected = whole.profile(city).unwrap();
    assert_eq!(
        (
            profile.unique_count,
            profile.empty_count,
            profile.missing_count
        ),
        (
            expected.unique_count,
            expected.empty_count,
            expected.missing_count
        )
    );
    assert_eq!((profile.unique_count, profile.empty_count), (3, 333));
    let values: Vec<_> = profile
        .common_values
        .iter()
        .map(|v| (v.value.as_str(), v.count))
        .collect();
    assert_eq!(values, [("Sydney", 334), ("Perth", 333)]);
    // Cached profiles return in one step and survive cancelling another scan.
    assert_eq!(sliced.profile_step(id, 600).unwrap().scanned, 600);
    sliced.cancel_profile();
    let cached = sliced.profile_step(city, 1).unwrap().profile.unwrap();
    assert_eq!(cached.unique_count, 3);
    // A cancelled scan is released: the column starts again from the first row.
    assert_eq!(sliced.profile_step(id, 10).unwrap().scanned, 10);
    sliced.cancel_profile();
    sliced.cancel_profile();
    assert_eq!(sliced.profile_step(id, 10).unwrap().scanned, 10);
    assert_eq!(sliced.profile(id).unwrap().unique_count, 1000);
    assert!(sliced.profile_step(999, 1).is_err());
}

#[test]
fn common_values_rank_by_count_then_text_in_any_table_order() {
    let rank = |values: &[String], slice: u32| {
        let input = format!("value\n{}\n", values.join("\n"));
        let mut session =
            DocumentSession::new(input.into_bytes(), ImportOptions::default()).unwrap();
        let id = session.columns()[0].id;
        let profile = loop {
            if let Some(profile) = session.profile_step(id, slice).unwrap().profile {
                break profile;
            }
        };
        profile
            .common_values
            .into_iter()
            .map(|v| (v.value, v.count))
            .collect::<Vec<_>>()
    };
    let owned = |items: &[&str]| items.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    // Ties order by UTF-8 text; empty text never ranks.
    let tied = owned(&["雪", "b", "", "", "", "🦀", "a", "é", "a", "雪"]);
    assert_eq!(
        rank(&tied, 1),
        [("a", 2), ("雪", 2), ("b", 1), ("é", 1), ("🦀", 1)].map(|(v, c)| (v.to_owned(), c))
    );
    let mut reversed = tied.clone();
    reversed.reverse();
    assert_eq!(rank(&reversed, 3), rank(&tied, u32::MAX));

    // Many distinct values: compare with a full sort.
    let values: Vec<String> = (0..20_000u32)
        .rev()
        .map(|i| format!("v{i:05}"))
        // Previously discarded values must enter the leaders as their counts rise.
        .chain((0..600u32).map(|i| format!("v{:05}", 19_000 + (i.wrapping_mul(7919) % 97))))
        .collect();
    let mut counts = std::collections::HashMap::<&str, u32>::new();
    for value in &values {
        *counts.entry(value).or_default() += 1;
    }
    let mut expected: Vec<(String, u32)> =
        counts.into_iter().map(|(v, c)| (v.to_owned(), c)).collect();
    expected.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    expected.truncate(5);
    for slice in [1, 137, u32::MAX] {
        assert_eq!(rank(&values, slice), expected);
    }
}

#[test]
fn sources_inspect_and_import_without_another_copy_and_survive_failed_imports() {
    let text = "name\tcode\n\"Smith, Mia\"\t007\n";
    let source = ImportSource::from_text(text.into(), "Pasted data".into());
    assert_eq!(source.byte_length(), text.len() as f64);
    let analysis = source.inspect().unwrap();
    let expected = ImportDetector::inspect(text.as_bytes().to_vec(), "Pasted data".into()).unwrap();
    assert_eq!(analysis.delimiter, expected.delimiter);
    assert_eq!(analysis.tab.cells, expected.tab.cells);

    let mut options = ImportOptions::default();
    options.delimiter = Some(";;".into());
    assert!(DocumentSession::from_source(&source, options).is_err());
    let mut options = ImportOptions::default();
    options.delimiter = Some(analysis.delimiter);
    let session = DocumentSession::from_source(&source, options).unwrap();
    let column = session.columns()[1].id;
    assert_eq!(
        session.read_cell(1, column).unwrap().text.as_deref(),
        Some("007")
    );
    let invalid = ImportSource::from_text(String::new(), "empty.tsv".into());
    assert_eq!(invalid.inspect().unwrap().comma.records, 0);
}
