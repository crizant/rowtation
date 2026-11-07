//! Opt-in local measurements. Nothing is recorded unless the UI asks, and nothing leaves the browser.
use nsh_core::prelude::*;

/// A monotonic clock in milliseconds: the page's `performance.now()` in the browser.
pub(crate) fn now_ms() -> f64 {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .and_then(|window| window.performance())
            .map_or_else(js_sys::Date::now, |performance| performance.now())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::{sync::OnceLock, time::Instant};
        static START: OnceLock<Instant> = OnceLock::new();
        START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
    }
}

/// Memory the engine can report reliably.
#[derive(JsObject, Default)]
pub struct MemoryUsage {
    /// WebAssembly linear memory in bytes. It grows but never shrinks, so it is the
    /// peak, not the live, size. Zero outside the browser.
    pub wasm_bytes: f64,
}

/// Local performance probes for the diagnostics panel.
#[module]
pub struct Diagnostics;

#[module]
impl Diagnostics {
    /// The size of WebAssembly linear memory.
    pub fn memory() -> MemoryUsage {
        #[cfg(target_arch = "wasm32")]
        let wasm_bytes = (core::arch::wasm32::memory_size::<0>() * 65_536) as f64;
        #[cfg(not(target_arch = "wasm32"))]
        let wasm_bytes = 0.0;
        MemoryUsage { wasm_bytes }
    }
}
