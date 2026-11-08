use nsh_app_rowtation::{
    DocumentSession,
    operations::SearchOptions,
    session::{ExportOptions, ImportOptions},
};
fn session(text: &str, format: &str) -> DocumentSession {
    let mut o = ImportOptions::default();
    o.format = Some(format.into());
    DocumentSession::new(text.as_bytes().to_vec(), o).unwrap()
}
fn options(format: &str) -> ExportOptions {
    let mut o = ExportOptions::default();
    o.format = Some(format.into());
    o.allow_lossy = Some(true);
    o
}
fn output(s: &DocumentSession, format: &str) -> String {
    s.serialize_text(options(format)).unwrap()
}
#[test]
fn csv_edit_json_and_replace_tsv_roundtrip() {
    let mut s = session("id,city\n001,Brisbane\n002,Sydney", "csv");
    s.edit_cell(1, 2, "Perth".into(), 0).unwrap();
    assert_eq!(
        output(&s, "json"),
        r#"[{"id":"001","city":"Perth"},{"id":"002","city":"Sydney"}]"#
    );
    let mut query = SearchOptions::default();
    query.term = "Sydney".into();
    s.search(query).unwrap();
    s.replace_matches(None, "Hobart".into(), 1).unwrap();
    let tsv = output(&s, "tsv");
    let imported = session(&tsv, "tsv");
    assert_eq!(
        imported.read_cell(2, 2).unwrap().text.as_deref(),
        Some("Hobart")
    );
    assert_eq!(s.metadata().revision, 2);
    assert!(s.metadata().dirty);
    let csv = output(&s, "csv");
    assert_eq!(output(&session(&csv, "csv"), "json"), output(&s, "json"));
}
#[test]
fn exact_nested_json_types_and_undo() {
    let input = r#"[{"items":[{"sku":"001"},{"sku":"002"}],"n":900719925474099312345,"d":1.2300,"b":true,"z":null,"empty":{},"a":[]},{"items":[]}]"#;
    let mut s = session(input, "json");
    s.edit_cell(1, 1, "003".into(), 0).unwrap();
    let edited = output(&s, "json");
    assert!(edited.contains(r#""items":[{"sku":"003"},{"sku":"002"}]"#));
    assert!(edited.contains("900719925474099312345"));
    assert!(edited.contains("1.2300"));
    s.history(false).unwrap();
    assert_eq!(output(&s, "json"), input);
    let before = s.metadata();
    let mut pretty = options("json");
    pretty.pretty = Some(true);
    let pretty = s.serialize_text(pretty).unwrap();
    assert!(pretty.contains("1.2300"));
    assert!(pretty.contains("900719925474099312345"));
    assert_eq!(output(&session(&pretty, "json"), "json"), input);
    assert_eq!(s.metadata().revision, before.revision);
    assert!(!s.metadata().dirty);
    s.history(true).unwrap();
    assert_eq!(output(&s, "json"), edited);
}
#[test]
fn ndjson_and_missing_null_empty() {
    let s = session(r#"[{"a":null,"b":"","n":12},{"b":"001"}]"#, "json");
    let lines = output(&s, "ndjson");
    assert_eq!(lines.lines().count(), 2);
    let nd = session(&lines, "ndjson");
    assert_eq!(output(&nd, "json"), output(&s, "json"));
    let mut strict = options("csv");
    strict.allow_lossy = None;
    assert_eq!(nd.serialize(strict).unwrap_err().code(), "LOSSY_EXPORT");
    let csv = output(&nd, "csv");
    let re = session(&csv, "csv");
    assert_eq!(re.read_cell(1, 1).unwrap().text.as_deref(), Some(""));
    assert_eq!(re.read_cell(2, 2).unwrap().text.as_deref(), Some("001"));
}
#[test]
fn ambiguous_insert_blocks_and_flat_is_explicit() {
    let mut s = session(r#"[{"a":{"b":1}},{"a":2}]"#, "json");
    s.edit_cell(2, 1, "new".into(), 0).unwrap();
    let before = s.metadata();
    assert_eq!(
        s.serialize(options("json")).unwrap_err().code(),
        "LOSSY_EXPORT"
    );
    let mut flat = options("json");
    flat.reconstruct = Some(false);
    assert!(!s.export_warning(options("csv")).unwrap().is_empty());
    assert!(s.serialize_text(flat).unwrap().contains(r#""a.b":"new""#));
    assert_eq!(s.metadata().revision, before.revision);
    s.history(false).unwrap();
    assert_eq!(output(&s, "json"), r#"[{"a":{"b":1}},{"a":2}]"#);
}
#[test]
fn sort_original_order_and_search_does_not_filter() {
    let mut s = session("a\nz\na", "csv");
    s.sort(Some(1), false).unwrap();
    let mut query = SearchOptions::default();
    query.term = "z".into();
    s.search(query).unwrap();
    assert_eq!(output(&s, "json"), r#"[{"a":"z"},{"a":"a"}]"#);
    let mut o = options("json");
    o.sorted = Some(true);
    assert_eq!(s.serialize_text(o).unwrap(), r#"[{"a":"a"},{"a":"z"}]"#);
    assert!(!s.metadata().dirty);
}
#[test]
fn csv_quoting_unicode_and_formula_protection() {
    let s = session("\"=header\",b\n\" =1+1\",\"Hana,\"\"雪\"\"\nline\"", "csv");
    let mut o = options("csv");
    o.safe_text = Some(true);
    let protected = s.serialize_text(o).unwrap();
    let parsed = session(&protected, "csv");
    assert_eq!(parsed.columns()[0].name, "'=header");
    assert_eq!(
        parsed.read_cell(1, 1).unwrap().text.as_deref(),
        Some("' =1+1")
    );
    assert_eq!(
        parsed.read_cell(1, 2).unwrap().text.as_deref(),
        Some("Hana,\"雪\"\nline")
    );
    assert_eq!(s.read_cell(1, 1).unwrap().text.as_deref(), Some(" =1+1"));
}
#[test]
fn xml_reconstruction_attributes_repeats_namespaces_and_json_mapping() {
    let input = r#"<r:rows xmlns:r="urn:rows"><r:row id="001" xml:lang="en"><tag>A</tag><tag>B</tag><n:value xmlns:n="urn:n" n:code="x">&lt;&amp;</n:value></r:row></r:rows>"#;
    let mut s = session(input, "xml");
    s.edit_cell(1, 3, "C<&\r".into(), 0).unwrap();
    let xml = output(&s, "xml");
    let round = session(&xml, "xml");
    assert_eq!(output(&round, "json"), output(&s, "json"));
    let json = output(&s, "json");
    assert!(json.contains(r#""@id":"001""#));
    assert!(json.contains(r##""tag":[{"#text":"C<&\r"},{"#text":"B"}]"##));
    let mut pretty = options("xml");
    pretty.pretty = Some(true);
    assert!(s.serialize_text(pretty).unwrap().contains('\n'));
}
#[test]
fn json_xml_explicit_policy_and_rejections() {
    let s = session(
        r#"[{"n":900719925474099312345,"items":[{"sku":"001"}],"b":true}]"#,
        "json",
    );
    let xml = output(&s, "xml");
    assert!(xml.contains("900719925474099312345"));
    assert!(xml.contains("<items><item><sku>001</sku></item></items>"));
    session(&xml, "xml");
    for input in [
        r#"[{"x":null}]"#,
        r#"[{"a.b?":"x"}]"#,
        r#"[{"x":[]}]"#,
        r#"[{"x":"\u0000"}]"#,
    ] {
        let s = session(input, "json");
        assert!(s.serialize(options("xml")).is_err());
        assert!(!s.metadata().dirty);
    }
    let duplicate = session("a,a\nx,y", "csv");
    assert!(duplicate.serialize(options("json")).is_err());
}
#[test]
fn filenames_and_failures_preserve_document() {
    use nsh_app_rowtation::browser_export::filename;
    assert_eq!(filename("orders.csv", "json").unwrap(), "orders.json");
    assert_eq!(
        filename("../bad/name.jsonl", "ndjson").unwrap(),
        "_bad_name.ndjson"
    );
    assert_eq!(filename("", "xml").unwrap(), "export.xml");
    assert!(filename("a", "xlsx").is_err());
    let mut s = session("a\nx", "csv");
    s.edit_cell(1, 1, "y".into(), 0).unwrap();
    assert!(s.download("orders.csv".into(), options("json")).is_err());
    assert_eq!(s.metadata().revision, 1);
    assert!(s.metadata().dirty);
    assert_eq!(output(&s, "json"), r#"[{"a":"y"}]"#);
}

#[test]
fn editing_ragged_rows_requires_missing_value_policy() {
    let mut s = session("a,b,c\nx,y,z\nq", "csv");
    s.edit_cell(2, 3, "r".into(), 0).unwrap();
    let mut o = options("csv");
    o.allow_lossy = None;
    assert!(
        s.export_warning(options("csv"))
            .unwrap()
            .contains("Interior missing")
    );
    assert_eq!(s.serialize(o).unwrap_err().code(), "LOSSY_EXPORT");
    assert_eq!(
        session(&output(&s, "csv"), "csv")
            .read_cell(2, 2)
            .unwrap()
            .text
            .as_deref(),
        Some("")
    );
}

#[test]
fn absent_json_paths_reconstruct_without_collisions_or_array_holes() {
    let mut s = session(
        r#"[{"user":{"name":"Mia"},"items":[{"sku":"001"},{"sku":"002"}]},{}]"#,
        "json",
    );
    s.edit_cell(2, 1, "Hana".into(), 0).unwrap();
    assert!(output(&s, "json").ends_with(r#"{"user":{"name":"Hana"}}]"#));
    s.edit_cell(2, 3, "gap".into(), 1).unwrap();
    assert!(
        s.serialize(options("json"))
            .unwrap_err()
            .message()
            .contains("missing array")
    );
    s.edit_cell(2, 2, "003".into(), 2).unwrap();
    let revision = s.metadata().revision;
    assert!(
        output(&s, "json")
            .ends_with(r#"{"user":{"name":"Hana"},"items":[{"sku":"003"},{"sku":"gap"}]}]"#)
    );
    assert_eq!(s.metadata().revision, revision);
    s.history(false).unwrap();
    assert!(s.serialize(options("json")).is_err());
}
