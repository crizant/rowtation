//! NSH transport records contain only requested snapshots; the session owns Rust data.
use crate::inspection::{ColumnProfile, ProfileJob, ProfileProgress};
use crate::{
    diagnostics::now_ms,
    document::{CellValue, Document},
    error::EngineError,
    formats::delimited,
    source::ImportSource,
};
use nsh_core::prelude::*;
use std::collections::VecDeque;

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

/// A bounded range in the current row order, using zero-based positions, optionally
/// narrowed to a column range and to previews of long values.
#[derive(JsObject, Default)]
#[non_exhaustive]
pub struct RowWindow {
    /// First row position; may equal rowCount for an empty result.
    pub offset: u32,
    /// Requested maximum rows, at most 256; zero is valid.
    pub limit: u32,
    /// First column position; may equal columnCount. Defaults to 0.
    pub column_offset: Option<u32>,
    /// Maximum columns; defaults to every column from columnOffset.
    pub column_limit: Option<u32>,
    /// Copy at most this many UTF-8 bytes of each value, cut on a character
    /// boundary and marked truncated. Defaults to whole values.
    pub preview_bytes: Option<u32>,
    /// Diagnostics: report rustMs. Defaults to false.
    pub measure: Option<bool>,
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

/// A copied cell snapshot. Only the payload for kind has a value. Values are whole
/// unless a window asked for previews; readCell always returns the whole value.
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
    /// UTF-8 length of the whole payload; zero for non-text/non-number cells.
    pub byte_length: u32,
    /// True when text or number holds only a prefix of the value.
    pub truncated: bool,
}
impl CellSnapshot {
    fn new(value: &CellValue, preview: usize) -> Self {
        let mut cell = Self::default();
        let mut copy = |text: &str| {
            cell.byte_length = saturating_u32(text.len());
            let shown = prefix(text, preview);
            cell.truncated = shown.len() < text.len();
            Some(shown.to_owned())
        };
        let (kind, text, number, boolean) = match value {
            CellValue::Missing => (CellKind::Missing, None, None, None),
            CellValue::Null => (CellKind::Null, None, None, None),
            CellValue::Text(text) => (CellKind::Text, copy(text), None, None),
            CellValue::Number(text) => (CellKind::Number, None, copy(text), None),
            CellValue::Boolean(value) => (CellKind::Boolean, None, None, Some(*value)),
        };
        cell.kind = kind;
        cell.text = text;
        cell.number = number;
        cell.boolean = boolean;
        cell
    }
}
impl From<&CellValue> for CellSnapshot {
    fn from(value: &CellValue) -> Self {
        Self::new(value, usize::MAX)
    }
}

/// The longest prefix of at most `bytes` bytes that ends on a character boundary.
pub(crate) fn prefix(text: &str, bytes: usize) -> &str {
    let mut end = text.len().min(bytes);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
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
    /// Position of the first column in each row's cells.
    pub column_offset: u32,
    /// Bounded row snapshots; fewer than limit at the end of the document.
    pub rows: Vec<RowSnapshot>,
    /// When the window asked to measure: milliseconds Rust spent building this
    /// snapshot, at the clock's resolution. Excludes the copy into JS.
    pub rust_ms: Option<f64>,
}

/// Retains one Rust-owned document. Register cleanup(() => session.dispose()) in
/// the TSX owner. NSH releases it on dispose or garbage collection; calls after
/// disposal throw TypeError. Engine failures throw Error with a stable CODE: prefix.
#[class]
pub struct DocumentSession {
    document: Document,
    profiles: VecDeque<ColumnProfile>,
    job: Option<ProfileJob>,
}

#[class]
impl DocumentSession {
    /// Import UTF-8 bytes atomically. A failed constructor creates no session and
    /// leaves any existing session intact. Defaults: comma delimiter, headers true.
    pub fn new(input: Vec<u8>, options: ImportOptions) -> Result<Self, EngineError> {
        Ok(Self {
            profiles: VecDeque::new(),
            job: None,
            document: delimited::parse(
                &input,
                options.delimiter.as_deref().unwrap_or(","),
                options.headers.unwrap_or(true),
            )?,
        })
    }

    /// Import a reviewed source without copying it through JS. The source keeps its
    /// bytes, so a failed import can be retried with other options; dispose it after.
    pub fn from_source(source: &ImportSource, options: ImportOptions) -> Result<Self, EngineError> {
        Ok(Self {
            profiles: VecDeque::new(),
            job: None,
            document: delimited::parse(
                &source.bytes,
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

    /// Retrieve row identities and cell values for at most 256 rows, in the requested
    /// columns. Values are whole unless previewBytes is set. An offset past rowCount,
    /// a columnOffset past columnCount or a limit above 256 throws INVALID_WINDOW.
    pub fn rows(&self, window: RowWindow) -> Result<RowsSnapshot, EngineError> {
        let started = window.measure.unwrap_or(false).then(now_ms);
        let rows = self
            .document
            .window(window.offset as usize, window.limit as usize)?;
        let count = self.document.column_count();
        let start = window.column_offset.unwrap_or(0) as usize;
        if start > count {
            return Err(EngineError::new(
                "INVALID_WINDOW",
                format!("Column offset must be at most {count}."),
            ));
        }
        let end = window.column_limit.map_or(count, |limit| {
            start.saturating_add(limit as usize).min(count)
        });
        let preview = window
            .preview_bytes
            .map_or(usize::MAX, |bytes| bytes as usize);
        let mut snapshot = RowsSnapshot {
            document_id: self.document.id().into(),
            revision: self.document.revision(),
            offset: window.offset,
            column_offset: saturating_u32(start),
            rows: rows
                .iter()
                .map(|r| RowSnapshot {
                    id: r.id(),
                    cells: (start..end)
                        .map(|i| {
                            CellSnapshot::new(
                                r.cells().get(i).unwrap_or(&CellValue::Missing),
                                preview,
                            )
                        })
                        .collect(),
                })
                .collect(),
            rust_ms: None,
        };
        snapshot.rust_ms = started.map(|started| now_ms() - started);
        Ok(snapshot)
    }

    /// Read one cell by stable IDs discovered through rows and columns.
    /// Unknown IDs throw UNKNOWN_ROW or UNKNOWN_COLUMN; absent fields are missing.
    pub fn read_cell(&self, row_id: u32, column_id: u32) -> Result<CellSnapshot, EngineError> {
        Ok(self.document.cell(row_id, column_id)?.into())
    }

    /// Exact column statistics, computed once per cached column. Keeps at most eight
    /// small summaries. Scans the whole column at once; prefer profileStep in the UI.
    pub fn profile(&mut self, column_id: u32) -> Result<ColumnProfile, EngineError> {
        loop {
            if let Some(profile) = self.profile_step(column_id, u32::MAX)?.profile {
                return Ok(profile);
            }
        }
    }

    /// Drop an unfinished profile scan and its distinct-value table. Finished profiles
    /// stay cached. Call when the UI abandons the column it was profiling.
    pub fn cancel_profile(&mut self) {
        self.job = None;
    }

    /// Count at most `rows` more rows of a column's statistics and report progress.
    /// Call again with the same column until `profile` is set, yielding in between to
    /// keep the UI responsive. Starting another column discards unfinished work.
    /// Finished and cached profiles return at once. Unknown IDs throw UNKNOWN_COLUMN.
    pub fn profile_step(
        &mut self,
        column_id: u32,
        rows: u32,
    ) -> Result<ProfileProgress, EngineError> {
        let total = saturating_u32(self.document.row_count());
        if let Some(profile) = self.profiles.iter().find(|p| p.column_id == column_id) {
            return Ok(ProfileProgress {
                column_id,
                scanned: total,
                total,
                profile: Some(profile.clone()),
            });
        }
        if self.job.as_ref().map(ProfileJob::column_id) != Some(column_id) {
            let index = self
                .document
                .columns()
                .iter()
                .position(|c| c.id() == column_id)
                .ok_or_else(|| {
                    EngineError::new("UNKNOWN_COLUMN", "Select a column in this document.")
                })?;
            self.job = Some(ProfileJob::new(column_id, index));
        }
        let Some(job) = self.job.as_mut() else {
            return Err(EngineError::new(
                "UNKNOWN_COLUMN",
                "Select a column in this document.",
            ));
        };
        let profile = job.step(&self.document, rows as usize);
        let scanned = saturating_u32(job.scanned());
        if let Some(profile) = &profile {
            self.job = None;
            if self.profiles.len() == 8 {
                self.profiles.pop_front();
            }
            self.profiles.push_back(profile.clone());
        }
        Ok(ProfileProgress {
            column_id,
            scanned,
            total,
            profile,
        })
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
