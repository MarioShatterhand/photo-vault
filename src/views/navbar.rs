use crate::Route;
use dioxus::prelude::*;

#[component]
pub fn Navbar() -> Element {
    let mut auth_checked = use_signal(|| false);
    let nav = use_navigator();

    // Auth guard: check session on mount, redirect if not authenticated
    use_effect(move || {
        spawn(async move {
            #[cfg(target_arch = "wasm32")]
            {
                match gloo_net::http::Request::get("/api/auth/status")
                    .send()
                    .await
                {
                    Ok(resp) => {
                        if let Ok(data) = resp.json::<serde_json::Value>().await {
                            let setup = data["setup"].as_bool().unwrap_or(false);
                            let authed = data["authenticated"].as_bool().unwrap_or(false);

                            if !setup {
                                nav.push(Route::Register {});
                                return;
                            }
                            if !authed {
                                nav.push(Route::Login {});
                                return;
                            }
                        }
                    }
                    Err(_) => {
                        nav.push(Route::Login {});
                        return;
                    }
                }
            }
            auth_checked.set(true);
        });
    });

    if !auth_checked() {
        return rsx! {
            div { class: "min-h-screen bg-slate-950 flex items-center justify-center",
                div { class: "text-slate-400", "Sprawdzanie sesji..." }
            }
        };
    }

    rsx! {
        nav { class: "bg-slate-900 text-white shadow-lg",
            div { class: "max-w-7xl mx-auto px-4 py-3 flex items-center justify-between",
                Link {
                    to: Route::Home {},
                    class: "text-xl font-bold tracking-tight hover:text-blue-400 transition-colors",
                    "PhotoVault"
                }
                div { class: "flex items-center gap-6",
                    Link {
                        to: Route::Home {},
                        class: "hover:text-blue-400 transition-colors",
                        "Galeria"
                    }
                    Link {
                        to: Route::Upload {},
                        class: "hover:text-blue-400 transition-colors",
                        "Upload"
                    }
                    Link {
                        to: Route::About {},
                        class: "hover:text-blue-400 transition-colors",
                        "O projekcie"
                    }
                    Link {
                        to: Route::Passkeys {},
                        class: "hover:text-blue-400 transition-colors",
                        "Klucze"
                    }
                    button {
                        class: "text-slate-400 hover:text-red-400 transition-colors text-sm cursor-pointer",
                        onclick: move |_| {
                            spawn(async move {
                                #[cfg(target_arch = "wasm32")]
                                {
                                    let _ = gloo_net::http::Request::post("/api/auth/logout")
                                        .send()
                                        .await;
                                    if let Some(window) = web_sys::window() {
                                        let _ = window.location().set_href("/login");
                                    }
                                }
                            });
                        },
                        "Wyloguj"
                    }
                }
            }
        }
        Outlet::<Route> {}
    }
}
