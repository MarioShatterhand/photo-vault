use dioxus::prelude::*;
use crate::api::list_photos;
use crate::components::{LazyImage, Lightbox};

#[component]
pub fn PhotoGrid(refresh: ReadSignal<u64>) -> Element {
    let mut current_lightbox: Signal<Option<usize>> = use_signal(|| None);
    let photos = use_server_future(move || {
        let _ = refresh();
        list_photos()
    })?;

    match photos() {
        Some(Ok(photos)) => rsx! {
            div { class: "grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-4 p-4",
                if photos.is_empty() {
                    div { class: "col-span-full text-center text-gray-500 py-12",
                        p { class: "text-lg", "Brak zdjęć" }
                        p { class: "text-sm mt-2", "Dodaj zdjęcia, aby zobaczyć je tutaj" }
                    }
                }
                for (index, photo) in photos.iter().enumerate() {
                    div {
                        class: "relative group cursor-pointer overflow-hidden rounded-lg shadow-md hover:shadow-xl transition-shadow",
                        onclick: move |_| {
                            current_lightbox.set(Some(index));
                        },
                        LazyImage {
                            src: format!("/api/photos/{}/thumb", photo.public_id),
                            alt: photo.original_name.clone(),
                            class: "w-full h-48 object-cover".to_string(),
                        }
                        div { class: "absolute bottom-0 left-0 right-0 bg-gradient-to-t from-black/60 to-transparent p-2",
                            p { class: "text-white text-sm truncate", "{photo.original_name}" }
                        }
                    }
                }
            }
            Lightbox {
                photos: photos.clone(),
                current_index: current_lightbox,
            }
        },
        Some(Err(e)) => rsx! {
            div { class: "text-red-500 text-center p-4", "Błąd ładowania zdjęć: {e}" }
        },
        None => rsx! {
            div { class: "text-center p-8 text-gray-400", "Ładowanie zdjęć..." }
        },
    }
}
