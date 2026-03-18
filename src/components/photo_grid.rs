use dioxus::prelude::*;

#[component]
pub fn PhotoGrid() -> Element {
    rsx! {
        div { class: "grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-4 p-4",
            div { class: "col-span-full text-center text-gray-500 py-12",
                p { class: "text-lg", "Brak zdjęć" }
                p { class: "text-sm mt-2", "Dodaj zdjęcia, aby zobaczyć je tutaj" }
            }
        }
    }
}
