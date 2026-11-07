//! Rust-owned logic, exposed to TSX as `rowtation/native`.
//! The counter verifies the bridge until the converter is implemented.
use nsh_core::prelude::*;

/// Registers the application API.
#[native_module("rowtation/native")]
fn native(registry: &mut Registry) {
    registry.class::<Counter>();
}

/// Starter state for verifying the TSX-to-Rust bridge.
#[class]
pub struct Counter {
    count: u32,
}

#[class]
impl Counter {
    /// Creates a counter at zero.
    pub fn new() -> Self {
        Self { count: 0 }
    }

    /// Increments and returns the current count.
    pub fn increment(&mut self) -> u32 {
        self.count += 1;
        self.count
    }

    /// Returns the current count.
    #[getter]
    pub fn count(&self) -> u32 {
        self.count
    }
}
