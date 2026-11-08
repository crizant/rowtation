//! Rust-owned document engine, registered with NSH as `rowtation/native`.
pub mod browser_export;
pub mod browser_import;
pub mod browser_styles;
pub mod diagnostics;
pub mod document;
pub mod error;
pub mod formats;
pub mod inspection;
pub mod operations;
pub mod session;
pub mod source;

use nsh_core::prelude::*;
pub use session::DocumentSession;
pub use source::ImportSource;

/// Registers the retained document session API.
#[native_module("rowtation/native")]
fn native(registry: &mut Registry) {
    registry.class::<DocumentSession>();
    registry.class::<ImportSource>();
    registry.module::<inspection::ImportDetector>();
    registry.module::<browser_import::BrowserImport>();
    registry.module::<browser_styles::BrowserStyles>();
    registry.module::<diagnostics::Diagnostics>();
}
