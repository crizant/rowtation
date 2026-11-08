//! Detection and profiling stay with the authoritative Rust dataset.
use crate::{
    document::{CellValue, Document},
    error::EngineError,
    session::{prefix, saturating_u32},
};
use hashbrown::{HashTable, hash_table::Entry};
use nsh_core::prelude::*;
use rustc_hash::FxBuildHasher;
use std::hash::BuildHasher;

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
    /// Suggested source format, overridable in review.
    pub format: String,
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

/// Local format and CSV/TSV separator detection; never evaluates imported content.
#[module]
pub struct ImportDetector;

#[module]
impl ImportDetector {
    /// Suggest a separator from the first 32 logical records, then measure the whole
    /// input for both separators. Extension breaks ties only; users can override.
    /// Any size is accepted; check the chosen separator's `large` before importing.
    pub fn inspect(input: Vec<u8>, filename: String) -> Result<ImportAnalysis, EngineError> {
        analyze(&input, &filename)
    }
}

/// The scan behind `ImportDetector.inspect` and `ImportSource.inspect`.
pub(crate) fn analyze(input: &[u8], filename: &str) -> Result<ImportAnalysis, EngineError> {
    let text = std::str::from_utf8(input)
        .map_err(|_| EngineError::new("INVALID_UTF8", "Save the file as UTF-8 and try again."))?;
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
        format: detect_format(text, filename, tab_wins).into(),
        delimiter: if tab_wins { "\t" } else { "," }.into(),
        ambiguous: counts[0] == counts[1] || (counts[0] > 0 && counts[1] > 0),
        byte_length: saturating_u32(input.len()),
        comma: comma.size(records, input.len()),
        tab: tab.size(records, input.len()),
    })
}

/// Most frequent nonempty values reported per column.
const COMMON_VALUES: usize = 5;
/// Bytes of each frequent value copied into a profile, cut on a character boundary.
pub const FREQUENT_VALUE_PREVIEW_BYTES: usize = 256;

/// A frequent text value. Long values are previews; counts are always exact.
#[derive(JsObject, Default, Clone)]
pub struct FrequentValue {
    /// Literal text, never interpreted as markup; at most 256 UTF-8 bytes.
    pub value: String,
    /// UTF-8 length of the whole value.
    pub byte_length: u32,
    /// True when value holds only a prefix.
    pub truncated: bool,
    /// Number of occurrences in this column.
    pub count: u32,
}

/// Exact column counts. Empty text and absent fields are reported separately.
#[derive(JsObject, Default, Clone)]
pub struct ColumnProfile {
    /// Document-scoped stable column identity.
    pub column_id: u32,
    /// Canonical type name, or Mixed; CSV/TSV and XML remain text.
    pub kind: String,
    /// Distinct canonical values, including Null and empty Text, excluding Missing.
    pub unique_count: u32,
    /// Present fields whose text is empty.
    pub empty_count: u32,
    /// Explicit null values.
    pub null_count: u32,
    /// Absent fields, including interior structured properties.
    pub missing_count: u32,
    /// Up to five most frequent nonempty values, ordered by count then text.
    pub common_values: Vec<FrequentValue>,
}

/// Progress of an incremental column profile.
#[derive(JsObject, Default)]
pub struct ProfileProgress {
    /// Document-scoped stable column identity.
    pub column_id: u32,
    /// Rows counted so far.
    pub scanned: u32,
    /// Rows in the document.
    pub total: u32,
    /// The finished profile once every row is counted; null while scanning.
    pub profile: Option<ColumnProfile>,
}

/// Counts one column in slices so a caller can yield between them. Distinct values
/// are entries of (hash, first row position, count): the text stays in the document.
pub(crate) struct ProfileJob {
    column_id: u32,
    index: usize,
    next: usize,
    table: HashTable<(u64, u32, u32)>,
    common: Vec<(u32, u32)>,
    empty: u32,
    missing: u32,
    nulls: u32,
    kinds: u8,
}

impl ProfileJob {
    pub(crate) fn new(column_id: u32, index: usize) -> Self {
        Self {
            column_id,
            index,
            next: 0,
            table: HashTable::new(),
            common: Vec::with_capacity(COMMON_VALUES + 1),
            empty: 0,
            missing: 0,
            nulls: 0,
            kinds: 0,
        }
    }

    pub(crate) fn column_id(&self) -> u32 {
        self.column_id
    }

    pub(crate) fn scanned(&self) -> usize {
        self.next
    }

    /// Count up to `limit` more rows; returns the profile once every row is counted.
    /// The document must be the one the job started on, unchanged since.
    pub(crate) fn step(&mut self, document: &Document, limit: usize) -> Option<ColumnProfile> {
        let rows = &document.rows;
        let index = self.index;
        let cell = |row: u32| {
            rows[row as usize]
                .cells()
                .get(index)
                .unwrap_or(&CellValue::Missing)
        };
        let text = |row: u32| display_value(cell(row));
        let end = self.next.saturating_add(limit).min(rows.len());
        for (position, row) in rows.iter().enumerate().take(end).skip(self.next) {
            let value = row.cells().get(index).unwrap_or(&CellValue::Missing);
            self.kinds |= match value {
                CellValue::Missing => {
                    self.missing += 1;
                    continue;
                }
                CellValue::Null => {
                    self.nulls += 1;
                    1
                }
                CellValue::Text(_) => 2,
                CellValue::Boolean(_) => 4,
                CellValue::Number(_) => 8,
            };
            let empty = matches!(value, CellValue::Text(s) if s.is_empty());
            if empty {
                self.empty += 1;
            }
            let hash = FxBuildHasher.hash_one(value);
            let candidate = match self.table.entry(
                hash,
                |&(h, row, _)| h == hash && cell(row) == value,
                |&(h, _, _)| h,
            ) {
                Entry::Occupied(mut entry) => {
                    let entry = entry.get_mut();
                    entry.2 += 1;
                    (entry.1, entry.2)
                }
                Entry::Vacant(entry) => {
                    entry.insert((hash, position as u32, 1));
                    (position as u32, 1)
                }
            };
            if empty || matches!(value, CellValue::Null) {
                continue;
            }
            // Maintain the five leaders within this slice. A final walk over all
            // distinct values can otherwise block the browser long after counting.
            let order = |a: &(u32, u32), b: &(u32, u32)| {
                b.1.cmp(&a.1).then_with(|| text(a.0).cmp(text(b.0)))
            };
            if let Some(at) = self.common.iter().position(|item| item.0 == candidate.0) {
                self.common.remove(at);
            }
            if self.common.len() < COMMON_VALUES
                || order(&candidate, &self.common[COMMON_VALUES - 1]).is_lt()
            {
                let at = self
                    .common
                    .partition_point(|item| order(item, &candidate).is_lt());
                self.common.insert(at, candidate);
                self.common.truncate(COMMON_VALUES);
            }
        }
        self.next = end;
        if end < rows.len() {
            return None;
        }
        Some(ColumnProfile {
            column_id: self.column_id,
            kind: match self.kinds {
                0 | 2 => "Text",
                1 => "Null",
                4 => "Boolean",
                8 => "Number",
                _ => "Mixed",
            }
            .into(),
            null_count: self.nulls,
            unique_count: saturating_u32(self.table.len()),
            empty_count: self.empty,
            missing_count: self.missing,
            common_values: self
                .common
                .iter()
                .map(|&(row, count)| {
                    let value = text(row);
                    let shown = prefix(value, FREQUENT_VALUE_PREVIEW_BYTES);
                    FrequentValue {
                        value: shown.into(),
                        byte_length: saturating_u32(value.len()),
                        truncated: shown.len() < value.len(),
                        count,
                    }
                })
                .collect(),
        })
    }
}

fn detect_format(text: &str, filename: &str, tab: bool) -> &'static str {
    let text = text.trim_start_matches('\u{feff}').trim_start();
    let lower = filename.to_ascii_lowercase();
    if text.starts_with('<') {
        return "xml";
    }
    if text.starts_with('{') || text.starts_with('[') {
        if lower.ends_with(".ndjson") || lower.ends_with(".jsonl") {
            return "ndjson";
        }
        let mut lines = text.lines().filter(|line| !line.trim().is_empty());
        if lines.next().is_some_and(|line| {
            serde_json::from_str::<Box<serde_json::value::RawValue>>(line).is_ok()
        }) && lines.next().is_some()
        {
            return "ndjson";
        }
        return "json";
    }
    if lower.ends_with(".json") {
        "json"
    } else if lower.ends_with(".ndjson") || lower.ends_with(".jsonl") {
        "ndjson"
    } else if lower.ends_with(".xml") {
        "xml"
    } else if tab {
        "tsv"
    } else {
        "csv"
    }
}

pub(crate) fn display_value(value: &CellValue) -> &str {
    match value {
        CellValue::Text(s) | CellValue::Number(s) => s,
        CellValue::Boolean(true) => "true",
        CellValue::Boolean(false) => "false",
        CellValue::Null => "null",
        CellValue::Missing => "",
    }
}
