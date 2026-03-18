use crate::components::{SearchBar, PhotoGrid};
use dioxus::prelude::*;

#[component]
pub fn Home() -> Element {
    let refresh = use_signal(|| 0u64);
    use_context_provider(|| refresh);

    rsx! {
        div { class: "min-h-screen bg-gray-50",
            SearchBar {}
            PhotoGrid { refresh }
        }
    }
}
