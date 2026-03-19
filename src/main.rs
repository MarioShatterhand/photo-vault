use dioxus::prelude::*;

use views::{About, Home, Login, Navbar, Passkeys, Register, Upload};

mod api;
mod components;
mod models;
mod views;

#[cfg(feature = "server")]
mod server;

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[route("/login")]
    Login {},
    #[route("/register")]
    Register {},
    #[layout(Navbar)]
        #[route("/")]
        Home {},
        #[route("/upload")]
        Upload {},
        #[route("/passkeys")]
        Passkeys {},
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

            // Protected routes (require auth)
            let protected_routes = axum::Router::new()
                .route("/api/upload", axum::routing::post(server::photos::upload_photo)
                    .layer(axum::extract::DefaultBodyLimit::max(20 * 1024 * 1024)))
                .route("/api/photos/{public_id}", axum::routing::delete(server::photos::delete_photo))
                .route("/api/photos/{public_id}/thumb", axum::routing::get(server::photos::serve_thumbnail))
                .route("/api/photos/{public_id}/full", axum::routing::get(server::photos::serve_full))
                .route("/api/auth/passkeys", axum::routing::get(server::auth::list_passkeys))
                .route("/api/auth/passkeys/add/start", axum::routing::post(server::auth::add_passkey_start))
                .route("/api/auth/passkeys/add/finish", axum::routing::post(server::auth::add_passkey_finish))
                .route("/api/auth/passkeys/{id}/name", axum::routing::put(server::auth::rename_passkey))
                .route("/api/auth/passkeys/{id}", axum::routing::delete(server::auth::delete_passkey))
                .layer(axum::middleware::from_fn(server::session::auth_middleware));

            let router = dioxus::server::router(App)
                // Public auth routes
                .route("/api/auth/status", axum::routing::get(server::session::auth_status))
                .route("/api/auth/register/start", axum::routing::post(server::auth::register_start))
                .route("/api/auth/register/finish", axum::routing::post(server::auth::register_finish))
                .route("/api/auth/login/start", axum::routing::post(server::auth::login_start))
                .route("/api/auth/login/finish", axum::routing::post(server::auth::login_finish))
                .route("/api/auth/logout", axum::routing::post(server::auth::logout))
                // Protected routes
                .merge(protected_routes);

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
