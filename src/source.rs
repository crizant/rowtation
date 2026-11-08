//! Import candidates held in Rust, so file bytes are copied into WASM once and
//! never mirrored in JS while the user reviews them.
use crate::{
    error::EngineError,
    inspection::{ImportAnalysis, analyze},
};
use nsh_core::prelude::*;

/// The bytes of one file or paste awaiting review. Inspect it, open it with
/// `DocumentSession.fromSource`, then dispose it: the session keeps its own data.
#[class]
pub struct ImportSource {
    pub(crate) bytes: Vec<u8>,
    name: String,
}

impl ImportSource {
    pub(crate) fn new(bytes: Vec<u8>, name: String) -> Self {
        Self { bytes, name }
    }
}

#[class]
impl ImportSource {
    /// Pasted text, encoded as UTF-8 in Rust.
    pub fn from_text(text: String, name: String) -> Self {
        Self::new(text.into_bytes(), name)
    }

    /// Display name, treated as text.
    #[getter]
    pub fn name(&self) -> String {
        self.name.clone()
    }

    /// Byte length, including a BOM.
    #[getter]
    pub fn byte_length(&self) -> f64 {
        self.bytes.len() as f64
    }

    /// Measure a structured candidate using its actual flattened shape. Parsing
    /// errors leave this source available for format correction.
    pub fn inspect_format(
        &self,
        format: String,
    ) -> Result<crate::inspection::ImportSize, EngineError> {
        let doc = crate::formats::structured::parse(&self.bytes, &format)?;
        let mut cells = 0usize;
        let mut longest = 0usize;
        for row in &doc.rows {
            for value in row.cells() {
                if !matches!(value, crate::document::CellValue::Missing) {
                    cells += 1;
                    longest = longest.max(crate::inspection::display_value(value).len());
                }
            }
        }
        use crate::inspection::*;
        Ok(ImportSize {
            records: crate::session::saturating_u32(doc.row_count()),
            columns: crate::session::saturating_u32(doc.column_count()),
            cells: crate::session::saturating_u32(cells),
            longest_field: crate::session::saturating_u32(longest),
            large: self.bytes.len() > LARGE_INPUT_BYTES
                || doc.row_count() > LARGE_RECORDS
                || doc.column_count() > LARGE_COLUMNS
                || cells > LARGE_CELLS
                || longest > LARGE_FIELD_BYTES,
        })
    }

    /// Suggest a separator and measure the whole input, as `ImportDetector.inspect`.
    pub fn inspect(&self) -> Result<ImportAnalysis, EngineError> {
        analyze(&self.bytes, &self.name)
    }
}
