use dioxus::prelude::*;

#[component]
pub fn About() -> Element {
    rsx! {
        div { class: "min-h-screen bg-gray-50",
            div { class: "max-w-3xl mx-auto py-12 px-4",
                h1 { class: "text-4xl font-bold text-gray-800 mb-4", "PhotoVault" }
                p { class: "text-lg text-gray-600 mb-8",
                    "Galeria zdjęć napisana w 100% w Rust — zero JavaScript."
                }

                h2 { class: "text-2xl font-semibold text-gray-700 mb-4", "Tech Stack" }
                div { class: "grid grid-cols-2 gap-4",
                    div { class: "bg-white p-4 rounded-lg shadow-sm",
                        h3 { class: "font-semibold text-gray-800", "Dioxus 0.7" }
                        p { class: "text-sm text-gray-500", "Fullstack Rust framework — frontend kompilowany do WASM" }
                    }
                    div { class: "bg-white p-4 rounded-lg shadow-sm",
                        h3 { class: "font-semibold text-gray-800", "SQLite" }
                        p { class: "text-sm text-gray-500", "Lekka baza danych z FTS5 do wyszukiwania" }
                    }
                    div { class: "bg-white p-4 rounded-lg shadow-sm",
                        h3 { class: "font-semibold text-gray-800", "Tailwind CSS" }
                        p { class: "text-sm text-gray-500", "Utility-first CSS framework" }
                    }
                    div { class: "bg-white p-4 rounded-lg shadow-sm",
                        h3 { class: "font-semibold text-gray-800", "WebAssembly" }
                        p { class: "text-sm text-gray-500", "Natywna wydajność w przeglądarce" }
                    }
                }
            }
        }
    }
}
