use crate::components::UploadForm;
use dioxus::prelude::*;

#[component]
pub fn Upload() -> Element {
    rsx! {
        div { class: "min-h-screen bg-gray-50",
            div { class: "max-w-4xl mx-auto py-8",
                h1 { class: "text-3xl font-bold text-gray-800 text-center mb-8", "Upload zdjęcia" }
                UploadForm {}
            }
        }
    }
}
