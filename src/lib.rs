//! Rust-owned document engine, registered with NSH as `rowtation/native`.
pub mod document;
pub mod error;
pub mod formats;
pub mod session;

use nsh_core::prelude::*;
pub use session::DocumentSession;

/// Registers the retained document session API.
#[native_module("rowtation/native")]
fn native(registry: &mut Registry) {
    registry.class::<DocumentSession>();
}
