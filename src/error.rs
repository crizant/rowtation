//! Errors retain machine-readable codes in Rust and readable locations in JS.
use std::fmt;

/// A recoverable engine failure. NSH exposes its Display text as an Error message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineError {
    code: &'static str,
    message: String,
    line: Option<usize>,
    record: Option<usize>,
}

impl EngineError {
    pub(crate) fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            line: None,
            record: None,
        }
    }

    pub(crate) fn at(mut self, line: usize, record: usize) -> Self {
        self.line = Some(line);
        self.record = Some(record);
        self
    }

    /// Stable error code, also the prefix of the JS Error message.
    pub fn code(&self) -> &'static str {
        self.code
    }
    /// Human-readable explanation, without code or location.
    pub fn message(&self) -> &str {
        &self.message
    }
    /// One-based physical line, when available.
    pub fn line(&self) -> Option<usize> {
        self.line
    }
    /// One-based logical record, including headers and blank rows.
    pub fn record(&self) -> Option<usize> {
        self.record
    }
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)?;
        if let (Some(line), Some(record)) = (self.line, self.record) {
            write!(f, " (line {line}, record {record})")?;
        }
        Ok(())
    }
}
impl std::error::Error for EngineError {}
