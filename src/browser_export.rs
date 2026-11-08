//! Browser transport only. Serialization remains in the document engine.
use crate::error::EngineError;
/// Sanitize a user filename and replace the final supported input extension.
pub fn filename(name: &str, format: &str) -> Result<String, EngineError> {
    if !matches!(format, "csv" | "tsv" | "json" | "ndjson" | "xml") {
        return Err(EngineError::new(
            "INVALID_FORMAT",
            "Choose a supported output format.",
        ));
    }
    let name: String = name
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                c
            }
        })
        .collect();
    let name = name.trim().trim_matches('.');
    let stem = if let Some((stem, ext)) = name.rsplit_once('.') {
        if matches!(
            ext.to_ascii_lowercase().as_str(),
            "csv" | "tsv" | "json" | "jsonl" | "ndjson" | "xml"
        ) {
            stem
        } else {
            name
        }
    } else {
        name
    };
    Ok(format!(
        "{}.{format}",
        if stem.is_empty() { "export" } else { stem }
    ))
}
/// Create and release a temporary anchor and object URL. No data is uploaded.
pub fn download(bytes: &[u8], filename: &str) -> Result<(), EngineError> {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;
        let err = |_| {
            EngineError::new(
                "DOWNLOAD_FAILED",
                "The browser could not start the download. Check available memory and browser download permissions.",
            )
        };
        let document = web_sys::window()
            .and_then(|w| w.document())
            .ok_or_else(|| {
                EngineError::new("DOWNLOAD_FAILED", "Browser document is unavailable.")
            })?;
        let mime = match filename.rsplit('.').next().unwrap_or("") {
            "csv" => "text/csv;charset=utf-8",
            "tsv" => "text/tab-separated-values;charset=utf-8",
            "json" => "application/json",
            "ndjson" => "application/x-ndjson",
            _ => "application/xml;charset=utf-8",
        };
        let parts = js_sys::Array::new();
        parts.push(&js_sys::Uint8Array::from(bytes));
        let options = web_sys::BlobPropertyBag::new();
        options.set_type(mime);
        let blob =
            web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options).map_err(err)?;
        let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(err)?;
        let result = (|| {
            let anchor = document
                .create_element("a")
                .map_err(err)?
                .dyn_into::<web_sys::HtmlAnchorElement>()
                .map_err(|_| {
                    EngineError::new("DOWNLOAD_FAILED", "Browser download links are unavailable.")
                })?;
            anchor.set_href(&url);
            anchor.set_download(filename);
            document
                .body()
                .ok_or_else(|| EngineError::new("DOWNLOAD_FAILED", "Browser body is unavailable."))?
                .append_child(&anchor)
                .map_err(err)?;
            anchor.click();
            anchor.remove();
            Ok(())
        })();
        // Delay revocation until the browser has consumed the click's URL.
        let revoke_url = url.clone();
        let revoke = wasm_bindgen::closure::Closure::once(move || {
            let _ = web_sys::Url::revoke_object_url(&revoke_url);
        });
        let scheduled = web_sys::window().is_some_and(|window| {
            window
                .set_timeout_with_callback_and_timeout_and_arguments_0(
                    revoke.as_ref().unchecked_ref(),
                    1000,
                )
                .is_ok()
        });
        if scheduled {
            revoke.forget();
        } else {
            let _ = web_sys::Url::revoke_object_url(&url);
        }

        result
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (bytes, filename);
        Err(EngineError::new(
            "DOWNLOAD_UNAVAILABLE",
            "Downloads require the web application.",
        ))
    }
}
