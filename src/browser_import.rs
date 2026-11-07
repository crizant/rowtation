//! Browser file I/O through the registered NSH API. No data processing in JavaScript.
use nsh_core::prelude::*;

/// A local file, or an actionable file-read error. Bytes never leave the browser.
#[derive(JsObject, Default)]
pub struct LocalImport {
    /// Local display name, treated as text.
    pub name: String,
    /// File bytes, capped at 8 MiB before reading.
    pub bytes: Vec<u8>,
    /// Read failure, if any; never replaces the active document.
    pub error: Option<String>,
}

/// Web-only local file picker and drop integration.
#[module]
pub struct BrowserImport;

#[module]
impl BrowserImport {
    /// Open the browser file picker using the input owned by onFile.
    /// Runs synchronously inside the user's press, preserving browser activation.
    pub fn pick() -> Result<(), String> {
        #[cfg(target_arch = "wasm32")]
        {
            browser::pick()
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            Err("File selection is available in the web application.".into())
        }
    }

    /// Listen for picked or dropped files. The returned stop removes the file input
    /// and every listener. Multiple-file drops report an error instead of choosing one.
    pub fn on_file(
        #[args(file)] callback: Callback<LocalImport>,
        #[args(active)] drag: Callback<bool>,
    ) -> Result<Stop, String> {
        #[cfg(target_arch = "wasm32")]
        {
            browser::listen(callback, drag)
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = (callback, drag);
            Ok(Stop::new(|| {}))
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use super::*;
    use std::{cell::Cell, rc::Rc};
    use wasm_bindgen::{JsCast, JsValue, closure::Closure};
    use wasm_bindgen_futures::{JsFuture, spawn_local};
    use web_sys::{DragEvent, File, HtmlInputElement};
    const INPUT_ID: &str = "rowtation-file-input";

    fn document() -> Result<web_sys::Document, String> {
        web_sys::window()
            .and_then(|w| w.document())
            .ok_or("Browser document is unavailable.".into())
    }

    async fn read(file: File) -> LocalImport {
        let mut result = LocalImport {
            name: file.name(),
            ..Default::default()
        };
        let lower = result.name.to_ascii_lowercase();
        if !(lower.ends_with(".csv") || lower.ends_with(".tsv")) {
            result.error =
                Some("Choose a .csv or .tsv file. Other formats are not supported yet.".into());
        } else if file.size() > crate::document::MAX_INPUT_BYTES as f64 {
            result.error = Some("Choose a file of 8 MiB or less.".into());
        } else {
            match JsFuture::from(file.array_buffer()).await {
                Ok(buffer) => result.bytes = js_sys::Uint8Array::new(&buffer).to_vec(),
                Err(_) => {
                    result.error = Some(
                        "The file could not be read. Select it again or paste its text.".into(),
                    )
                }
            }
        }
        result
    }

    pub fn pick() -> Result<(), String> {
        let input: HtmlInputElement = document()?
            .get_element_by_id(INPUT_ID)
            .ok_or("File picker is unavailable. Reload the page and try again.")?
            .unchecked_into();
        input.click();
        Ok(())
    }

    fn deliver(
        file: File,
        callback: Callback<LocalImport>,
        live: Rc<Cell<bool>>,
        generation: Rc<Cell<u64>>,
    ) {
        let id = generation.get().wrapping_add(1);
        generation.set(id);
        spawn_local(async move {
            let result = read(file).await;
            if live.get() && generation.get() == id {
                callback.post(result);
            }
        });
    }

    pub fn listen(callback: Callback<LocalImport>, drag: Callback<bool>) -> Result<Stop, String> {
        let document = document()?;
        let input: HtmlInputElement = document
            .create_element("input")
            .map_err(|_| "Could not create file picker.")?
            .unchecked_into();
        input.set_id(INPUT_ID);
        input.set_type("file");
        input.set_accept(".csv,.tsv,text/csv,text/tab-separated-values");
        input
            .set_attribute("aria-label", "Choose CSV or TSV file")
            .ok();
        input.set_attribute("style", "display:none").ok();
        document
            .body()
            .ok_or("Browser body is unavailable.")?
            .append_child(&input)
            .map_err(|_| "Could not enable file picker.")?;
        let live = Rc::new(Cell::new(true));
        let generation = Rc::new(Cell::new(0));
        let (picked, alive, version, changed) = (
            input.clone(),
            live.clone(),
            generation.clone(),
            callback.clone(),
        );
        let change = Closure::<dyn FnMut()>::new(move || {
            if let Some(file) = picked.files().and_then(|files| files.get(0)) {
                deliver(file, changed.clone(), alive.clone(), version.clone());
            }
            picked.set_value(""); // The same file can be selected again after a failure.
        });
        input.set_onchange(Some(change.as_ref().unchecked_ref()));
        let over = Closure::<dyn FnMut(DragEvent)>::new(|event: DragEvent| {
            if event
                .data_transfer()
                .is_some_and(|data| data.types().includes(&JsValue::from_str("Files"), 0))
            {
                event.prevent_default();
            }
        });
        let depth = Rc::new(Cell::new(0usize));
        let (entered, entered_drag) = (depth.clone(), drag.clone());
        let enter = Closure::<dyn FnMut(DragEvent)>::new(move |event: DragEvent| {
            if event
                .data_transfer()
                .is_some_and(|data| data.types().includes(&JsValue::from_str("Files"), 0))
            {
                entered.set(entered.get() + 1);
                entered_drag.post(true);
            }
        });
        let (left, left_drag) = (depth.clone(), drag.clone());
        let leave = Closure::<dyn FnMut(DragEvent)>::new(move |_: DragEvent| {
            left.set(left.get().saturating_sub(1));
            if left.get() == 0 {
                left_drag.post(false);
            }
        });
        let (alive, version) = (live.clone(), generation.clone());
        let drop = Closure::<dyn FnMut(DragEvent)>::new(move |event: DragEvent| {
            let Some(files) = event.data_transfer().and_then(|data| data.files()) else {
                return;
            };
            if files.length() == 0 {
                return;
            }
            event.prevent_default();
            depth.set(0);
            drag.post(false);
            if files.length() != 1 {
                version.set(version.get().wrapping_add(1));
                callback.post(LocalImport {
                    error: Some("Drop one CSV or TSV file at a time.".into()),
                    ..Default::default()
                });
                return;
            }
            deliver(
                files.get(0).unwrap(),
                callback.clone(),
                alive.clone(),
                version.clone(),
            );
        });
        document
            .add_event_listener_with_callback("dragover", over.as_ref().unchecked_ref())
            .map_err(|_| "Could not enable file drops.")?;
        document
            .add_event_listener_with_callback("drop", drop.as_ref().unchecked_ref())
            .map_err(|_| "Could not enable file drops.")?;
        document
            .add_event_listener_with_callback("dragenter", enter.as_ref().unchecked_ref())
            .map_err(|_| "Could not enable file drops.")?;
        document
            .add_event_listener_with_callback("dragleave", leave.as_ref().unchecked_ref())
            .map_err(|_| "Could not enable file drops.")?;
        Ok(Stop::new(move || {
            live.set(false);
            document
                .remove_event_listener_with_callback("dragover", over.as_ref().unchecked_ref())
                .ok();
            document
                .remove_event_listener_with_callback("drop", drop.as_ref().unchecked_ref())
                .ok();
            document
                .remove_event_listener_with_callback("dragenter", enter.as_ref().unchecked_ref())
                .ok();
            document
                .remove_event_listener_with_callback("dragleave", leave.as_ref().unchecked_ref())
                .ok();
            input.set_onchange(None);
            input.remove();
            std::mem::drop(change);
        }))
    }
}
