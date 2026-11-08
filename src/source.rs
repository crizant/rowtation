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

    /// Structured review checks byte size only, without parsing or validating a
    /// dataset. Exact dimensions and syntax errors are available after Import.
    pub fn inspect_format(
        &self,
        format: String,
    ) -> Result<crate::inspection::ImportSize, EngineError> {
        if !matches!(format.as_str(), "json" | "ndjson" | "xml") {
            return Err(EngineError::new(
                "INVALID_FORMAT",
                "Choose JSON, NDJSON or XML.",
            ));
        }
        Ok(crate::inspection::structured_review_size(self.bytes.len()))
    }

    /// Detect a structured prefix or measure delimited input, as `ImportDetector.inspect`.
    pub fn inspect(&self) -> Result<ImportAnalysis, EngineError> {
        analyze(&self.bytes, &self.name)
    }
}
