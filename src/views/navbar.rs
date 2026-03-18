use crate::Route;
use dioxus::prelude::*;

#[component]
pub fn Navbar() -> Element {
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
                }
            }
        }
        Outlet::<Route> {}
    }
}
