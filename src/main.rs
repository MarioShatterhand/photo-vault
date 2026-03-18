use dioxus::prelude::*;

use views::{About, Home, Navbar, Upload};

mod api;
mod components;
mod models;
mod views;

#[cfg(feature = "server")]
mod server;

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[layout(Navbar)]
        #[route("/")]
        Home {},
        #[route("/upload")]
        Upload {},
        #[route("/about")]
        About {},
}

const FAVICON: Asset = asset!("/assets/favicon.ico");
const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");

fn main() {
    #[cfg(not(feature = "server"))]
    dioxus::launch(App);

    #[cfg(feature = "server")]
    {
        use tracing_subscriber::fmt;
        fmt::init();

        dioxus::serve(|| async move {
            // Force DB initialization on startup
            let _ = &*server::db::DB;
            let router = dioxus::server::router(App)
                .route("/api/upload", axum::routing::post(server::photos::upload_photo)
                    .layer(axum::extract::DefaultBodyLimit::max(20 * 1024 * 1024)))
                .route("/api/photos/{public_id}/thumb", axum::routing::get(server::photos::serve_thumbnail))
                .route("/api/photos/{public_id}/full", axum::routing::get(server::photos::serve_full));
            Ok(router)
        });
    }
}

#[component]
fn App() -> Element {
    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: TAILWIND_CSS }
        Router::<Route> {}
    }
}
