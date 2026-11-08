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
    assert_eq!((size.records, size.columns, size.cells), (2, 1, 1));
    let source = ImportSource::from_text(
        format!("{{\"a\":\"{}\"}}", "x".repeat(1024 * 1024 + 1)),
        "big.json".into(),
    );
    assert!(source.inspect_format("json".into()).unwrap().large);
}
