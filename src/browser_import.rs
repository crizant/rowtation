//! Browser file I/O through the registered NSH API. No data processing in JavaScript.
use crate::source::ImportSource;
use nsh_core::prelude::*;

/// A picked or dropped file: read and waiting in Rust, deferred until the user confirms
/// a large read, or an actionable error. Bytes never leave the browser or enter JS.
#[derive(JsObject, Default)]
pub struct LocalImport {
    /// Local display name, treated as text.
    pub name: String,
    /// File size in bytes, known before reading.
    pub size: f64,
    /// True when the file is larger than LARGE_INPUT_BYTES and was not read yet.
    /// Call readDeferred to read it, or cancelPending to drop it.
    pub deferred: bool,
    /// Read generation, checked again when a queued callback reaches the UI.
    pub request_id: Option<String>,
    /// Read failure, if any; never replaces the active document.
    pub error: Option<String>,
    /// Milliseconds spent reading the file into WebAssembly memory.
    pub read_ms: f64,
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

    /// Take the file a delivered LocalImport read, once. Throws when the request is not
    /// current or was already taken.
    pub fn take(request_id: String) -> Result<ImportSource, String> {
        #[cfg(target_arch = "wasm32")]
        {
            browser::take(&request_id)
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = request_id;
            Err("File selection is available in the web application.".into())
        }
    }

    /// Read a deferred file after the user confirms. The result arrives through the
    /// onFile callback with a new request ID. Throws when the request is not current.
    pub fn read_deferred(request_id: String) -> Result<(), String> {
        #[cfg(target_arch = "wasm32")]
        {
            browser::read_deferred(&request_id)
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = request_id;
            Err("File selection is available in the web application.".into())
        }
    }

    /// Invalidate pending reads when the UI cancels or starts another import workflow,
    /// and release any deferred or read file. A read in progress stops at its next slice.
    pub fn cancel_pending() {
        #[cfg(target_arch = "wasm32")]
        browser::cancel_pending();
    }

    /// Check the same read generation at callback delivery, including callbacks that
    /// were already queued before cancelPending. IDs are opaque, not document IDs.
    pub fn is_current(request_id: String) -> bool {
        #[cfg(target_arch = "wasm32")]
        {
            browser::is_current(&request_id)
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = request_id;
            false
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
    use crate::{diagnostics::now_ms, inspection::LARGE_INPUT_BYTES};
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };
    use wasm_bindgen::{JsCast, JsValue, closure::Closure};
    use wasm_bindgen_futures::{JsFuture, spawn_local};
    use web_sys::{DragEvent, File, HtmlInputElement};
    const INPUT_ID: &str = "rowtation-file-input";
    /// Bytes per browser read; bounds the transient JS buffer, not the file.
    const READ_SLICE: usize = 16 * 1024 * 1024;
    const READ_ERROR: &str = "The browser could not read the file. It may be too large for this tab's memory, or it changed after selection.";

    struct Deferred {
        id: u64,
        file: File,
        callback: Callback<LocalImport>,
        live: Rc<Cell<bool>>,
    }

    // The adapter already owns one input/listener per app. Workflow cancellation
    // and out-of-order reads invalidate the same generation, not separate tokens.
    // At most one file waits for confirmation and one read file waits to be taken.
    thread_local! {
        static GENERATION: Rc<Cell<u64>> = Rc::new(Cell::new(0));
        static DEFERRED: RefCell<Option<Deferred>> = const { RefCell::new(None) };
        static LOADED: RefCell<Option<(u64, ImportSource)>> = const { RefCell::new(None) };
    }

    pub fn cancel_pending() {
        GENERATION.with(|generation| generation.set(generation.get().wrapping_add(1)));
        DEFERRED.take();
        LOADED.take();
    }

    pub fn take(request_id: &str) -> Result<ImportSource, String> {
        LOADED
            .with_borrow_mut(|loaded| match loaded.take() {
                Some((id, source)) if id.to_string() == request_id && is_current(request_id) => {
                    Some(source)
                }
                other => {
                    *loaded = other;
                    None
                }
            })
            .ok_or_else(|| "This file is no longer available. Choose it again.".into())
    }

    pub fn read_deferred(request_id: &str) -> Result<(), String> {
        let deferred = DEFERRED
            .with_borrow_mut(|deferred| {
                deferred.take_if(|d| d.id.to_string() == request_id && is_current(request_id))
            })
            .ok_or("This file is no longer available. Choose it again.")?;
        let generation = GENERATION.with(Rc::clone);
        deliver(
            deferred.file,
            deferred.callback,
            deferred.live,
            generation,
            true,
        );
        Ok(())
    }

    pub fn is_current(request_id: &str) -> bool {
        GENERATION.with(|generation| generation.get().to_string() == request_id)
    }

    fn document() -> Result<web_sys::Document, String> {
        web_sys::window()
            .and_then(|w| w.document())
            .ok_or("Browser document is unavailable.".into())
    }

    /// Read the file into one Rust buffer, a slice at a time, so the browser never
    /// holds a second full copy. Returns None when the request goes stale mid-read.
    async fn read(file: &File, id: u64, generation: &Cell<u64>) -> Result<Option<Vec<u8>>, String> {
        if file.size() > usize::MAX as f64 {
            return Err(
                "This file is larger than WebAssembly memory can address in a browser tab.".into(),
            );
        }
        let size = file.size() as usize;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(size)
            .map_err(|_| "This tab does not have enough memory to read the file.")?;
        while bytes.len() < size {
            let start = bytes.len();
            let end = start.saturating_add(READ_SLICE).min(size);
            let slice = file
                .slice_with_f64_and_f64(start as f64, end as f64)
                .map_err(|_| READ_ERROR)?;
            let buffer = JsFuture::from(slice.array_buffer())
                .await
                .map_err(|_| READ_ERROR)?;
            if generation.get() != id {
                return Ok(None);
            }
            let array = js_sys::Uint8Array::new(&buffer);
            let read = array.length() as usize;
            if read == 0 {
                return Err(READ_ERROR.into());
            }
            bytes.resize(start + read, 0);
            array.copy_to(&mut bytes[start..]);
        }
        Ok(Some(bytes))
    }

    pub fn pick() -> Result<(), String> {
        let input: HtmlInputElement = document()?
            .get_element_by_id(INPUT_ID)
            .ok_or("File picker is unavailable. Reload the page and try again.")?
            .unchecked_into();
        input.click();
        Ok(())
    }

    /// Start a new request for the file. Large files wait for confirmation unless
    /// `confirmed`; others are read and kept for take.
    fn deliver(
        file: File,
        callback: Callback<LocalImport>,
        live: Rc<Cell<bool>>,
        generation: Rc<Cell<u64>>,
        confirmed: bool,
    ) {
        let id = generation.get().wrapping_add(1);
        generation.set(id);
        DEFERRED.take();
        LOADED.take();
        let mut result = LocalImport {
            name: file.name(),
            size: file.size(),
            request_id: Some(id.to_string()),
            ..Default::default()
        };
        let lower = result.name.to_ascii_lowercase();
        if !(lower.ends_with(".csv") || lower.ends_with(".tsv")) {
            result.error =
                Some("Choose a .csv or .tsv file. Other formats are not supported yet.".into());
        } else if !confirmed && result.size > LARGE_INPUT_BYTES as f64 {
            result.deferred = true;
            DEFERRED.set(Some(Deferred {
                id,
                file,
                callback: callback.clone(),
                live: live.clone(),
            }));
        } else {
            spawn_local(async move {
                let start = now_ms();
                let read = read(&file, id, &generation).await;
                if !live.get() || generation.get() != id {
                    return;
                }
                match read {
                    Ok(Some(bytes)) => {
                        result.read_ms = now_ms() - start;
                        LOADED.set(Some((id, ImportSource::new(bytes, result.name.clone()))));
                    }
                    Ok(None) => return,
                    Err(error) => result.error = Some(error),
                }
                callback.post(result);
            });
            return;
        }
        callback.post(result);
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
        let generation = GENERATION.with(Rc::clone);
        let (picked, alive, version, changed) = (
            input.clone(),
            live.clone(),
            generation.clone(),
            callback.clone(),
        );
        let change = Closure::<dyn FnMut()>::new(move || {
            if let Some(file) = picked.files().and_then(|files| files.get(0)) {
                deliver(file, changed.clone(), alive.clone(), version.clone(), false);
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
                cancel_pending();
                callback.post(LocalImport {
                    request_id: Some(version.get().to_string()),
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
                false,
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
