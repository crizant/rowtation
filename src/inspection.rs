//! Detection and profiling stay with the authoritative Rust dataset.
use crate::{document::MAX_INPUT_BYTES, error::EngineError};
use nsh_core::prelude::*;

/// Suggested import settings. Header mode is always chosen explicitly by the user.
#[derive(JsObject, Default)]
pub struct ImportAnalysis {
    /// Comma or tab, counting separators outside quoted fields.
    pub delimiter: String,
    /// True when the sample cannot distinguish comma from tab reliably.
    pub ambiguous: bool,
}

/// Local CSV/TSV detection; never evaluates imported content.
#[module]
pub struct ImportDetector;

#[module]
impl ImportDetector {
    /// Inspect at most 32 logical records. Extension breaks ties only; users can override.
    pub fn inspect(input: Vec<u8>, filename: String) -> Result<ImportAnalysis, EngineError> {
        if input.len() > MAX_INPUT_BYTES {
            return Err(EngineError::new(
                "INPUT_LIMIT",
                "Choose a file of 8 MiB or less.",
            ));
        }
        let text = std::str::from_utf8(&input).map_err(|_| {
            EngineError::new(
                "INVALID_UTF8",
                "Save the file as UTF-8 CSV or TSV and try again.",
            )
        })?;
        let mut counts = [0usize; 2];
        let mut quoted = false;
        let mut records = 0;
        let mut bytes = text.bytes().peekable();
        while let Some(byte) = bytes.next() {
            if byte == b'"' {
                if quoted && bytes.peek() == Some(&b'"') {
                    bytes.next();
                } else {
                    quoted = !quoted;
                }
            } else if !quoted {
                match byte {
                    b',' => counts[0] += 1,
                    b'\t' => counts[1] += 1,
                    b'\n' | b'\r' => {
                        if byte == b'\r' && bytes.peek() == Some(&b'\n') {
                            bytes.next();
                        }
                        records += 1;
                        if records == 32 {
                            break;
                        }
                    }
                    _ => (),
                }
            }
        }
        let tab = counts[1] > counts[0]
            || (counts[0] == counts[1] && filename.to_ascii_lowercase().ends_with(".tsv"));
        Ok(ImportAnalysis {
            delimiter: if tab { "\t" } else { "," }.into(),
            ambiguous: counts[0] == counts[1] || (counts[0] > 0 && counts[1] > 0),
        })
    }
}

/// A frequent text value; preview text is bounded to 256 UTF-8 bytes.
#[derive(JsObject, Default, Clone)]
pub struct FrequentValue {
    /// Literal preview text, never interpreted as markup.
    pub value: String,
    /// Number of occurrences in this column.
    pub count: u32,
    /// Whether the value is longer than its preview.
    pub truncated: bool,
}

/// Exact column counts. Empty text and absent fields are reported separately.
#[derive(JsObject, Default, Clone)]
pub struct ColumnProfile {
    /// Document-scoped stable column identity.
    pub column_id: u32,
    /// Stored semantic type; CSV/TSV values remain text.
    pub kind: String,
    /// Distinct present text values, including empty text, excluding missing fields.
    pub unique_count: u32,
    /// Present fields whose text is empty.
    pub empty_count: u32,
    /// Absent trailing fields in short rows.
    pub missing_count: u32,
    /// Up to five most frequent nonempty values, ordered by count then text.
    pub common_values: Vec<FrequentValue>,
}
