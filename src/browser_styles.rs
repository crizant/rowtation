//! Scoped application CSS for browser-only presentation compatibility.
use nsh_core::prelude::*;

/// Installs application CSS on the browser surface; never receives imported data.
#[module]
pub struct BrowserStyles;

#[module]
impl BrowserStyles {
    /// Attach app-owned CSS and remove it when the returned stop or owner is disposed.
    /// Native development/test runners need no browser stylesheet.
    pub fn install(css: String) -> Result<Stop, String> {
        #[cfg(target_arch = "wasm32")]
        {
            let document = web_sys::window()
                .and_then(|window| window.document())
                .ok_or("Browser document is unavailable.")?;
            let style = document
                .create_element("style")
                .map_err(|_| "Could not apply application styles.")?;
            style.set_text_content(Some(&css));
            document
                .body()
                .ok_or("Browser body is unavailable.")?
                .append_child(&style)
                .map_err(|_| "Could not apply application styles.")?;
            Ok(Stop::new(move || style.remove()))
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = css;
            Ok(Stop::new(|| {}))
        }
    }
}
