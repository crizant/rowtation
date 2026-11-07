//! UTF-8 CSV/TSV adapters, with explicit preservation policies. Size is not
//! capped here; the import review warns about large input before parsing.
use super::records::Records;
use crate::{
    document::{CellValue, Column, Document, Row},
    error::EngineError,
};

pub(crate) fn delimiter(value: &str) -> Result<u8, EngineError> {
    match value.as_bytes() {
        [b] if b.is_ascii() && *b != 0 && !matches!(*b, b'"' | b'\r' | b'\n') => Ok(*b),
        _ => Err(EngineError::new(
            "INVALID_DELIMITER",
            "Use one ASCII character other than NUL, quote, CR or LF.",
        )),
    }
}

/// Parse UTF-8 bytes with an explicit delimiter and header mode. No inference,
/// trimming or scalar coercion. Ragged rows grow the schema and retain absences.
pub fn parse(input: &[u8], separator: &str, headers: bool) -> Result<Document, EngineError> {
    let delimiter = delimiter(separator)?;
    let original = input;
    let input = input.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(input);
    std::str::from_utf8(input).map_err(|e| {
        EngineError::new(
            "INVALID_UTF8",
            format!(
                "Invalid UTF-8 at byte {} after BOM removal.",
                e.valid_up_to()
            ),
        )
    })?;
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .delimiter(delimiter)
        .from_reader(original);
    let mut record = csv::StringRecord::new();
    let mut columns = Vec::new();
    let mut rows = Vec::new();
    for scanned in Records::new(input, delimiter) {
        let (blank, line, number) = scanned?;
        record.clear();
        if blank {
            record.push_field("");
        } else if !reader
            .read_record(&mut record)
            .map_err(|e| EngineError::new("INVALID_CSV", e.to_string()).at(line, number))?
        {
            return Err(
                EngineError::new("INVALID_CSV", "Could not decode the logical record.")
                    .at(line, number),
            );
        }
        if headers && number == 1 {
            columns = record
                .iter()
                .enumerate()
                .map(|(i, name)| Column {
                    id: i as u32 + 1,
                    name: name.into(),
                    original_name: Some(name.into()),
                })
                .collect();
            continue;
        }
        while columns.len() < record.len() {
            let id = columns.len() as u32 + 1;
            columns.push(Column {
                id,
                name: format!("Column {id}"),
                original_name: None,
            });
        }
        rows.push(Row {
            id: rows.len() as u32 + 1,
            cells: record.iter().map(|s| CellValue::Text(s.into())).collect(),
        });
    }
    Document::new(columns, rows, delimiter, headers)
}

/// Serialize text with standard quoting and LF records. Trailing Missing fields
/// remain absent. Interior Missing and non-text types fail rather than lose meaning.
/// Imported formula-like text is emitted verbatim and is never evaluated.
pub fn serialize(
    document: &Document,
    separator: &str,
    headers: bool,
) -> Result<Vec<u8>, EngineError> {
    let delimiter = delimiter(separator)?;
    // A literal leading BOM must stay inside quotes; otherwise a later UTF-8
    // reader would consume the field's first character as transport metadata.
    let first_text = if headers && !document.columns.is_empty() {
        Some(document.columns[0].name.as_str())
    } else {
        document
            .rows
            .first()
            .and_then(|row| match row.cells.first() {
                Some(CellValue::Text(text)) => Some(text.as_str()),
                _ => None,
            })
    };
    let quote_style = if first_text.is_some_and(|text| text.starts_with('\u{feff}')) {
        csv::QuoteStyle::Always
    } else {
        csv::QuoteStyle::Necessary
    };
    let mut writer = csv::WriterBuilder::new()
        .delimiter(delimiter)
        .quote_style(quote_style)
        .flexible(true)
        .from_writer(Vec::new());
    let write_error = |e: csv::Error| EngineError::new("SERIALIZE_FAILED", e.to_string());
    if headers && !document.columns.is_empty() {
        writer
            .write_record(document.columns.iter().map(|c| c.name.as_str()))
            .map_err(write_error)?;
    }
    for row in &document.rows {
        let length = row
            .cells
            .iter()
            .rposition(|c| !matches!(c, CellValue::Missing))
            .map_or(0, |i| i + 1);
        let fields: Result<Vec<&str>, EngineError> = row.cells[..length].iter().map(|c| match c {
            CellValue::Text(text) => Ok(text.as_str()),
            _ => Err(EngineError::new("LOSSY_EXPORT", "Delimited text cannot preserve interior missing, null or typed values without a conversion policy.")),
        }).collect();
        writer.write_record(fields?).map_err(write_error)?;
    }
    writer
        .into_inner()
        .map_err(|e| EngineError::new("SERIALIZE_FAILED", e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_or_interior_missing_values_require_an_explicit_loss_policy() {
        for value in [
            CellValue::Missing,
            CellValue::Null,
            CellValue::Boolean(true),
            CellValue::Number("9007199254740993".into()),
        ] {
            let mut document = parse(b"a,b\nx,y", ",", true).unwrap();
            document.rows[0].cells[0] = value;
            assert_eq!(
                serialize(&document, ",", true).unwrap_err().code(),
                "LOSSY_EXPORT"
            );
        }
        let mut document = parse(b"a,b\nx,y", ",", true).unwrap();
        document.rows[0].cells[1] = CellValue::Missing;
        assert_eq!(serialize(&document, ",", true).unwrap(), b"a,b\nx\n");
    }
}
