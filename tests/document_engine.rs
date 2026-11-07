use nsh_app_rowtation::{
    document::{
        CellValue, MAX_CELLS, MAX_COLUMNS, MAX_INPUT_BYTES, MAX_ROWS, MAX_WINDOW_BYTES,
        MAX_WINDOW_ROWS,
    },
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
fn windows_enforce_positions_row_cell_and_byte_bounds() {
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
    let wide = format!("{}\n", vec!["x"; MAX_COLUMNS].join(",")).repeat(33);
    let document = parse(wide.as_bytes(), ",", false).unwrap();
    assert_eq!(document.window(0, 33).unwrap_err().code(), "WINDOW_LIMIT");
    assert_eq!(document.window(0, 32).unwrap().len(), 32);
    let large = "x".repeat(MAX_WINDOW_BYTES + 1);
    let document = parse(large.as_bytes(), ",", false).unwrap();
    assert_eq!(document.window(0, 1).unwrap_err().code(), "WINDOW_LIMIT");
    assert_eq!(document.cell(1, 1).unwrap(), &CellValue::Text(large));
}

#[test]
fn imports_enforce_limits_without_truncation() {
    assert_eq!(
        parse(&vec![b'x'; MAX_INPUT_BYTES + 1], ",", false)
            .unwrap_err()
            .code(),
        "INPUT_LIMIT"
    );
    let width = vec!["x"; MAX_COLUMNS].join(",");
    assert_eq!(
        parse(width.as_bytes(), ",", false).unwrap().column_count(),
        MAX_COLUMNS
    );
    assert_eq!(
        parse(format!("{width},x").as_bytes(), ",", true)
            .unwrap_err()
            .code(),
        "COLUMN_LIMIT"
    );
    let rows = "x\n".repeat(MAX_ROWS);
    assert_eq!(
        parse(rows.as_bytes(), ",", false).unwrap().row_count(),
        MAX_ROWS
    );
    assert_eq!(
        parse(format!("{rows}x").as_bytes(), ",", false)
            .unwrap_err()
            .code(),
        "DATA_LIMIT"
    );
    let fields = format!("{width}\n").repeat(MAX_CELLS / MAX_COLUMNS + 1);
    assert_eq!(
        parse(fields.as_bytes(), ",", false).unwrap_err().code(),
        "DATA_LIMIT"
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
