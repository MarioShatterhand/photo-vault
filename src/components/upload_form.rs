use dioxus::prelude::*;

#[component]
pub fn UploadForm() -> Element {
    rsx! {
        div { class: "max-w-xl mx-auto p-8",
            div { class: "border-2 border-dashed border-gray-300 rounded-lg p-12 text-center",
                p { class: "text-gray-500 text-lg mb-4", "Przeciągnij zdjęcia tutaj" }
                p { class: "text-gray-400 text-sm mb-6", "lub" }
                button {
                    class: "px-6 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors",
                    "Wybierz pliki"
                }
            }
        }
    }
}
