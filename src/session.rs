//! NSH transport records contain only requested snapshots; the session owns Rust data.
use crate::inspection::{ColumnProfile, FrequentValue};
use crate::{
    document::{CellValue, Document},
    error::EngineError,
    formats::delimited,
};
use nsh_core::prelude::*;
use std::collections::{HashMap, VecDeque};

/// Explicit CSV/TSV import options. CSV with a header is the default.
#[derive(JsObject, Default)]
#[non_exhaustive]
pub struct ImportOptions {
    /// One ASCII character; defaults to comma. Pass a tab for TSV.
    pub delimiter: Option<String>,
    /// Defaults to true; false generates Column 1, Column 2, etc.
    pub headers: Option<bool>,
}

/// Serialization options. Defaults to the imported delimiter and header mode.
#[derive(JsObject, Default)]
#[non_exhaustive]
pub struct ExportOptions {
    /// One ASCII delimiter, or the source delimiter when omitted.
    pub delimiter: Option<String>,
    /// Include ordered column names, or use the source header mode when omitted.
    pub headers: Option<bool>,
}

/// A bounded range in the current row order, using zero-based positions.
#[derive(JsObject, Default)]
#[non_exhaustive]
pub struct RowWindow {
    /// First row position; may equal rowCount for an empty result.
    pub offset: u32,
    /// Requested maximum rows, at most 256; zero is valid.
    pub limit: u32,
}

/// Small document snapshot; revision and dirty remain unchanged by reads/export.
#[derive(JsObject, Default)]
#[non_exhaustive]
pub struct DocumentMetadata {
    /// Runtime-unique document identity. Not persistent across browser reloads.
    pub id: String,
    /// Number of data rows, excluding a header.
    pub row_count: u32,
    /// Number of ordered columns, including generated extra-field columns.
    pub column_count: u32,
    /// Zero for a newly imported document; reserved for future mutations.
    pub revision: u32,
    /// False for an imported document; reserved for future mutations.
    pub dirty: bool,
    /// Imported delimiter character.
    pub delimiter: String,
    /// Whether input had a header record.
    pub headers: bool,
}

/// Ordered column metadata. Identities are scoped to the owning session.
#[derive(JsObject, Default)]
#[non_exhaustive]
pub struct ColumnMetadata {
    /// Stable opaque identity, independent of the column's name.
    pub id: u32,
    /// Current name; duplicate and empty source headers are allowed.
    pub name: String,
    /// Imported header, or null for generated names.
    pub original_name: Option<String>,
}

/// Cell semantic type; CSV imports never infer numbers, booleans or null.
#[derive(PropEnum, Default)]
pub enum CellKind {
    /// Absent field, including the tail of a short delimited record.
    #[default]
    Missing,
    /// Explicit null, reserved for future format adapters.
    Null,
    /// Literal text, including empty strings.
    Text,
    /// Boolean, reserved for future format adapters.
    Boolean,
    /// Exact numeric lexical form, reserved for future format adapters.
    Number,
}

/// A copied cell snapshot with the full value. Only the payload for kind has a value.
#[derive(JsObject, Default)]
#[non_exhaustive]
pub struct CellSnapshot {
    /// Discriminates missing, null, text, boolean and number.
    pub kind: CellKind,
    /// Literal text for text cells; null otherwise.
    pub text: Option<String>,
    /// Boolean payload, or null.
    pub boolean: Option<bool>,
    /// Exact numeric lexical form; null otherwise.
    pub number: Option<String>,
    /// UTF-8 payload length; zero for non-text/non-number cells.
    pub byte_length: u32,
}
impl From<&CellValue> for CellSnapshot {
    fn from(value: &CellValue) -> Self {
        let mut cell = Self::default();
        match value {
            CellValue::Missing => (),
            CellValue::Null => cell.kind = CellKind::Null,
            CellValue::Text(text) => {
                cell.kind = CellKind::Text;
                cell.byte_length = saturating_u32(text.len());
                cell.text = Some(text.clone());
            }
            CellValue::Boolean(value) => {
                cell.kind = CellKind::Boolean;
                cell.boolean = Some(*value);
            }
            CellValue::Number(text) => {
                cell.kind = CellKind::Number;
                cell.byte_length = saturating_u32(text.len());
                cell.number = Some(text.clone());
            }
        }
        cell
    }
}

pub(crate) fn saturating_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

/// One row of a requested window, with cells in the returned schema order.
#[derive(JsObject, Default)]
#[non_exhaustive]
pub struct RowSnapshot {
    /// Stable document-scoped identity; use with readCell, never a display index.
    pub id: u32,
    /// Full cell values in schema order, including implicit missing fields.
    pub cells: Vec<CellSnapshot>,
}

/// A row window tied to the document identity and revision that produced it.
#[derive(JsObject, Default)]
#[non_exhaustive]
pub struct RowsSnapshot {
    /// Owning document identity.
    pub document_id: String,
    /// Dataset revision; future UI can reject stale windows after mutations.
    pub revision: u32,
    /// Zero-based start position.
    pub offset: u32,
    /// Bounded row snapshots; fewer than limit at the end of the document.
    pub rows: Vec<RowSnapshot>,
}

/// Retains one Rust-owned document. Register cleanup(() => session.dispose()) in
/// the TSX owner. NSH releases it on dispose or garbage collection; calls after
/// disposal throw TypeError. Engine failures throw Error with a stable CODE: prefix.
#[class]
pub struct DocumentSession {
    document: Document,
    profiles: VecDeque<ColumnProfile>,
}

#[class]
impl DocumentSession {
    /// Import UTF-8 bytes atomically. A failed constructor creates no session and
    /// leaves any existing session intact. Defaults: comma delimiter, headers true.
    pub fn new(input: Vec<u8>, options: ImportOptions) -> Result<Self, EngineError> {
        Ok(Self {
            profiles: VecDeque::new(),
            document: delimited::parse(
                &input,
                options.delimiter.as_deref().unwrap_or(","),
                options.headers.unwrap_or(true),
            )?,
        })
    }

    /// Retrieve counts, identity, source options and revision without copying rows.
    pub fn metadata(&self) -> DocumentMetadata {
        DocumentMetadata {
            id: self.document.id().into(),
            row_count: self.document.row_count() as u32,
            column_count: self.document.column_count() as u32,
            revision: self.document.revision(),
            dirty: self.document.dirty(),
            delimiter: char::from(self.document.source_delimiter()).to_string(),
            headers: self.document.source_headers(),
        }
    }

    /// Retrieve the ordered schema.
    pub fn columns(&self) -> Vec<ColumnMetadata> {
        self.document
            .columns()
            .iter()
            .map(|c| ColumnMetadata {
                id: c.id(),
                name: c.name().into(),
                original_name: c.original_name().map(str::to_owned),
            })
            .collect()
    }

    /// Retrieve row identities and full cell values for at most 256 rows. Every
    /// column and the whole text of each value are copied; there is no byte budget.
    /// An offset past rowCount or a limit above 256 throws INVALID_WINDOW.
    pub fn rows(&self, window: RowWindow) -> Result<RowsSnapshot, EngineError> {
        let rows = self
            .document
            .window(window.offset as usize, window.limit as usize)?;
        Ok(RowsSnapshot {
            document_id: self.document.id().into(),
            revision: self.document.revision(),
            offset: window.offset,
            rows: rows
                .iter()
                .map(|r| RowSnapshot {
                    id: r.id(),
                    cells: (0..self.document.column_count())
                        .map(|i| r.cells().get(i).unwrap_or(&CellValue::Missing).into())
                        .collect(),
                })
                .collect(),
        })
    }

    /// Read one cell by stable IDs discovered through rows and columns.
    /// Unknown IDs throw UNKNOWN_ROW or UNKNOWN_COLUMN; absent fields are missing.
    pub fn read_cell(&self, row_id: u32, column_id: u32) -> Result<CellSnapshot, EngineError> {
        Ok(self.document.cell(row_id, column_id)?.into())
    }

    /// Exact column statistics, computed once per cached column. Keeps at most eight
    /// small summaries; frequency maps borrow Rust text and are freed after profiling.
    pub fn profile(&mut self, column_id: u32) -> Result<ColumnProfile, EngineError> {
        if let Some(profile) = self.profiles.iter().find(|p| p.column_id == column_id) {
            return Ok(profile.clone());
        }
        let index = self
            .document
            .columns()
            .iter()
            .position(|c| c.id() == column_id)
            .ok_or_else(|| {
                EngineError::new("UNKNOWN_COLUMN", "Select a column in this document.")
            })?;
        let mut counts: HashMap<&str, u32> = HashMap::new();
        let mut profile = ColumnProfile {
            column_id,
            kind: "Text".into(),
            ..Default::default()
        };
        for row in &self.document.rows {
            match row.cells().get(index) {
                Some(CellValue::Text(value)) => {
                    *counts.entry(value.as_str()).or_default() += 1;
                    if value.is_empty() {
                        profile.empty_count += 1;
                    }
                }
                _ => profile.missing_count += 1,
            }
        }
        profile.unique_count = counts.len() as u32;
        let mut top: Vec<_> = counts
            .into_iter()
            .filter(|(value, _)| !value.is_empty())
            .collect();
        // Only five entries are needed, so avoid sorting the whole distinct-value set.
        let order = |a: &(&str, u32), b: &(&str, u32)| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0));
        if top.len() > 5 {
            top.select_nth_unstable_by(5, order);
            top.truncate(5);
        }
        top.sort_unstable_by(order);
        profile.common_values = top
            .into_iter()
            .map(|(value, count)| FrequentValue {
                value: value.into(),
                count,
            })
            .collect();
        if self.profiles.len() == 8 {
            self.profiles.pop_front();
        }
        self.profiles.push_back(profile.clone());
        Ok(profile)
    }

    /// Serialize the whole document to UTF-8 bytes (Uint8Array). This is an internal
    /// operation, not a browser download. Does not mutate dirty state or revision.
    pub fn serialize(&self, options: ExportOptions) -> Result<Vec<u8>, EngineError> {
        let source = char::from(self.document.source_delimiter()).to_string();
        delimited::serialize(
            &self.document,
            options.delimiter.as_deref().unwrap_or(&source),
            options.headers.unwrap_or(self.document.source_headers()),
        )
    }
}
