//! Format-independent values and owned document storage. No JS dataset mirror.
use crate::error::EngineError;
use std::sync::atomic::{AtomicU64, Ordering};

/// Maximum rows returned by one window request. Documents themselves are unbounded.
pub const MAX_WINDOW_ROWS: usize = 256;

/// A value's semantic type. Number retains its exact lexical form, not an f64.
/// CSV imports produce Text only; absence in ragged rows reads as Missing.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CellValue {
    Missing,
    Null,
    Text(String),
    Boolean(bool),
    Number(String),
}

/// A stable schema entry, ordered by its position in Document::columns().
#[derive(Debug, PartialEq, Eq)]
pub struct Column {
    pub(crate) id: u32,
    pub(crate) name: String,
    pub(crate) original_name: Option<String>,
}
impl Column {
    /// Opaque document-scoped identity, never inferred from a name.
    pub fn id(&self) -> u32 {
        self.id
    }
    /// Display name; empty and duplicate imported names are preserved.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Original header, or None for generated columns.
    pub fn original_name(&self) -> Option<&str> {
        self.original_name.as_deref()
    }
}

/// Dense delimited rows keep their contiguous fast path. Structured rows store
/// only present (zero-based column index, value) pairs in sorted order.
#[derive(Debug, PartialEq, Eq)]
pub enum RowCells {
    Dense(Vec<CellValue>),
    Sparse(Vec<(usize, CellValue)>),
}
impl RowCells {
    /// Stored value at a schema index; absent entries are implicit Missing.
    pub fn get(&self, index: usize) -> Option<&CellValue> {
        match self {
            Self::Dense(values) => values.get(index),
            Self::Sparse(values) => values
                .binary_search_by_key(&index, |(i, _)| *i)
                .ok()
                .map(|i| &values[i].1),
        }
    }
    /// Number of stored entries, independent of schema width.
    pub fn stored_len(&self) -> usize {
        match self {
            Self::Dense(v) => v.len(),
            Self::Sparse(v) => v.len(),
        }
    }
    /// Iterate stored values without materializing implicit Missing cells.
    pub fn iter(&self) -> impl Iterator<Item = &CellValue> {
        let (dense, sparse): (&[CellValue], &[(usize, CellValue)]) = match self {
            Self::Dense(v) => (v, &[]),
            Self::Sparse(v) => (&[], v),
        };
        dense.iter().chain(sparse.iter().map(|(_, value)| value))
    }
    pub(crate) fn set(&mut self, index: usize, value: CellValue) {
        match self {
            Self::Dense(values) => {
                values.resize(values.len().max(index + 1), CellValue::Missing);
                values[index] = value;
                while values.last() == Some(&CellValue::Missing) {
                    values.pop();
                }
            }
            Self::Sparse(values) => match values.binary_search_by_key(&index, |(i, _)| *i) {
                Ok(at) if value == CellValue::Missing => {
                    values.remove(at);
                }
                Ok(at) => values[at].1 = value,
                Err(at) if value != CellValue::Missing => values.insert(at, (index, value)),
                Err(_) => (),
            },
        }
    }
}

/// A stable row. Its backing position is immutable; sort operations
/// order identities in a view rather than reorder this storage.
#[derive(Debug, PartialEq, Eq)]
pub struct Row {
    pub(crate) id: u32,
    pub(crate) cells: RowCells,
}
impl Row {
    /// Opaque document-scoped identity.
    pub fn id(&self) -> u32 {
        self.id
    }
    /// Stored fields; structured rows do not allocate absent properties.
    pub fn cells(&self) -> &RowCells {
        &self.cells
    }
}

static NEXT_DOCUMENT_ID: AtomicU64 = AtomicU64::new(1);

/// The authoritative dataset. Dropping it releases all columns, rows and text.
#[derive(Debug)]
pub struct Document {
    id: String,
    pub(crate) columns: Vec<Column>,
    pub(crate) rows: Vec<Row>,
    pub(crate) source_format: String,
    pub(crate) structure: Option<crate::formats::structured::Structure>,
    source_delimiter: u8,
    source_headers: bool,
    pub(crate) revision: u32,
    pub(crate) dirty: bool,
}

impl Document {
    pub(crate) fn new(
        columns: Vec<Column>,
        rows: Vec<Row>,
        delimiter: u8,
        headers: bool,
    ) -> Result<Self, EngineError> {
        let mut sequence = NEXT_DOCUMENT_ID.load(Ordering::Relaxed);
        loop {
            let next = sequence.checked_add(1).ok_or_else(|| {
                EngineError::new("ID_EXHAUSTED", "Document identity space exhausted.")
            })?;
            match NEXT_DOCUMENT_ID.compare_exchange_weak(
                sequence,
                next,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(current) => sequence = current,
            }
        }
        let id = format!("document-{sequence}");
        Ok(Self {
            id,
            columns,
            rows,
            source_format: if delimiter == b'\t' { "tsv" } else { "csv" }.into(),
            structure: None,
            source_delimiter: delimiter,
            source_headers: headers,
            revision: 0,
            dirty: false,
        })
    }
    /// Runtime-unique identity; not a persistent document identifier.
    pub fn id(&self) -> &str {
        &self.id
    }
    /// Ordered schema.
    pub fn columns(&self) -> &[Column] {
        &self.columns
    }
    /// Data row count, excluding the header.
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }
    /// Ordered schema width.
    pub fn column_count(&self) -> usize {
        self.columns.len()
    }
    /// Source format selected at import.
    pub fn source_format(&self) -> &str {
        &self.source_format
    }
    /// Reconstruction metadata; contains identities and shapes, never cell copies.
    pub fn structure(&self) -> Option<&crate::formats::structured::Structure> {
        self.structure.as_ref()
    }
    /// Imported delimiter byte.
    pub fn source_delimiter(&self) -> u8 {
        self.source_delimiter
    }
    /// Whether the input used a header record.
    pub fn source_headers(&self) -> bool {
        self.source_headers
    }
    /// Imported documents begin at revision zero. Reads and exports do not change it.
    pub fn revision(&self) -> u32 {
        self.revision
    }
    /// Whether current values differ from the imported document.
    pub fn dirty(&self) -> bool {
        self.dirty
    }

    /// Borrow a row window without copying the dataset. Offset equal to row_count
    /// is valid; larger offsets and limits above MAX_WINDOW_ROWS are errors.
    pub fn window(&self, offset: usize, limit: usize) -> Result<&[Row], EngineError> {
        if offset > self.rows.len() || limit > MAX_WINDOW_ROWS {
            return Err(EngineError::new(
                "INVALID_WINDOW",
                format!(
                    "Offset must be at most {} and limit at most {MAX_WINDOW_ROWS}.",
                    self.rows.len()
                ),
            ));
        }
        let end = offset.saturating_add(limit).min(self.rows.len());
        Ok(&self.rows[offset..end])
    }

    /// Read a cell by document-scoped stable identities, including implicit Missing.
    pub fn cell(&self, row_id: u32, column_id: u32) -> Result<&CellValue, EngineError> {
        let row = row_id
            .checked_sub(1)
            .and_then(|i| self.rows.get(i as usize))
            .filter(|r| r.id == row_id)
            .ok_or_else(|| {
                EngineError::new(
                    "UNKNOWN_ROW",
                    "Row identity does not exist in this document.",
                )
            })?;
        let index = column_id
            .checked_sub(1)
            .map(|i| i as usize)
            .filter(|&i| self.columns.get(i).is_some_and(|c| c.id == column_id))
            .ok_or_else(|| {
                EngineError::new(
                    "UNKNOWN_COLUMN",
                    "Column identity does not exist in this document.",
                )
            })?;
        Ok(row.cells.get(index).unwrap_or(&CellValue::Missing))
    }
}
