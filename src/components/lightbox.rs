use dioxus::prelude::*;
use crate::models::Photo;

fn format_size(bytes: i64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

#[component]
pub fn Lightbox(
    photos: Vec<Photo>,
    current_index: Signal<Option<usize>>,
) -> Element {
    let index = match current_index() {
        Some(i) => i,
        None => return rsx! {},
    };

    let total = photos.len();
    let photo = photos[index].clone();

    let mut confirm_delete = use_signal(|| false);
    let deleting = use_signal(|| false);
    let mut delete_error = use_signal(|| None::<String>);

    let go_prev = move |_| {
        if index > 0 {
            current_index.set(Some(index - 1));
            confirm_delete.set(false);
            delete_error.set(None);
        }
    };

    let go_next = move |_| {
        if index + 1 < total {
            current_index.set(Some(index + 1));
            confirm_delete.set(false);
            delete_error.set(None);
        }
    };

    let close = move |_| {
        current_index.set(None);
        confirm_delete.set(false);
    };

    let on_keydown = move |evt: KeyboardEvent| {
        match evt.key() {
            Key::ArrowLeft => {
                if index > 0 {
                    current_index.set(Some(index - 1));
                    confirm_delete.set(false);
                    delete_error.set(None);
                }
            }
            Key::ArrowRight => {
                if index + 1 < total {
                    current_index.set(Some(index + 1));
                    confirm_delete.set(false);
                    delete_error.set(None);
                }
            }
            Key::Escape => {
                if confirm_delete() {
                    confirm_delete.set(false);
                } else {
                    current_index.set(None);
                }
            }
            _ => {}
        }
    };

    let photo_name = photo.original_name.clone();
    let photo_size = format_size(photo.size);
    let photo_date = photo.created_at.clone();
    let photo_src = format!("/api/photos/{}/full", photo.public_id);
    let delete_public_id = photo.public_id.clone();

    rsx! {
        div {
            class: "fixed inset-0 z-50 bg-black/90 flex items-center justify-center",
            tabindex: "0",
            onkeydown: on_keydown,
            onclick: move |_| {
                current_index.set(None);
                confirm_delete.set(false);
            },
            onmounted: move |evt| async move {
                let _ = evt.data().set_focus(true).await;
            },

            // Close button (top-right of overlay)
            button {
                class: "absolute top-4 right-4 text-white text-3xl bg-black/50 hover:bg-black/70 rounded-full w-10 h-10 flex items-center justify-center cursor-pointer z-10",
                onclick: close,
                "×"
            }

            // Image area
            div {
                class: "relative flex-1 flex items-center justify-center h-full",
                onclick: move |evt| evt.stop_propagation(),

                // Left nav button
                button {
                    class: if index > 0 { "absolute top-1/2 -translate-y-1/2 left-4 bg-black/50 hover:bg-black/70 text-white rounded-full w-12 h-12 flex items-center justify-center text-2xl transition-colors" } else { "absolute top-1/2 -translate-y-1/2 left-4 bg-black/30 text-white/30 rounded-full w-12 h-12 flex items-center justify-center text-2xl cursor-default" },
                    onclick: go_prev,
                    "‹"
                }

                // Photo
                img {
                    class: "max-h-[90vh] max-w-full object-contain",
                    src: "{photo_src}",
                    alt: "{photo_name}",
                }

                // Right nav button
                button {
                    class: if index + 1 < total { "absolute top-1/2 -translate-y-1/2 right-4 bg-black/50 hover:bg-black/70 text-white rounded-full w-12 h-12 flex items-center justify-center text-2xl transition-colors" } else { "absolute top-1/2 -translate-y-1/2 right-4 bg-black/30 text-white/30 rounded-full w-12 h-12 flex items-center justify-center text-2xl cursor-default" },
                    onclick: go_next,
                    "›"
                }
            }

            // Metadata panel (hidden on mobile)
            div {
                class: "hidden md:flex flex-col w-80 bg-slate-800 text-white p-6 h-full overflow-y-auto",
                onclick: move |evt| evt.stop_propagation(),

                h2 {
                    class: "text-lg font-semibold truncate mb-4",
                    "{photo_name}"
                }

                div {
                    class: "space-y-3 text-sm text-slate-300",

                    div {
                        span { class: "text-slate-400 block", "File size" }
                        span { "{photo_size}" }
                    }

                    div {
                        span { class: "text-slate-400 block", "Uploaded" }
                        span { "{photo_date}" }
                    }

                    if photo.width > 0 && photo.height > 0 {
                        div {
                            span { class: "text-slate-400 block", "Dimensions" }
                            span { "{photo.width} × {photo.height}" }
                        }
                    }

                    div {
                        span { class: "text-slate-400 block", "Index" }
                        span { "Photo {index + 1} of {total}" }
                    }
                }

                // Delete section — at the bottom of the panel
                div {
                    class: "mt-auto pt-6 border-t border-slate-700",

                    if confirm_delete() {
                        // Confirmation state
                        div {
                            class: "space-y-3",
                            p { class: "text-red-400 text-sm font-medium", "Usunąć to zdjęcie?" }
                            if let Some(err) = delete_error() {
                                p { class: "text-red-300 text-xs", "{err}" }
                            }
                            div {
                                class: "flex gap-2",
                                button {
                                    class: "flex-1 px-3 py-2 bg-red-600 hover:bg-red-700 text-white text-sm rounded transition-colors disabled:opacity-50",
                                    disabled: deleting(),
                                    onclick: {
                                        let delete_public_id = delete_public_id.clone();
                                        move |evt: MouseEvent| {
                                            evt.stop_propagation();
                                            delete_error.set(None);
                                            #[allow(unused_variables)]
                                            let public_id = delete_public_id.clone();
                                            #[allow(unused_variables, unused_mut)]
                                            let mut deleting = deleting;
                                            #[allow(unused_variables, unused_mut)]
                                            let mut current_index = current_index;
                                            #[allow(unused_variables, unused_mut)]
                                            let mut confirm_delete = confirm_delete;
                                            spawn(async move {
                                                deleting.set(true);
                                                // Call DELETE endpoint
                                                #[cfg(target_arch = "wasm32")]
                                                {
                                                    let resp = gloo_net::http::Request::delete(&format!("/api/photos/{}", public_id))
                                                        .send()
                                                        .await;
                                                    match resp {
                                                        Ok(r) if r.ok() => {
                                                            // Trigger gallery refresh
                                                            if let Some(mut refresh) = try_consume_context::<Signal<u64>>() {
                                                                refresh += 1;
                                                            }
                                                            current_index.set(None);
                                                        }
                                                        _ => {
                                                            deleting.set(false);
                                                            delete_error.set(Some("Nie udało się usunąć zdjęcia".to_string()));
                                                        }
                                                    }
                                                }
                                            });
                                        }
                                    },
                                    if deleting() { "Usuwanie..." } else { "Tak, usuń" }
                                }
                                button {
                                    class: "flex-1 px-3 py-2 bg-slate-600 hover:bg-slate-500 text-white text-sm rounded transition-colors",
                                    onclick: move |evt: MouseEvent| {
                                        evt.stop_propagation();
                                        confirm_delete.set(false);
                                    },
                                    "Anuluj"
                                }
                            }
                        }
                    } else {
                        // Normal state — show delete button
                        button {
                            class: "w-full px-3 py-2 bg-red-600/20 hover:bg-red-600/40 text-red-400 hover:text-red-300 text-sm rounded transition-colors",
                            onclick: move |evt: MouseEvent| {
                                evt.stop_propagation();
                                confirm_delete.set(true);
                            },
                            "Usuń zdjęcie"
                        }
                    }
                }
            }
        }
    }
}
