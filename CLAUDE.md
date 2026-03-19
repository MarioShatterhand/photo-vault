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
- **Auth**: WebAuthn/Passkeys via `webauthn-rs` (passkey-only, zero passwords)
- **Image processing**: `image` crate (server-side only — never in WASM)
- **CLI**: `dx serve` for development, `dx build --release` for production
- **IDE**: RustRover

## Architecture

```
src/
├── main.rs              # dioxus::launch, root App component, Route enum, Axum route wiring
├── api.rs               # Shared server functions (list_photos)
├── models/
│   └── photo.rs         # Photo struct (id, public_id, filename, hash, size, width, height, created_at)
├── components/
│   ├── mod.rs
│   ├── photo_grid.rs    # Thumbnail grid with LazyImage + Lightbox integration
│   ├── search_bar.rs    # Debounced search input
│   ├── upload_form.rs   # File upload component (web_sys FormData + gloo-net)
│   ├── lightbox.rs      # Fullscreen photo viewer with metadata panel + delete
│   ├── lazy_image.rs    # IntersectionObserver lazy loading wrapper
│   ├── login_form.rs    # Passkey login (credentials.get via document::eval JS interop)
│   ├── register_form.rs # Passkey registration (credentials.create via document::eval JS interop)
│   └── passkey_card.rs  # Single passkey display card (name, dates, rename/delete)
├── views/
│   ├── mod.rs
│   ├── home.rs          # Landing / gallery view
│   ├── upload.rs        # Upload page
│   ├── navbar.rs        # Navigation bar + auth guard (redirects unauthenticated users)
│   ├── login.rs         # Login page (outside Navbar layout)
│   ├── register.rs      # Register page (outside Navbar layout, first-register-wins)
│   └── passkeys.rs      # Passkey management (list, add, rename, delete)
└── server/
    ├── mod.rs
    ├── photos.rs        # Photo CRUD: upload, list, serve thumb/full, delete
    ├── db.rs            # SQLite connection pool, migrations
    ├── auth.rs          # WebAuthn handlers: register, login, logout, passkey management
    └── session.rs       # Session middleware, cookie management, auth_status endpoint
```

### Key directories

- `assets/` — static assets (CSS, images), processed by dx CLI
- `uploads/` — photo storage (gitignored), created at runtime
- `uploads/thumbs/` — generated thumbnails (300px wide)
- `migrations/` — SQLite migrations (photos, public_id, dimensions, users, credentials, sessions, webauthn_challenges)

## Authentication (Phase 1.5)

- **Single-user, first-register-wins**: First user to register owns the app. No open registration after that.
- **Passkey-only, zero fallback**: No passwords. Recovery = register 2+ passkeys.
- **Server-side sessions in SQLite**: `sessions` table with token_hash. Cookie: `photovault_session`.
- **WebAuthn via `webauthn-rs` 0.5**: `rp_id = "localhost"`, `rp_origin = "http://localhost:8080"`.
- **Auth guard in Navbar**: Calls `GET /api/auth/status` on mount. Redirects to `/register` (no users) or `/login` (no session).
- **Protected API routes**: Upload, delete, passkey management require valid session (Axum middleware).
- **Public routes**: `/login`, `/register`, `/api/auth/status`, `/api/auth/login/*`, `/api/auth/register/*`, photo serving (thumb/full).
- **WebAuthn JS interop**: Uses `document::eval()` with inline JS for `navigator.credentials.create/get` (base64url↔ArrayBuffer conversions).
- **Important**: WebAuthn requires `localhost` (not `127.0.0.1`) — "insecure protocol" error otherwise.

## Conventions & Rules

### Rust / Dioxus

- **No JavaScript. Ever.** Browser APIs accessed through `web-sys` / `wasm-bindgen` only.
  Exception: `document::eval()` for WebAuthn ceremonies (base64url↔ArrayBuffer conversion is impractical in pure web-sys).
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
- 7 migrations: photos, public_id, dimensions, users, credentials, sessions, webauthn_challenges.
- FTS5 virtual table for search (photo name, tags, EXIF data).

### Styling

- Tailwind CSS only. Dioxus 0.7 has built-in Tailwind support.
- Classes go directly in RSX: `div { class: "flex items-center gap-2", ... }`
- No separate CSS files for components — keep styles inline via Tailwind classes.
- Responsive design: mobile-first, use `sm:`, `md:`, `lg:` breakpoints.
- Dark theme: slate-950 background, slate-800 cards, blue-600 accent.

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

## Current Phase: 4 — Search

Phases 1-3 and 1.5 complete. Now building search functionality.

### Phase 1 goals (DONE):
- [x] Project scaffold, route structure, Navbar, SQLite setup, Photo model

### Phase 2 goals (DONE):
- [x] File upload (Axum multipart), thumbnails, gallery grid, file serving, dedup

### Phase 3 goals (DONE):
- [x] Lightbox, lazy loading, metadata, delete, keyboard navigation

### Phase 1.5 goals (DONE — Authentication, WebAuthn/Passkeys):
- [x] `webauthn-rs` 0.5 dependency (server-side, `danger-allow-state-serialisation`)
- [x] DB migrations: users, credentials, sessions, webauthn_challenges tables
- [x] Registration flow (first-register-wins, navigator.credentials.create via eval)
- [x] Login flow (navigator.credentials.get via eval, session cookie)
- [x] Session management (cookie-based, SHA-256 hashed tokens, 30-day expiry)
- [x] Axum auth middleware on protected routes (upload, delete, passkey management)
- [x] Client-side auth guard in Navbar (redirect to /register or /login)
- [x] Logout (POST /api/auth/logout destroys session + redirect)
- [x] Passkey management page (list, add, rename, delete with last-passkey guard)

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

## API Endpoints

### Public (no auth)
| Method | Path | Purpose |
|--------|------|---------|
| GET | /api/auth/status | Check setup + auth state |
| POST | /api/auth/register/start | Start registration ceremony |
| POST | /api/auth/register/finish | Complete registration |
| POST | /api/auth/login/start | Start login ceremony |
| POST | /api/auth/login/finish | Complete login |

### Protected (require session)
| Method | Path | Purpose |
|--------|------|---------|
| POST | /api/upload | Upload photo |
| DELETE | /api/photos/{id} | Delete photo |
| GET | /api/photos/{id}/thumb | Serve thumbnail |
| GET | /api/photos/{id}/full | Serve full image |
| POST | /api/auth/logout | Destroy session |
| GET | /api/auth/passkeys | List user's passkeys |
| POST | /api/auth/passkeys/add/start | Start adding passkey |
| POST | /api/auth/passkeys/add/finish | Finish adding passkey |
| PUT | /api/auth/passkeys/{id}/name | Rename passkey |
| DELETE | /api/auth/passkeys/{id} | Delete passkey (blocks last) |

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

7. **WebAuthn requires localhost**: Use `http://localhost:8080`, not `127.0.0.1`. The latter
   triggers "insecure protocol" errors from passkey providers.

8. **WASM-side HTTP calls**: Use `gloo_net::http::Request` (gloo-net crate). Wrap in
   `#[cfg(target_arch = "wasm32")]` blocks since gloo-net is only available on wasm32.

9. **Shared deps**: `serde_json` is a non-optional dependency (used by both server and WASM).
   Don't make it feature-gated.
