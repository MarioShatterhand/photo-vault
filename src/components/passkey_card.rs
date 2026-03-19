use dioxus::prelude::*;

#[component]
pub fn PasskeyCard(
    passkey: serde_json::Value,
    on_delete: EventHandler<i64>,
    on_rename: EventHandler<(i64, String)>,
) -> Element {
    let id = passkey["id"].as_i64().unwrap_or(0);
    let name = passkey["name"].as_str().unwrap_or("Klucz").to_string();
    let created_at = passkey["created_at"].as_str().unwrap_or("-").to_string();
    let last_used: Option<String> = passkey["last_used"].as_str().map(|s| s.to_string());

    let mut editing = use_signal(|| false);
    let mut edit_name = use_signal(move || name.clone());
    let mut confirming_delete = use_signal(|| false);

    let display_name = passkey["name"].as_str().unwrap_or("Klucz").to_string();

    rsx! {
        div { class: "bg-slate-800 rounded-lg p-4 border border-slate-700",
            div { class: "flex items-center justify-between",
                div { class: "flex-1",
                    if editing() {
                        div { class: "flex items-center gap-2",
                            input {
                                class: "bg-slate-700 text-white border border-slate-600 rounded px-2 py-1 text-sm focus:outline-none focus:border-blue-500",
                                value: "{edit_name}",
                                oninput: move |e| edit_name.set(e.value()),
                            }
                            button {
                                class: "text-green-400 hover:text-green-300 text-sm",
                                onclick: move |_| {
                                    on_rename.call((id, edit_name().clone()));
                                    editing.set(false);
                                },
                                "Zapisz"
                            }
                            button {
                                class: "text-slate-400 hover:text-slate-300 text-sm",
                                onclick: move |_| editing.set(false),
                                "Anuluj"
                            }
                        }
                    } else {
                        div {
                            h3 { class: "text-white font-medium", "{display_name}" }
                            div { class: "text-slate-400 text-sm mt-1",
                                span { "Dodano: {created_at}" }
                                if let Some(ref lu) = last_used {
                                    span { class: "ml-3", "Ostatnio uzyty: {lu}" }
                                }
                            }
                        }
                    }
                }

                if !editing() {
                    div { class: "flex items-center gap-2 ml-4",
                        button {
                            class: "text-slate-400 hover:text-blue-400 text-sm transition-colors",
                            onclick: move |_| editing.set(true),
                            "Zmien nazwe"
                        }
                        if confirming_delete() {
                            div { class: "flex items-center gap-1",
                                span { class: "text-red-400 text-sm", "Na pewno?" }
                                button {
                                    class: "text-red-400 hover:text-red-300 text-sm font-medium",
                                    onclick: move |_| {
                                        on_delete.call(id);
                                        confirming_delete.set(false);
                                    },
                                    "Tak"
                                }
                                button {
                                    class: "text-slate-400 hover:text-slate-300 text-sm",
                                    onclick: move |_| confirming_delete.set(false),
                                    "Nie"
                                }
                            }
                        } else {
                            button {
                                class: "text-slate-400 hover:text-red-400 text-sm transition-colors",
                                onclick: move |_| confirming_delete.set(true),
                                "Usun"
                            }
                        }
                    }
                }
            }
        }
    }
}
