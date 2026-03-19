use dioxus::prelude::*;
use crate::components::PasskeyCard;

#[component]
pub fn Passkeys() -> Element {
    let mut passkeys = use_signal(|| Vec::<serde_json::Value>::new());
    let mut loading = use_signal(|| true);
    let mut error = use_signal(|| None::<String>);
    let mut adding = use_signal(|| false);

    // Fetch passkeys on mount
    use_effect(move || {
        spawn(async move {
            #[cfg(target_arch = "wasm32")]
            {
                match gloo_net::http::Request::get("/api/auth/passkeys")
                    .send()
                    .await
                {
                    Ok(resp) if resp.ok() => {
                        if let Ok(data) = resp.json::<Vec<serde_json::Value>>().await {
                            passkeys.set(data);
                        }
                    }
                    Ok(resp) => {
                        error.set(Some(format!("Blad ladowania kluczy: {}", resp.status())));
                    }
                    Err(e) => {
                        error.set(Some(format!("Blad sieci: {e}")));
                    }
                }
                loading.set(false);
            }
        });
    });

    let on_add = move |_| {
        adding.set(true);
        error.set(None);
        spawn(async move {
            #[cfg(target_arch = "wasm32")]
            {
                match crate::components::register_form::webauthn_add_passkey().await {
                    Ok(new_passkey) => {
                        passkeys.write().push(new_passkey);
                    }
                    Err(e) => {
                        error.set(Some(e));
                    }
                }
            }
            adding.set(false);
        });
    };

    rsx! {
        div { class: "max-w-2xl mx-auto py-8 px-4",
            div { class: "flex items-center justify-between mb-6",
                h1 { class: "text-2xl font-bold text-white", "Zarzadzanie kluczami" }
                button {
                    class: "bg-blue-600 hover:bg-blue-700 text-white px-4 py-2 rounded-lg transition-colors disabled:opacity-50",
                    disabled: adding(),
                    onclick: on_add,
                    if adding() { "Dodawanie..." } else { "Dodaj klucz" }
                }
            }

            if let Some(err) = error() {
                div { class: "bg-red-900/50 border border-red-500 text-red-200 rounded-lg p-4 mb-4",
                    "{err}"
                }
            }

            if loading() {
                div { class: "text-slate-400 text-center py-8", "Ladowanie kluczy..." }
            } else if passkeys().is_empty() {
                div { class: "text-slate-400 text-center py-8", "Brak zarejestrowanych kluczy." }
            } else {
                div { class: "space-y-3",
                    for passkey in passkeys() {
                        PasskeyCard {
                            key: "{passkey[\"id\"]}",
                            passkey: passkey.clone(),
                            on_delete: {
                                let mut passkeys = passkeys.clone();
                                let mut error = error.clone();
                                move |id: i64| {
                                    let mut passkeys = passkeys.clone();
                                    let mut error = error.clone();
                                    spawn(async move {
                                        #[cfg(target_arch = "wasm32")]
                                        {
                                            match gloo_net::http::Request::delete(&format!("/api/auth/passkeys/{id}"))
                                                .send()
                                                .await
                                            {
                                                Ok(resp) if resp.ok() => {
                                                    passkeys.write().retain(|p| p["id"].as_i64() != Some(id));
                                                }
                                                Ok(resp) => {
                                                    if let Ok(body) = resp.json::<serde_json::Value>().await {
                                                        error.set(Some(body["error"].as_str().unwrap_or("Usuwanie nie powiodlo sie").to_string()));
                                                    }
                                                }
                                                Err(e) => {
                                                    error.set(Some(format!("Blad sieci: {e}")));
                                                }
                                            }
                                        }
                                    });
                                }
                            },
                            on_rename: {
                                let mut passkeys = passkeys.clone();
                                move |(id, new_name): (i64, String)| {
                                    let mut passkeys = passkeys.clone();
                                    let new_name2 = new_name.clone();
                                    spawn(async move {
                                        #[cfg(target_arch = "wasm32")]
                                        {
                                            let _ = gloo_net::http::Request::put(&format!("/api/auth/passkeys/{id}/name"))
                                                .json(&serde_json::json!({"name": new_name2}))
                                                .unwrap()
                                                .send()
                                                .await;
                                            for p in passkeys.write().iter_mut() {
                                                if p["id"].as_i64() == Some(id) {
                                                    p["name"] = serde_json::Value::String(new_name2.clone());
                                                }
                                            }
                                        }
                                    });
                                }
                            },
                        }
                    }
                }
            }

            div { class: "mt-6 p-4 bg-slate-800 rounded-lg",
                p { class: "text-slate-400 text-sm",
                    "Zalecamy rejestracje co najmniej 2 kluczy na wypadek utraty jednego z nich. Utrata wszystkich kluczy wymaga resetu bazy danych."
                }
            }
        }
    }
}
