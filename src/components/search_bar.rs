use dioxus::prelude::*;

#[component]
pub fn SearchBar() -> Element {
    let mut query = use_signal(|| String::new());

    rsx! {
        div { class: "w-full max-w-2xl mx-auto p-4",
            input {
                class: "w-full px-4 py-2 rounded-lg border border-gray-300 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent",
                r#type: "text",
                placeholder: "Szukaj zdjęć...",
                value: "{query}",
                oninput: move |e| *query.write() = e.value(),
            }
        }
    }
}
