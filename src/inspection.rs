//! Detection and profiling stay with the authoritative Rust dataset.
use crate::{error::EngineError, session::saturating_u32};
use nsh_core::prelude::*;

/// Input size above which the import review asks for confirmation.
pub const LARGE_INPUT_BYTES: usize = 8 * 1024 * 1024;
/// Logical record count above which the import review asks for confirmation.
pub const LARGE_RECORDS: usize = 100_000;
/// Record width above which the import review asks for confirmation.
pub const LARGE_COLUMNS: usize = 512;
/// Stored field count above which the import review asks for confirmation.
pub const LARGE_CELLS: usize = 1_000_000;
/// Raw field length above which the import review asks for confirmation.
pub const LARGE_FIELD_BYTES: usize = 1024 * 1024;

/// Size of the input when split with one separator. Advisory only: nothing is
/// rejected, and the strict parser remains the authority on the final shape.
#[derive(JsObject, Default)]
pub struct ImportSize {
    /// Logical records, including a header record and blank records.
    pub records: u32,
    /// Fields in the widest record.
    pub columns: u32,
    /// Fields across all records.
    pub cells: u32,
    /// Raw UTF-8 bytes of the longest field, including any surrounding quotes.
    pub longest_field: u32,
    /// True when any figure, or the input size, exceeds the large-import thresholds.
    pub large: bool,
}

/// Suggested import settings. Header mode is always chosen explicitly by the user.
#[derive(JsObject, Default)]
pub struct ImportAnalysis {
    /// Comma or tab, counting separators outside quoted fields.
    pub delimiter: String,
    /// True when the sample cannot distinguish comma from tab reliably.
    pub ambiguous: bool,
    /// Input length in bytes, including a BOM.
    pub byte_length: u32,
    /// The whole input split by commas.
    pub comma: ImportSize,
    /// The whole input split by tabs.
    pub tab: ImportSize,
}

#[derive(Default)]
struct Tally {
    fields: usize,
    field_start: usize,
    columns: usize,
    cells: usize,
    longest: usize,
}
impl Tally {
    fn end_field(&mut self, end: usize, next: usize) {
        self.longest = self.longest.max(end - self.field_start);
        self.fields += 1;
        self.field_start = next;
    }
    fn end_record(&mut self, end: usize, next: usize) {
        self.end_field(end, next);
        self.columns = self.columns.max(self.fields);
        self.cells += self.fields;
        self.fields = 0;
    }
    fn size(&self, records: usize, bytes: usize) -> ImportSize {
        ImportSize {
            records: saturating_u32(records),
            columns: saturating_u32(self.columns),
            cells: saturating_u32(self.cells),
            longest_field: saturating_u32(self.longest),
            large: bytes > LARGE_INPUT_BYTES
                || records > LARGE_RECORDS
                || self.columns > LARGE_COLUMNS
                || self.cells > LARGE_CELLS
                || self.longest > LARGE_FIELD_BYTES,
        }
    }
}

/// Local CSV/TSV detection; never evaluates imported content.
#[module]
pub struct ImportDetector;

#[module]
impl ImportDetector {
    /// Suggest a separator from the first 32 logical records, then measure the whole
    /// input for both separators. Extension breaks ties only; users can override.
    /// Any size is accepted; check the chosen separator's `large` before importing.
    pub fn inspect(input: Vec<u8>, filename: String) -> Result<ImportAnalysis, EngineError> {
        let text = std::str::from_utf8(&input).map_err(|_| {
            EngineError::new(
                "INVALID_UTF8",
                "Save the file as UTF-8 CSV or TSV and try again.",
            )
        })?;
        let bytes = text.as_bytes();
        let mut counts = [0usize; 2];
        let (mut comma, mut tab) = (Tally::default(), Tally::default());
        let mut quoted = false;
        let mut records = 0;
        let mut i = 0;
        while i < bytes.len() {
            let byte = bytes[i];
            if byte == b'"' {
                if quoted && bytes.get(i + 1) == Some(&b'"') {
                    i += 1;
                } else {
                    quoted = !quoted;
                }
            } else if !quoted {
                match byte {
                    b',' => {
                        comma.end_field(i, i + 1);
                        if records < 32 {
                            counts[0] += 1;
                        }
                    }
                    b'\t' => {
                        tab.end_field(i, i + 1);
                        if records < 32 {
                            counts[1] += 1;
                        }
                    }
                    b'\n' | b'\r' => {
                        let next = if byte == b'\r' && bytes.get(i + 1) == Some(&b'\n') {
                            i + 2
                        } else {
                            i + 1
                        };
                        comma.end_record(i, next);
                        tab.end_record(i, next);
                        records += 1;
                        i = next;
                        continue;
                    }
                    _ => (),
                }
            }
            i += 1;
        }
        if comma.field_start < bytes.len() || comma.fields > 0 {
            comma.end_record(bytes.len(), bytes.len());
            tab.end_record(bytes.len(), bytes.len());
            records += 1;
        }
        let tab_wins = counts[1] > counts[0]
            || (counts[0] == counts[1] && filename.to_ascii_lowercase().ends_with(".tsv"));
        Ok(ImportAnalysis {
            delimiter: if tab_wins { "\t" } else { "," }.into(),
            ambiguous: counts[0] == counts[1] || (counts[0] > 0 && counts[1] > 0),
            byte_length: saturating_u32(input.len()),
            comma: comma.size(records, input.len()),
            tab: tab.size(records, input.len()),
        })
    }
}

/// A frequent text value, with its full text.
#[derive(JsObject, Default, Clone)]
pub struct FrequentValue {
    /// Literal text, never interpreted as markup.
    pub value: String,
    /// Number of occurrences in this column.
    pub count: u32,
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
