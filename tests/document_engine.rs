use nsh_app_rowtation::{
    document::{CellValue, MAX_WINDOW_ROWS},
    formats::delimited::{parse, serialize},
    session::{DocumentSession, ExportOptions, ImportOptions, RowWindow},
};

#[test]
fn quoted_unicode_and_literal_scalars_survive() {
    let document = parse("\u{feff}id,description,value\r\n00123,\"雪,🙂 and \"\"quotes\"\"\",true\r\n9007199254740993,\"first\r\nsecond\nthird\",null\n".as_bytes(), ",", true).unwrap();
    assert_eq!((document.row_count(), document.column_count()), (2, 3));
    for (row, column, value) in [
        (1, 1, "00123"),
        (1, 2, "雪,🙂 and \"quotes\""),
        (1, 3, "true"),
        (2, 1, "9007199254740993"),
        (2, 2, "first\r\nsecond\nthird"),
        (2, 3, "null"),
    ] {
        assert_eq!(
            document.cell(row, column).unwrap(),
            &CellValue::Text(value.into())
        );
    }
}

#[test]
fn empty_fields_blank_records_and_ragged_rows_are_distinct() {
    let document = parse(b"a,b\n,\n\nx\nx,y,z\n\n", ",", true).unwrap();
    assert_eq!((document.row_count(), document.column_count()), (5, 3));
    assert_eq!(document.cell(1, 2).unwrap(), &CellValue::Text("".into()));
    assert_eq!(document.cell(1, 3).unwrap(), &CellValue::Missing);
    assert_eq!(document.cell(2, 1).unwrap(), &CellValue::Text("".into()));
    assert_eq!(document.cell(2, 2).unwrap(), &CellValue::Missing);
    assert_eq!(document.cell(3, 2).unwrap(), &CellValue::Missing);
    assert_eq!(document.cell(4, 3).unwrap(), &CellValue::Text("z".into()));
    assert_eq!(document.cell(5, 1).unwrap(), &CellValue::Text("".into()));
    assert_eq!(document.columns()[2].name(), "Column 3");
    assert_eq!(document.columns()[2].original_name(), None);
    assert_ne!(CellValue::Missing, CellValue::Null);
    assert_ne!(CellValue::Null, CellValue::Text("".into()));
}

#[test]
fn header_modes_preserve_duplicate_and_empty_names() {
    let document = parse(b"same,,same\n  a  ,b,c\n", ",", true).unwrap();
    assert_eq!(
        document
            .columns()
            .iter()
            .map(|c| c.name())
            .collect::<Vec<_>>(),
        ["same", "", "same"]
    );
    assert_eq!(document.columns()[1].original_name(), Some(""));
    assert_eq!(
        document.cell(1, 1).unwrap(),
        &CellValue::Text("  a  ".into())
    );
    let document = parse(b"00123\t\"a\tb\"\n", "\t", false).unwrap();
    assert_eq!(document.columns()[0].name(), "Column 1");
    assert_eq!(document.columns()[0].original_name(), None);
    assert_eq!(
        document.cell(1, 1).unwrap(),
        &CellValue::Text("00123".into())
    );
    assert_eq!(
        document.cell(1, 2).unwrap(),
        &CellValue::Text("a\tb".into())
    );
}

#[test]
fn empty_bom_only_header_only_and_leading_blank_input() {
    for bytes in [b"".as_slice(), b"\xef\xbb\xbf".as_slice()] {
        let document = parse(bytes, ",", true).unwrap();
        assert_eq!((document.row_count(), document.column_count()), (0, 0));
        assert!(serialize(&document, ",", true).unwrap().is_empty());
    }
    let document = parse(b"a,b", ",", true).unwrap();
    assert_eq!((document.row_count(), document.column_count()), (0, 2));
    assert_eq!(serialize(&document, ",", true).unwrap(), b"a,b\n");
    let document = parse(b"\n\r\na,b\r", ",", false).unwrap();
    assert_eq!(document.row_count(), 3);
    assert_eq!(document.cell(3, 2).unwrap(), &CellValue::Text("b".into()));
    // Only the first BOM is transport metadata; a second belongs to the field.
    let document = parse(b"\xef\xbb\xbf\xef\xbb\xbfx", ",", false).unwrap();
    assert_eq!(
        document.cell(1, 1).unwrap(),
        &CellValue::Text("\u{feff}x".into())
    );
}

#[test]
fn invalid_quotes_have_actionable_locations() {
    for (input, line, record) in [
        ("a,b\n\"unclosed", 2, 2),
        ("a,b\nnot\"quoted,x", 2, 2),
        ("a,b\n\"closed\"oops,x", 2, 2),
        ("a,b\n\"multi\nline\"oops,x", 3, 2),
    ] {
        let error = parse(input.as_bytes(), ",", true).unwrap_err();
        assert_eq!(error.code(), "INVALID_CSV");
        assert_eq!((error.line(), error.record()), (Some(line), Some(record)));
        assert!(
            error
                .to_string()
                .contains(&format!("line {line}, record {record}"))
        );
    }
}

#[test]
fn invalid_encoding_and_delimiters_fail_without_coercion() {
    assert_eq!(
        parse(&[0xff], ",", false).unwrap_err().code(),
        "INVALID_UTF8"
    );
    for separator in ["", "||", "雪", "\"", "\r", "\n", "\0"] {
        assert_eq!(
            parse(b"abc", separator, false).unwrap_err().code(),
            "INVALID_DELIMITER"
        );
        let document = parse(b"abc", ",", false).unwrap();
        assert_eq!(
            serialize(&document, separator, false).unwrap_err().code(),
            "INVALID_DELIMITER"
        );
    }
}

#[test]
fn parse_serialize_round_trips_preserve_values_absences_and_order() {
    for (input, source, headers) in [
        (
            "a,b\r\n00123,\"x,y\"\r\n\"\"\"quoted\"\"\",\"雪\n🙂\"\n\nshort\nlong,row,extra\n",
            ",",
            true,
        ),
        ("a\t\"b\tc\"\n\n\"\"\t\"line\r\nbreak\"\n", "\t", false),
        ("\"\"; x ;\"quote\"\"x\"\n", ";", false),
    ] {
        let original = parse(input.as_bytes(), source, headers).unwrap();
        for target in [",", "\t", ";", "|"] {
            let bytes = serialize(&original, target, headers).unwrap();
            let reparsed = parse(&bytes, target, headers).unwrap();
            assert_eq!(original.row_count(), reparsed.row_count());
            assert_eq!(original.column_count(), reparsed.column_count());
            assert_eq!(
                original
                    .columns()
                    .iter()
                    .map(|c| c.name())
                    .collect::<Vec<_>>(),
                reparsed
                    .columns()
                    .iter()
                    .map(|c| c.name())
                    .collect::<Vec<_>>()
            );
            for row in 1..=original.row_count() as u32 {
                for column in 1..=original.column_count() as u32 {
                    assert_eq!(
                        original.cell(row, column).unwrap(),
                        reparsed.cell(row, column).unwrap()
                    );
                }
            }
        }
    }
}

#[test]
fn serialization_can_change_headers_and_keeps_formula_text_literal() {
    let document = parse(b"key,value\n00123,=1+1\n", ",", true).unwrap();
    assert_eq!(serialize(&document, "\t", false).unwrap(), b"00123\t=1+1\n");
    let document = parse(b"a,b", ",", false).unwrap();
    assert_eq!(
        serialize(&document, ",", true).unwrap(),
        b"Column 1,Column 2\na,b\n"
    );
}

#[test]
fn identities_and_revision_remain_stable_across_overlapping_windows_and_exports() {
    let document = parse(b"same,same\na,b\nc,d\ne,f\n", ",", true).unwrap();
    let first = document.window(0, 2).unwrap();
    let second = document.window(1, 2).unwrap();
    assert_eq!(first[1].id(), second[0].id());
    assert_ne!(first[0].id(), first[1].id());
    assert_ne!(document.columns()[0].id(), document.columns()[1].id());
    let id = document.id().to_owned();
    serialize(&document, "\t", false).unwrap();
    assert_eq!(document.id(), id);
    assert_eq!(document.revision(), 0);
    assert!(!document.dirty());
    assert_eq!(
        document
            .cell(first[1].id(), document.columns()[1].id())
            .unwrap(),
        &CellValue::Text("d".into())
    );
    assert_eq!(document.cell(0, 1).unwrap_err().code(), "UNKNOWN_ROW");
    assert_eq!(
        document.cell(u32::MAX, 1).unwrap_err().code(),
        "UNKNOWN_ROW"
    );
    assert_eq!(document.cell(1, 0).unwrap_err().code(), "UNKNOWN_COLUMN");
    assert_eq!(document.cell(1, 3).unwrap_err().code(), "UNKNOWN_COLUMN");
}

#[test]
fn borrowed_windows_enforce_positions_and_row_bounds() {
    let document = parse(b"a\nb\nc\n", ",", false).unwrap();
    assert_eq!(document.window(1, 256).unwrap().len(), 2);
    assert!(document.window(3, 1).unwrap().is_empty());
    assert!(document.window(0, 0).unwrap().is_empty());
    for (offset, limit) in [(4, 1), (0, MAX_WINDOW_ROWS + 1), (usize::MAX, usize::MAX)] {
        assert_eq!(
            document.window(offset, limit).unwrap_err().code(),
            "INVALID_WINDOW"
        );
    }
}

#[test]
fn values_and_shapes_past_former_caps_round_trip() {
    // Just past former caps: 1 MiB values, 512 columns and 100,000 rows. Inputs of
    // tens of megabytes live in the ignored stress tests.
    let long = format!("0{}", "雪".repeat(350_000));
    let input = format!("value\n{long}\n=1+1\n\"\"\n");
    let document = parse(input.as_bytes(), ",", true).unwrap();
    assert_eq!(document.cell(1, 1).unwrap(), &CellValue::Text(long.clone()));
    assert_eq!(serialize(&document, ",", true).unwrap(), input.into_bytes());

    let width = vec!["x"; 513].join(",");
    assert_eq!(
        parse(width.as_bytes(), ",", false).unwrap().column_count(),
        513
    );
    let rows = "x\n".repeat(100_001);
    assert_eq!(
        parse(rows.as_bytes(), ",", false).unwrap().row_count(),
        100_001
    );
}

#[test]
fn sessions_are_independent_and_failed_imports_preserve_the_live_document() {
    let session = DocumentSession::new(b"key\n00123".to_vec(), ImportOptions::default()).unwrap();
    let id = session.metadata().id;
    let other = DocumentSession::new(b"key\nother".to_vec(), ImportOptions::default()).unwrap();
    assert_ne!(id, other.metadata().id);
    assert!(DocumentSession::new(vec![255], ImportOptions::default()).is_err());
    drop(other);
    assert_eq!(session.metadata().id, id);
    let mut request = RowWindow::default();
    request.limit = 1;
    let window = session.rows(request).unwrap();
    assert_eq!(window.document_id, id);
    assert_eq!(
        session
            .read_cell(window.rows[0].id, session.columns()[0].id)
            .unwrap()
            .text
            .as_deref(),
        Some("00123")
    );
    assert_eq!(
        session.serialize(ExportOptions::default()).unwrap(),
        b"key\n00123\n"
    );
    drop(session);
    let replacement = DocumentSession::new(vec![], ImportOptions::default()).unwrap();
    assert_ne!(replacement.metadata().id, id);
}

#[test]
fn literal_bom_in_first_header_or_value_survives_serialization() {
    for (input, headers) in [
        ("\"\u{feff}name\"\nx\n", true),
        ("\"\u{feff}value\"\n", false),
    ] {
        let document = parse(input.as_bytes(), ",", headers).unwrap();
        let bytes = serialize(&document, ",", headers).unwrap();
        let reparsed = parse(&bytes, ",", headers).unwrap();
        assert_eq!(document.columns()[0].name(), reparsed.columns()[0].name());
        assert_eq!(document.cell(1, 1).unwrap(), reparsed.cell(1, 1).unwrap());
    }
}

#[test]
fn document_ids_are_unique_across_native_test_threads() {
    let first = parse(b"a", ",", false).unwrap();
    let other_id = std::thread::spawn(|| parse(b"b", ",", false).unwrap().id().to_owned())
        .join()
        .unwrap();
    assert_ne!(first.id(), other_id);
}

fn window(offset: u32, limit: u32) -> RowWindow {
    let mut request = RowWindow::default();
    request.offset = offset;
    request.limit = limit;
    request
}

#[test]
fn row_windows_return_whole_values_unless_previews_are_requested() {
    let text = "雪".repeat(100_000);
    let session = DocumentSession::new(
        format!("value\nbefore\n{text}\nafter").into_bytes(),
        ImportOptions::default(),
    )
    .unwrap();
    let whole = session.rows(window(1, 1)).unwrap();
    let cell = &whole.rows[0].cells[0];
    assert_eq!(cell.text.as_deref(), Some(text.as_str()));
    assert_eq!(
        (cell.byte_length as usize, cell.truncated),
        (text.len(), false)
    );

    let mut request = window(1, 1);
    request.preview_bytes = Some(512);
    let preview = session.rows(request).unwrap();
    let cell = &preview.rows[0].cells[0];
    assert_eq!(cell.text.as_deref(), Some("雪".repeat(170).as_str()));
    assert_eq!(
        (cell.byte_length as usize, cell.truncated),
        (text.len(), true)
    );
    let full = session
        .read_cell(preview.rows[0].id, session.columns()[0].id)
        .unwrap();
    assert_eq!(full.text.as_deref(), Some(text.as_str()));
    assert!(!full.truncated);
    assert_eq!(session.metadata().revision, preview.revision);
    assert!(!session.metadata().dirty);
}

#[test]
fn previews_cut_on_character_boundaries_and_keep_short_values_exact() {
    let session = DocumentSession::new(
        "a,b,c,d,e\na雪🙂,,007,=SUM(A1)\n".as_bytes().to_vec(),
        ImportOptions::default(),
    )
    .unwrap();
    let cells = |bytes: u32| {
        let mut request = window(0, 1);
        request.preview_bytes = Some(bytes);
        session.rows(request).unwrap().rows.remove(0).cells
    };
    let texts = |bytes: u32| {
        cells(bytes)
            .into_iter()
            .map(|c| (c.text, c.truncated))
            .collect::<Vec<_>>()
    };
    let some = |text: &str, truncated| (Some(text.to_owned()), truncated);
    // "a雪🙂" is 1 + 3 + 4 bytes; previews never split a character.
    assert_eq!(texts(0)[0], some("", true));
    assert_eq!(texts(3)[0], some("a", true));
    assert_eq!(texts(4)[0], some("a雪", true));
    assert_eq!(texts(7)[0], some("a雪", true));
    assert_eq!(
        texts(8),
        [
            some("a雪🙂", false),
            some("", false),
            some("007", false),
            some("=SUM(A1)", false),
            (None, false),
        ]
    );
    assert_eq!(cells(1)[0].byte_length, 8);
    assert!(matches!(
        cells(1)[4].kind,
        nsh_app_rowtation::session::CellKind::Missing
    ));
}

#[test]
fn column_ranges_copy_only_requested_columns() {
    let mut options = ImportOptions::default();
    options.headers = Some(false);
    let session = DocumentSession::new(b"a,b,c,d,e\nf,g\n".to_vec(), options).unwrap();
    let range = |offset, limit| {
        let mut request = window(0, 2);
        request.column_offset = offset;
        request.column_limit = limit;
        session.rows(request)
    };
    let texts = |snapshot: nsh_app_rowtation::session::RowsSnapshot| {
        snapshot
            .rows
            .into_iter()
            .map(|r| r.cells.into_iter().map(|c| c.text).collect::<Vec<_>>())
            .collect::<Vec<_>>()
    };
    let middle = range(Some(1), Some(2)).unwrap();
    assert_eq!(middle.column_offset, 1);
    assert_eq!(
        texts(middle),
        [
            vec![Some("b".into()), Some("c".into())],
            vec![Some("g".into()), None]
        ]
    );
    assert_eq!(texts(range(Some(3), Some(u32::MAX)).unwrap())[0].len(), 2);
    assert_eq!(texts(range(Some(5), None).unwrap())[0].len(), 0);
    assert_eq!(texts(range(None, Some(0)).unwrap())[1].len(), 0);
    assert_eq!(texts(range(None, None).unwrap())[0].len(), 5);
    assert_eq!(
        range(Some(6), None)
            .err()
            .map(|e| e.code().to_owned())
            .as_deref(),
        Some("INVALID_WINDOW")
    );
    assert!(session.measure_rows(window(0, 2)).unwrap() >= 0.0);
    assert!(session.measure_rows(window(3, 1)).is_err());
}

#[test]
fn snapshots_distinguish_empty_missing_and_text_values() {
    let session = DocumentSession::new(
        b"empty,text,missing\n,abc\n".to_vec(),
        ImportOptions::default(),
    )
    .unwrap();
    let mut request = RowWindow::default();
    request.limit = 1;
    let window = session.rows(request).unwrap();
    let cells = &window.rows[0].cells;
    assert_eq!(
        (cells[0].text.as_deref(), cells[0].byte_length),
        (Some(""), 0)
    );
    assert_eq!(
        (cells[1].text.as_deref(), cells[1].byte_length),
        (Some("abc"), 3)
    );
    assert!(matches!(
        cells[2].kind,
        nsh_app_rowtation::session::CellKind::Missing
    ));
    assert_eq!((cells[2].text.as_deref(), cells[2].byte_length), (None, 0));
}
