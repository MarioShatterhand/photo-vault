# CLAUDE.md — PhotoVault

> **Read `AGENTS.md` first** — it contains the Dioxus 0.7 API reference (RSX, signals, routing,
> server functions, hydration). This file covers project-specific architecture and decisions.

## Project Overview

PhotoVault is a **full-stack photo gallery application written 100% in Rust** — zero JavaScript.
Built with Dioxus 0.7 (fullstack mode) + Axum backend + SQLite, compiled to WASM for the frontend.

This is a learning/portfolio project. The developer (Mariusz) is an experienced fullstack dev
(PHP/Symfony professionally) learning Rust for web. He explicitly chose the harder path — no JS
libraries, no npm, everything hand-built in Rust.

## Tech Stack

- **Framework**: Dioxus 0.7.x (fullstack — single binary serves WASM frontend + Axum backend)
- **Frontend**: RSX! macro → compiled to WASM, styled with Tailwind CSS
- **Backend**: Axum (integrated via Dioxus fullstack), server functions
- **Database**: SQLite via `sqlx` (with FTS5 for full-text search)
- **Image processing**: `image` crate (server-side only — never in WASM)
- **CLI**: `dx serve` for development, `dx build --release` for production
- **IDE**: RustRover

## Architecture

```
src/
├── main.rs              # dioxus::launch, root App component, Route enum
├── models/
│   └── photo.rs         # Photo struct (shared between frontend/backend)
├── components/
│   ├── mod.rs
│   ├── photo_grid.rs    # Thumbnail grid component
│   ├── search_bar.rs    # Debounced search input
│   ├── upload_form.rs   # File upload component
│   └── lightbox.rs      # Fullscreen photo viewer modal
├── views/
│   ├── mod.rs
│   ├── home.rs          # Landing / gallery view
│   ├── upload.rs        # Upload page
│   └── navbar.rs        # Navigation bar
└── server/
    ├── mod.rs
    ├── photos.rs        # Server functions: upload, list, search, serve file
    └── db.rs            # SQLite connection pool, migrations, queries
```

### Key directories

- `assets/` — static assets (CSS, images), processed by dx CLI
- `uploads/` — photo storage (gitignored), created at runtime
- `uploads/thumbs/` — generated thumbnails (300px wide)

## Conventions & Rules

### Rust / Dioxus

- **No JavaScript. Ever.** Browser APIs accessed through `web-sys` / `wasm-bindgen` only.
- Use `#[component]` macro for all UI components.
- Use `use_signal` for local state, `use_server_future` for server-side data fetching (ensures hydration works).
- Use `use_resource` for client-only async operations.
- Server functions use **`#[post("/path")]` / `#[get("/path")]`** macros (NOT `#[server]` — that's pre-0.7).
  They generate API endpoints on server, HTTP calls on client. See AGENTS.md for examples.
- All props must implement `Clone + PartialEq` (Dioxus requirement for memoization).
- Use `ReadOnlySignal<T>` for reactive props that should auto-trigger re-renders in child components.
- Use `Signal<T>` for two-way binding props (e.g. input components).
- Assets use the `asset!()` macro: `img { src: asset!("/assets/logo.png") }`
- RSX! macro for all markup — it looks like JSX but is Rust. Example:
  ```rust
  rsx! {
      div { class: "grid grid-cols-3 gap-4",
          for photo in photos() {
              img { src: "/api/photos/{photo.id}/thumb", alt: "{photo.name}" }
          }
      }
  }
  ```

### File upload approach

Dioxus server functions serialize arguments, so large binary uploads must NOT go through
`#[post]` / `#[get]` functions. Instead:
- Use a dedicated **Axum endpoint** with `axum-multipart` for file uploads
- Register custom Axum routes via Dioxus server config
- Call from WASM using `reqwest` (which compiles to WASM) or `gloo-net`

### Image processing

- **Always server-side.** The `image` crate is too slow in WASM.
- Generate thumbnails (300px width, preserve aspect ratio) on upload.
- Store originals in `uploads/` and thumbnails in `uploads/thumbs/`.
- Use content-hash filenames to avoid collisions: `{sha256_hex}.{ext}`

### Database

- SQLite via `sqlx` with compile-time query checking where possible.
- Migrations in `migrations/` directory, run at startup.
- FTS5 virtual table for search (photo name, tags, EXIF data).

### Styling

- Tailwind CSS only. Dioxus 0.7 has built-in Tailwind support.
- Classes go directly in RSX: `div { class: "flex items-center gap-2", ... }`
- No separate CSS files for components — keep styles inline via Tailwind classes.
- Responsive design: mobile-first, use `sm:`, `md:`, `lg:` breakpoints.

### Error handling

- Use `Result<T, ServerFnError>` for server functions.
- Display user-friendly errors in UI — never show raw Rust errors.
- Log server errors via `tracing` crate.

### Project commands

```bash
dx serve              # Dev server with hot-reload (default: localhost:8080)
dx build --release    # Production build
cargo test            # Run tests
cargo clippy          # Lint
```

## Current Phase: 1 — Scaffold

We are restructuring the dx-generated Jumpstart template into PhotoVault layout.

### Phase 1 goals:
- [x] Project created with dx new (Jumpstart, fullstack, router, tailwind)
- [ ] Remove demo content (hero, blog, echo components)
- [ ] Set up route structure: Home (gallery), Upload, About
- [ ] Create placeholder components: PhotoGrid, SearchBar, UploadForm
- [ ] Navbar with PhotoVault branding and navigation links
- [ ] Add SQLx + SQLite dependency, create db.rs with connection pool
- [ ] Create Photo model (id, filename, original_name, hash, size, created_at)
- [ ] First migration: photos table
- [ ] Verify everything compiles and `dx serve` shows the new layout

### Phase 2 goals (next):
- [ ] File upload via dedicated Axum multipart endpoint
- [ ] Save photo to disk + record in SQLite
- [ ] Thumbnail generation on upload (image crate, 300px)
- [ ] Basic gallery grid showing thumbnails from DB

### Phase 3 goals:
- [ ] Lightbox component (click thumbnail → fullscreen)
- [ ] Lazy loading (IntersectionObserver via web-sys)
- [ ] Photo detail view with metadata

### Phase 4 goals:
- [ ] SQLite FTS5 search
- [ ] Debounced search bar
- [ ] Search results update gallery in real time

### Phase 5 goals:
- [ ] Drag-and-drop upload (DragEvent via web-sys)
- [ ] Upload progress indicator
- [ ] File validation (size, format)
- [ ] Responsive layout polish

### Phase 6 goals (stretch):
- [ ] EXIF extraction (kamadak-exif)
- [ ] Bulk upload
- [ ] Tagging system
- [ ] Sort/filter options
- [ ] Pagination
- [ ] Dark mode

## Dependencies (target Cargo.toml)

The project uses Cargo feature flags to split server/client code (as described in AGENTS.md):

```toml
[features]
default = ["web", "server"]
web = ["dioxus/web"]
server = ["dioxus/server"]
```

Target dependencies:

```toml
dioxus = { version = "0.7", features = ["fullstack", "router"] }
serde = { version = "1", features = ["derive"] }
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite"] }
image = "0.25"
tokio = { version = "1", features = ["full"] }
tracing = "0.1"
sha2 = "0.10"
hex = "0.4"
chrono = { version = "0.4", features = ["serde"] }
reqwest = { version = "0.12", features = ["multipart"] }
axum-extra = { version = "0.10", features = ["multipart"] }

# Only compiled for server target:
# [target.'cfg(not(target_arch = "wasm32"))'.dependencies]
# (sqlx, image, axum-extra go here if needed)
```

## Important Gotchas

1. **WASM vs Server code**: Code inside `#[post]`/`#[get]` server functions only runs on the server.
   Code outside runs on both. Use `#[cfg(feature = "server")]` for server-only modules.
   Never import `sqlx`, `image`, `tokio::fs` etc. in code that compiles to WASM.

2. **Dioxus server functions serialize via serde**: All arguments and return types must be
   `Serialize + Deserialize`. Don't pass large blobs through server functions.

3. **web-sys is verbose**: Every browser API call involves `JsValue` casting and `unwrap()`.
   Wrap repeated patterns in helper functions in a `browser_utils.rs` module.

4. **Thumbnails server-side only**: The `image` crate in WASM is 10x slower and bloats
   the binary. All image processing happens in Axum handlers.

5. **First compile is slow** (~2-5 min). Subsequent compiles with hot-reload are faster.
   Use `dx serve` not `cargo run` during development.

6. **Tailwind in Dioxus**: Classes must be full strings, not dynamically constructed.
   Tailwind's purge won't detect `format!("grid-cols-{n}")`. Use conditional full classes instead:
   `class: if wide { "grid-cols-4" } else { "grid-cols-2" }`
