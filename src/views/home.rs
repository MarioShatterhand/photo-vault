use crate::components::{SearchBar, PhotoGrid};
use dioxus::prelude::*;

#[component]
pub fn Home() -> Element {
    rsx! {
        div { class: "min-h-screen bg-gray-50",
            SearchBar {}
            PhotoGrid {}
        }
    }
}
