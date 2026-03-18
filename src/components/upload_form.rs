use dioxus::prelude::*;

#[component]
pub fn UploadForm() -> Element {
    let mut status = use_signal(|| String::new());
    let mut is_uploading = use_signal(|| false);

    rsx! {
        div { class: "max-w-xl mx-auto p-8",
            div { class: "border-2 border-dashed border-gray-300 rounded-lg p-12 text-center hover:border-blue-400 transition-colors",
                if is_uploading() {
                    div { class: "text-blue-600",
                        p { class: "text-lg mb-2", "Przesyłanie..." }
                        div { class: "w-8 h-8 border-4 border-blue-600 border-t-transparent rounded-full animate-spin mx-auto" }
                    }
                } else {
                    p { class: "text-gray-500 text-lg mb-4", "Przeciągnij zdjęcia tutaj" }
                    p { class: "text-gray-400 text-sm mb-6", "lub" }
                    label {
                        class: "px-6 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors cursor-pointer inline-block",
                        "Wybierz pliki"
                        input {
                            r#type: "file",
                            accept: "image/jpeg,image/png,image/webp,image/gif",
                            class: "hidden",
                            onchange: move |event| async move {
                                let files = event.files();
                                if files.is_empty() {
                                    return;
                                }

                                is_uploading.set(true);
                                status.set(String::new());

                                let file = &files[0];
                                let name = file.name();
                                status.set(format!("Przesyłanie {}...", name));

                                match upload_file(file).await {
                                    Ok(msg) => {
                                        status.set(msg);
                                        // Trigger gallery refresh via context if available
                                        if let Some(mut refresh) = try_consume_context::<Signal<u64>>() {
                                            *refresh.write() += 1;
                                        }
                                        navigator().push(crate::Route::Home {});
                                    }
                                    Err(e) => status.set(format!("Błąd: {}", e)),
                                }

                                is_uploading.set(false);
                            }
                        }
                    }
                }
            }

            if !status().is_empty() {
                p {
                    class: if status().starts_with("Błąd") { "mt-4 text-center text-sm text-red-500" } else { "mt-4 text-center text-sm text-green-600" },
                    "{status}"
                }
            }

            div { class: "mt-6 text-center text-xs text-gray-400",
                p { "Dozwolone formaty: JPEG, PNG, WebP, GIF" }
                p { "Maksymalny rozmiar: 20MB" }
            }
        }
    }
}

async fn upload_file(file: &dioxus::html::FileData) -> Result<String, String> {
    #[cfg(target_arch = "wasm32")]
    {
        use dioxus::web::WebFileExt;
        use wasm_bindgen::JsValue;

        let web_file = file
            .get_web_file()
            .ok_or_else(|| "Nie udało się odczytać pliku".to_string())?;

        let form_data = web_sys::FormData::new()
            .map_err(|_| "Nie udało się utworzyć FormData".to_string())?;

        form_data
            .append_with_blob_and_filename("file", web_file.as_ref(), &web_file.name())
            .map_err(|_| "Nie udało się dodać pliku do FormData".to_string())?;

        let response = gloo_net::http::Request::post("/api/upload")
            .body(JsValue::from(form_data))
            .map_err(|e| format!("Request error: {}", e))?
            .send()
            .await
            .map_err(|e| format!("Network error: {}", e))?;

        if response.ok() {
            Ok("Zdjęcie przesłane pomyślnie!".to_string())
        } else {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            Err(format!(
                "Serwer zwrócił błąd {}: {}",
                response.status(),
                error_text
            ))
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = file;
        Err("Upload dostępny tylko w przeglądarce".to_string())
    }
}
