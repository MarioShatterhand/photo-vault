use dioxus::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};

#[allow(dead_code)]
static LAZY_IMG_COUNTER: AtomicU64 = AtomicU64::new(0);

#[component]
pub fn LazyImage(src: String, alt: String, class: String) -> Element {
    let mut in_viewport = use_signal(|| true); // true for SSR — real src in initial HTML
    let mut img_loaded = use_signal(|| false);
    let element_id = use_hook(|| {
        format!("lazy-img-{}", LAZY_IMG_COUNTER.fetch_add(1, Ordering::Relaxed))
    });

    // use_effect only runs on the client after hydration
    {
        let element_id = element_id.clone();
        use_effect(move || {
            // On the client: flip in_viewport to false so IntersectionObserver controls it
            in_viewport.set(false);

            #[cfg(target_arch = "wasm32")]
            {
                use wasm_bindgen::closure::Closure;
                use wasm_bindgen::JsCast;

                let window = match web_sys::window() {
                    Some(w) => w,
                    None => return,
                };
                let document = match window.document() {
                    Some(d) => d,
                    None => return,
                };
                let element = match document.get_element_by_id(&element_id) {
                    Some(el) => el,
                    None => return,
                };

                let mut in_vp = in_viewport;
                let callback =
                    Closure::<dyn FnMut(js_sys::Array, web_sys::IntersectionObserver)>::new(
                        move |entries: js_sys::Array,
                              observer: web_sys::IntersectionObserver| {
                            for entry in entries.iter() {
                                let entry: web_sys::IntersectionObserverEntry =
                                    wasm_bindgen::JsCast::unchecked_into(entry);
                                if entry.is_intersecting() {
                                    in_vp.set(true);
                                    observer.disconnect();
                                    return;
                                }
                            }
                        },
                    );

                let options = web_sys::IntersectionObserverInit::new();
                let threshold_array = js_sys::Array::new();
                threshold_array.push(&wasm_bindgen::JsValue::from_f64(0.1));
                options.set_threshold(&threshold_array);

                if let Ok(observer) = web_sys::IntersectionObserver::new_with_options(
                    callback.as_ref().unchecked_ref(),
                    &options,
                ) {
                    observer.observe(&element);
                }

                // Intentional small leak: the closure lives for the lifetime of the observer.
                // The observer disconnects itself after the element enters the viewport,
                // so this is a one-shot allocation per image.
                callback.forget();
            }
        });
    }

    let display_src = if in_viewport() { src.clone() } else { String::new() };
    let opacity = if img_loaded() { "opacity-100" } else { "opacity-0" };

    rsx! {
        div {
            id: "{element_id}",
            class: "relative overflow-hidden",

            // Skeleton shown until the image fires its onload event
            if !img_loaded() {
                div {
                    class: "absolute inset-0 bg-gray-200 animate-pulse rounded",
                }
            }

            img {
                src: "{display_src}",
                alt: "{alt}",
                class: "{class} transition-opacity duration-300 {opacity}",
                onload: move |_| {
                    img_loaded.set(true);
                },
            }
        }
    }
}
