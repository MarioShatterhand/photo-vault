use dioxus::prelude::*;
use crate::components::LoginForm;

#[component]
pub fn Login() -> Element {
    rsx! {
        div { class: "min-h-screen bg-slate-950 flex items-center justify-center",
            div { class: "bg-slate-800 rounded-xl shadow-2xl p-8 w-full max-w-md",
                h1 { class: "text-3xl font-bold text-white text-center mb-2", "PhotoVault" }
                p { class: "text-slate-400 text-center mb-8", "Zaloguj sie za pomoca klucza dostepu" }
                LoginForm {}
            }
        }
    }
}
