use dioxus::prelude::*;

#[cfg(target_arch = "wasm32")]
use gloo_timers::callback::Timeout;

#[component]
pub fn SearchBar(mut query: Signal<String>) -> Element {
    let mut input_value = use_signal(|| String::new());

    #[cfg(target_arch = "wasm32")]
    let mut timeout_handle = use_signal(|| None::<Timeout>);

    let on_input = move |e: Event<FormData>| {
        let val = e.value();
        input_value.set(val.clone());

        #[cfg(target_arch = "wasm32")]
        {
            let mut query = query;
            timeout_handle.set(Some(Timeout::new(300, move || {
                query.set(val);
            })));
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            query.set(val);
        }
    };

    let on_clear = move |_| {
        input_value.set(String::new());
        query.set(String::new());
        #[cfg(target_arch = "wasm32")]
        timeout_handle.set(None);
    };

    rsx! {
        div { class: "w-full max-w-2xl mx-auto p-4",
            div { class: "relative",
                input {
                    class: "w-full px-4 py-2 pr-10 rounded-lg bg-slate-800 border border-slate-600 text-white placeholder-slate-400 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent",
                    r#type: "text",
                    placeholder: "Szukaj zdjęć...",
                    value: "{input_value}",
                    oninput: on_input,
                }
                if !input_value().is_empty() {
                    button {
                        class: "absolute right-2 top-1/2 -translate-y-1/2 text-slate-400 hover:text-white transition-colors px-2 py-1",
                        onclick: on_clear,
                        "\u{2715}"
                    }
                }
            }
        }
    }
}
