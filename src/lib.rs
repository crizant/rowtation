//! Rust-owned document engine, registered with NSH as `rowtation/native`.
pub mod browser_import;
pub mod browser_styles;
pub mod document;
pub mod error;
pub mod formats;
pub mod inspection;
pub mod session;

use nsh_core::prelude::*;
pub use session::DocumentSession;

/// Registers the retained document session API.
#[native_module("rowtation/native")]
fn native(registry: &mut Registry) {
    registry.class::<DocumentSession>();
    registry.module::<inspection::ImportDetector>();
    registry.module::<browser_import::BrowserImport>();
    registry.module::<browser_styles::BrowserStyles>();
}
