use nsh_app_rowtation::{
    ImportSource,
    document::CellValue,
    formats::{
        delimited,
        structured::{self, Shape},
    },
    inspection::ImportDetector,
    session::{DocumentSession, ImportOptions},
};

fn session(input: &str, format: &str) -> DocumentSession {
    let mut options = ImportOptions::default();
    options.format = Some(format.into());
    DocumentSession::new(input.as_bytes().to_vec(), options).unwrap()
}
#[test]
fn nested_mapping_precision_and_shapes() {
    let input = r#"[{"user":{"name":"00123","address":{"city":"Brisbane"}},"items":[{"sku":900719925474099312345},{"sku":1.2300e+002}],"empty":{},"list":[]},{"user":null,"items":[],"flag":true}]"#;
    let doc = structured::parse(input.as_bytes(), "json").unwrap();
    let names: Vec<_> = doc.columns().iter().map(|c| c.name()).collect();
    assert_eq!(
        names,
        [
            "user.name",
            "user.address.city",
            "items[0].sku",
            "items[1].sku",
            "user",
            "flag"
        ]
    );
    assert_eq!(doc.cell(1, 1).unwrap(), &CellValue::Text("00123".into()));
    assert_eq!(
        doc.cell(1, 3).unwrap(),
        &CellValue::Number("900719925474099312345".into())
    );
    assert_eq!(
        doc.cell(1, 4).unwrap(),
        &CellValue::Number("1.2300e+002".into())
    );
    assert_eq!(doc.cell(2, 1).unwrap(), &CellValue::Missing);
    assert_eq!(doc.cell(2, 5).unwrap(), &CellValue::Null);
    let structure = doc.structure().unwrap();
    assert_eq!(structure.envelope, "array");
    let Shape::Object(fields) = &structure.rows[0] else {
        panic!()
    };
    assert_eq!(fields[2], ("empty".into(), Shape::Object(vec![])));
    assert_eq!(fields[3], ("list".into(), Shape::Array(vec![])));
    assert_eq!(doc.source_format(), "json");
    assert!(!doc.dirty());
    assert_eq!(doc.revision(), 0);
    assert_eq!(structured::parse(b"[]", "json").unwrap().row_count(), 0);
    assert_eq!(structured::parse(b"{}", "json").unwrap().row_count(), 1);
    // Literal key syntax cannot alias nested paths.
    let doc = structured::parse(
        br#"{"a.b":1,"a":{"b":2},"":[],"zero":-0,"decimal":0.0001000}"#,
        "json",
    )
    .unwrap();
    assert_eq!(doc.columns()[0].name(), "[\"a.b\"]");
    assert_eq!(doc.cell(1, 3).unwrap(), &CellValue::Number("-0".into()));
    assert_eq!(
        doc.cell(1, 4).unwrap(),
        &CellValue::Number("0.0001000".into())
    );
}
#[test]
fn ndjson_lines_blank_policy_and_failures() {
    let doc = structured::parse(b"\r\n{\"a\":null}\r\n  \r\n{\"b\":\"\"}\n", "ndjson").unwrap();
    assert_eq!(doc.row_count(), 2);
    assert_eq!(doc.cell(1, 1).unwrap(), &CellValue::Null);
    assert_eq!(doc.cell(2, 1).unwrap(), &CellValue::Missing);
    assert_eq!(doc.cell(2, 2).unwrap(), &CellValue::Text("".into()));
    let err = structured::parse(b"{}\n\n{broken}\n{}", "ndjson").unwrap_err();
    assert_eq!(err.line(), Some(3));
    assert_eq!(err.record(), Some(2));
    assert!(
        structured::parse(b"{}\n[]", "ndjson")
            .unwrap_err()
            .to_string()
            .contains("line 2")
    );
}
#[test]
fn xml_mapping_and_safety() {
    let doc = structured::parse(br#"<rows xmlns:p="urn:test"><row id="001"><p:name>A&amp;B</p:name><tag>x</tag><tag/><empty/></row><row id="002"><p:name>C</p:name></row></rows>"#, "xml").unwrap();
    assert_eq!(doc.row_count(), 2);
    assert_eq!(
        doc.columns().iter().map(|c| c.name()).collect::<Vec<_>>(),
        [
            "@id",
            "[\"{urn:test}name\"]#text",
            "tag[0]#text",
            "tag[1]#text",
            "empty#text"
        ]
    );
    assert_eq!(doc.cell(1, 2).unwrap(), &CellValue::Text("A&B".into()));
    assert_eq!(doc.cell(1, 4).unwrap(), &CellValue::Text("".into()));
    assert_eq!(doc.cell(2, 4).unwrap(), &CellValue::Missing);
    let Shape::Xml {
        name,
        children,
        namespaces,
        ..
    } = &doc.structure().unwrap().rows[0]
    else {
        panic!()
    };
    assert_eq!(name, "row");
    assert_eq!(children.len(), 4);
    assert!(!namespaces.is_empty());
    assert!(doc.structure().unwrap().xml_root.is_some());
    for xml in [
        "<!DOCTYPE rows [<!ENTITY x SYSTEM 'file:///etc/passwd'>]><rows><row>&x;</row></rows>",
        "<!DOCTYPE rows><rows/>",
        "<rows><row>a<b/>c</row></rows>",
        "<rows><!--comment--><row/></rows>",
        "<rows><?foo bar?><row/></rows>",
        "<rows><a/><b/></rows>",
        "<rows id='x'/>",
        "<rows><row></rows>",
        "<rows><row>&unknown;</row></rows>",
    ] {
        assert!(structured::parse(xml.as_bytes(), "xml").is_err(), "{xml}");
    }
    assert!(
        structured::parse(
            "<rows><row>雪<![CDATA[<b>]]></row></rows>".as_bytes(),
            "xml"
        )
        .is_ok()
    );
}
#[test]
fn malformed_and_depth_limits() {
    for input in [
        "[1]",
        "null",
        "42",
        "{\"a\":1,\"a\":2}",
        "{\"nested\":{\"a\":1,\"a\":2}}",
        "{",
        "{} garbage",
        "{\"a\":NaN}",
    ] {
        assert!(
            structured::parse(input.as_bytes(), "json").is_err(),
            "{input}"
        );
    }
    assert!(structured::parse(&[255], "json").is_err());
    assert!(
        structured::parse(
            br#"<?xml version="1.0" encoding="ISO-8859-1"?><rows/>"#,
            "xml"
        )
        .is_err()
    );
    assert!(structured::parse(br#"<?xml version="1.0" encoding="UTF-8"?><rows/>"#, "xml").is_ok());
    let deep = format!("{{\"a\":{}0{}}}", "[".repeat(70), "]".repeat(70));
    assert!(
        structured::parse(deep.as_bytes(), "json")
            .unwrap_err()
            .to_string()
            .contains("64")
    );
    let deep = format!("<rows>{}{}</rows>", "<row>".repeat(70), "</row>".repeat(70));
    assert!(structured::parse(deep.as_bytes(), "xml").is_err());
    let err = structured::parse(b"[\n{broken}\n]", "json").unwrap_err();
    assert_eq!(err.line(), Some(2));
}
#[test]
fn operations_preserve_identities_types_history_and_profile() {
    let mut s = session(
        r#"[{"a":123,"b":true,"v":null},{"a":"123","b":false,"v":""},{"a":2},{"a":123}]"#,
        "json",
    );
    let id = s.metadata().id;
    assert_eq!(s.metadata().format, "json");
    let p = s.profile(1).unwrap();
    assert_eq!(p.kind, "Mixed");
    assert_eq!(p.unique_count, 3);
    assert_eq!(p.missing_count, 0);
    let p = s.profile(3).unwrap();
    assert_eq!((p.null_count, p.empty_count, p.missing_count), (1, 1, 2));
    s.edit_cell(1, 1, "123".into(), 0).unwrap();
    assert_eq!(s.metadata().revision, 0);
    s.edit_cell(1, 2, "true".into(), 0).unwrap();
    assert_eq!(s.metadata().revision, 0);
    s.edit_cell(1, 1, "124".into(), 0).unwrap();
    assert_eq!(s.read_cell(1, 1).unwrap().text.as_deref(), Some("124"));
    assert!(s.metadata().dirty);
    assert_eq!(s.metadata().id, id);
    s.history(false).unwrap();
    assert_eq!(s.read_cell(1, 1).unwrap().number.as_deref(), Some("123"));
    assert!(!s.metadata().dirty);
    s.history(true).unwrap();
    assert_eq!(s.read_cell(1, 1).unwrap().text.as_deref(), Some("124"));
    let mut options = nsh_app_rowtation::operations::SearchOptions::default();
    options.term = "true".into();
    assert_eq!(s.search(options).unwrap().count, 1);
    let rev = s.metadata().revision;
    s.replace_matches(None, "true".into(), rev).unwrap();
    assert_eq!(s.metadata().revision, rev);
    s.replace_matches(None, "yes".into(), rev).unwrap();
    assert_eq!(s.read_cell(1, 2).unwrap().text.as_deref(), Some("yes"));
    s.history(false).unwrap();
    assert_eq!(s.read_cell(1, 2).unwrap().boolean, Some(true));
    s.sort(Some(1), false).unwrap();
    assert_eq!(s.row_position(3).unwrap(), 0);
    // Both old delimited adapters keep their existing all-text and round-trip policy.
    for delimiter in [",", "\t"] {
        let text = format!("a{delimiter}b\n001{delimiter}true\n");
        let d = delimited::parse(text.as_bytes(), delimiter, true).unwrap();
        assert_eq!(
            delimited::serialize(&d, delimiter, true).unwrap(),
            text.as_bytes()
        );
    }
}
#[test]
fn detection_and_review_use_content_and_hints() {
    for (text, name, format) in [
        ("{\"a\":1}", "x.csv", "json"),
        ("{}\n{}", "paste", "ndjson"),
        ("[{}]", "paste", "json"),
        ("<rows/>", "x.json", "xml"),
        ("{}", "x.jsonl", "ndjson"),
        ("broken", "x.xml", "xml"),
    ] {
        assert_eq!(
            ImportDetector::inspect(text.as_bytes().to_vec(), name.into())
                .unwrap()
                .format,
            format
        );
    }
    let source = ImportSource::from_text(r#"[{"a":{"b":1}},{}]"#.into(), "data.json".into());
    let size = source.inspect_format("json".into()).unwrap();
    assert!(!size.dimensions_known);
    let mut options = ImportOptions::default();
    options.format = Some("json".into());
    let session = DocumentSession::from_source(&source, options).unwrap();
    let exact = session.import_size(source.byte_length() as u32);
    assert!(exact.dimensions_known);
    assert_eq!((exact.records, exact.columns, exact.cells), (2, 1, 1));
    let source = ImportSource::from_text(
        format!("{{\"a\":\"{}\"}}", "x".repeat(1024 * 1024 + 1)),
        "big.json".into(),
    );
    assert!(!source.inspect_format("json".into()).unwrap().large);
    let mut options = ImportOptions::default();
    options.format = Some("json".into());
    let session = DocumentSession::from_source(&source, options).unwrap();
    assert!(session.import_size(source.byte_length() as u32).large);
}

#[test]
fn nested_json_errors_use_document_lines_including_unicode_and_crlf() {
    for newline in ["\n", "\r\n"] {
        let input = [
            "[",
            "  {\"雪\": \"x\"},",
            "  {",
            "    \"nested\": {",
            "      \"keys\": [",
            "        {\"dup\": 1,",
            "         \"dup\": 2}",
            "      ]",
            "    }",
            "  }",
            "]",
        ]
        .join(newline);
        let e = structured::parse(input.as_bytes(), "json").unwrap_err();
        assert_eq!(e.line(), Some(7));
        assert_eq!(e.record(), Some(2));
        assert!(e.message().contains("at line 7 column"), "{e}");
        let malformed = input.replace("\"dup\": 2", "\"dup\": !");
        assert_eq!(
            structured::parse(malformed.as_bytes(), "json")
                .unwrap_err()
                .line(),
            Some(7)
        );
    }
}

#[test]
fn heterogeneous_rows_allocate_only_present_cells_and_missing_edits_stay_sparse() {
    use nsh_app_rowtation::document::RowCells;
    for format in ["json", "ndjson"] {
        let records = (0..10_000)
            .map(|i| format!("{{\"field_{i}\":{i}}}"))
            .collect::<Vec<_>>();
        let input = if format == "json" {
            format!("[{}]", records.join(","))
        } else {
            records.join("\n")
        };
        let doc = structured::parse(input.as_bytes(), format).unwrap();
        assert_eq!(doc.column_count(), 10_000);
        assert_eq!(
            doc.window(0, 256)
                .unwrap()
                .iter()
                .map(|r| r.cells().stored_len())
                .sum::<usize>(),
            256
        );
        // Check the end too: the old dense implementation allocated 10,000 cells
        // in this one-field row, and ~50 million Missing cells across the file.
        assert_eq!(doc.window(9999, 1).unwrap()[0].cells().stored_len(), 1);
        assert!(matches!(
            doc.window(9999, 1).unwrap()[0].cells(),
            RowCells::Sparse(_)
        ));
        assert_eq!(
            doc.cell(10_000, 10_000).unwrap(),
            &CellValue::Number("9999".into())
        );
        assert_eq!(doc.cell(10_000, 1).unwrap(), &CellValue::Missing);
        let mut s = session(&input, format);
        s.edit_cell(1, 10_000, "new".into(), 0).unwrap();
        assert_eq!(s.read_cell(1, 10_000).unwrap().text.as_deref(), Some("new"));
        s.history(false).unwrap();
        assert!(matches!(
            s.read_cell(1, 10_000).unwrap().kind,
            nsh_app_rowtation::session::CellKind::Missing
        ));
        assert!(!s.metadata().dirty);
        s.history(true).unwrap();
        assert!(s.metadata().dirty);
        let profile = s.profile(10_000).unwrap();
        assert_eq!((profile.unique_count, profile.missing_count), (2, 9998));
    }
    let doc = delimited::parse(b"a,b\n1,2", ",", true).unwrap();
    assert!(matches!(
        doc.window(0, 1).unwrap()[0].cells(),
        RowCells::Dense(_)
    ));
}

#[test]
fn structured_review_neither_validates_nor_parses_full_input() {
    // An invalid tail beyond the 64-KiB detection sample must not be visited
    // until Import. Review still exposes byte-size confirmation first.
    let input = format!("[{{\"x\":\"{}\"}}]BROKEN", "x".repeat(9 * 1024 * 1024));
    let source = ImportSource::from_text(input, "large.json".into());
    let analysis = source.inspect().unwrap();
    assert_eq!(analysis.format, "json");
    assert!(analysis.comma.large);
    assert!(!analysis.comma.dimensions_known);
    assert!(source.inspect_format("json".into()).unwrap().large);
    let mut options = ImportOptions::default();
    options.format = Some("json".into());
    assert!(DocumentSession::from_source(&source, options).is_err());
}

#[test]
fn numeric_sort_keys_preserve_equal_values_and_rebuild_after_edit_and_history() {
    let mut s = session(
        r#"[{"n":9007199254740993},{"n":9007199254740992},{"n":1.000},{"n":1e0},{"n":-0},{"n":0.0},{"n":-1.234567890123456789012345678901}]"#,
        "json",
    );
    let rows = |s: &DocumentSession| {
        let mut w = nsh_app_rowtation::session::RowWindow::default();
        w.limit = 7;
        s.rows(w)
            .unwrap()
            .rows
            .into_iter()
            .map(|r| r.id)
            .collect::<Vec<_>>()
    };
    s.sort(Some(1), false).unwrap();
    assert_eq!(rows(&s), [7, 5, 6, 3, 4, 2, 1]);
    s.sort(Some(1), true).unwrap();
    assert_eq!(rows(&s), [1, 2, 3, 4, 5, 6, 7]);
    s.edit_cell(7, 1, "text".into(), 0).unwrap();
    assert_eq!(rows(&s)[0], 7);
    s.history(false).unwrap();
    assert_eq!(rows(&s), [1, 2, 3, 4, 5, 6, 7]);
    s.history(true).unwrap();
    assert_eq!(rows(&s)[0], 7);
    s.history(false).unwrap();
    assert!(!s.metadata().dirty);
    assert_eq!(s.read_cell(3, 1).unwrap().number.as_deref(), Some("1.000"));
}
