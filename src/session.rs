//! NSH transport records contain only requested snapshots; the session owns Rust data.
use crate::{
    document::{CellValue, Document, MAX_CELL_PREVIEW_BYTES, MAX_WINDOW_BYTES},
    error::EngineError,
    formats::delimited,
};
use nsh_core::prelude::*;

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

/// A copied cell snapshot. Row windows may contain flagged UTF-8 prefixes;
/// readCell always returns the full value. Only the payload for kind has a value.
#[derive(JsObject, Default)]
#[non_exhaustive]
pub struct CellSnapshot {
    /// Discriminates missing, null, text, boolean and number.
    pub kind: CellKind,
    /// Literal text or a flagged preview prefix for text cells; null otherwise.
    pub text: Option<String>,
    /// Boolean payload, or null.
    pub boolean: Option<bool>,
    /// Exact numeric lexical form or a flagged preview prefix; null otherwise.
    pub number: Option<String>,
    /// True when the payload is a preview prefix; readCell retrieves the full value.
    pub truncated: bool,
    /// Full UTF-8 payload length before previewing; zero for non-text/non-number cells.
    pub byte_length: u32,
}
impl From<&CellValue> for CellSnapshot {
    fn from(value: &CellValue) -> Self {
        Self::with_text_limit(value, usize::MAX)
    }
}
impl CellSnapshot {
    fn with_text_limit(value: &CellValue, limit: usize) -> Self {
        let mut cell = Self::default();
        match value {
            CellValue::Missing => (),
            CellValue::Null => cell.kind = CellKind::Null,
            CellValue::Text(text) => {
                cell.kind = CellKind::Text;
                cell.text = Some(cell.text_prefix(text, limit));
            }
            CellValue::Boolean(value) => {
                cell.kind = CellKind::Boolean;
                cell.boolean = Some(*value);
            }
            CellValue::Number(text) => {
                cell.kind = CellKind::Number;
                cell.number = Some(cell.text_prefix(text, limit));
            }
        }
        cell
    }

    fn text_prefix(&mut self, text: &str, limit: usize) -> String {
        let mut end = limit.min(text.len());
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        self.byte_length = text.len() as u32;
        self.truncated = end < text.len();
        text[..end].to_owned()
    }
}

/// One row of a requested window, with cells in the returned schema order.
#[derive(JsObject, Default)]
#[non_exhaustive]
pub struct RowSnapshot {
    /// Stable document-scoped identity; use with readCell, never a display index.
    pub id: u32,
    /// Cell previews in schema order, including implicit missing fields. Check
    /// truncated before treating a text/number payload as the full value.
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
}

#[class]
impl DocumentSession {
    /// Import UTF-8 bytes atomically. A failed constructor creates no session and
    /// leaves any existing session intact. Defaults: comma delimiter, headers true.
    pub fn new(input: Vec<u8>, options: ImportOptions) -> Result<Self, EngineError> {
        Ok(Self {
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

    /// Retrieve ordered schema only, at most 512 columns.
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

    /// Retrieve row identities and cell previews: at most 256 rows, 16,384 logical
    /// cells and 1 MiB of text. Text/number previews are UTF-8 prefixes of at most
    /// 256 bytes, reduced equally for wide windows to fit the total byte budget.
    /// Each cell reports truncated and byteLength; readCell returns the full value.
    /// Oversized row/cell counts throw INVALID_WINDOW or WINDOW_LIMIT.
    pub fn rows(&self, window: RowWindow) -> Result<RowsSnapshot, EngineError> {
        let rows = self
            .document
            .window(window.offset as usize, window.limit as usize)?;
        let cell_count = rows.len() * self.document.column_count();
        let preview_bytes = MAX_CELL_PREVIEW_BYTES.min(MAX_WINDOW_BYTES / cell_count.max(1));
        Ok(RowsSnapshot {
            document_id: self.document.id().into(),
            revision: self.document.revision(),
            offset: window.offset,
            rows: rows
                .iter()
                .map(|r| RowSnapshot {
                    id: r.id(),
                    cells: (0..self.document.column_count())
                        .map(|i| {
                            CellSnapshot::with_text_limit(
                                r.cells().get(i).unwrap_or(&CellValue::Missing),
                                preview_bytes,
                            )
                        })
                        .collect(),
                })
                .collect(),
        })
    }

    /// Read the full cell by stable IDs discovered through rows and columns.
    /// Always reports truncated false, including for values over the window budget.
    /// Unknown IDs throw UNKNOWN_ROW or UNKNOWN_COLUMN; absent fields are missing.
    pub fn read_cell(&self, row_id: u32, column_id: u32) -> Result<CellSnapshot, EngineError> {
        Ok(self.document.cell(row_id, column_id)?.into())
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
