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
├── api.rs               # (unused — photo listing moved to server/photos.rs as Axum handler)
├── models/
│   └── photo.rs         # Photo struct (id, public_id, filename, hash, size, width, height, created_at)
├── components/
│   ├── mod.rs
│   ├── photo_grid.rs    # Thumbnail grid with LazyImage + Lightbox, fetches via gloo-net GET /api/photos?q=
│   ├── search_bar.rs    # Debounced search input (300ms, gloo-timers, clear button)
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
    ├── photos.rs        # Photo CRUD: upload, list (FTS5 search), serve thumb/full, delete
    ├── db.rs            # SQLite connection pool, migrations
    ├── auth.rs          # WebAuthn handlers: register, login, logout, passkey management
    └── session.rs       # Session middleware, cookie management, auth_status endpoint
```

### Key directories

- `assets/` — static assets (CSS, images), processed by dx CLI
- `uploads/{user_id}/` — original photos per user (gitignored), created on demand at upload
- `uploads/{user_id}/thumbs/` — generated thumbnails (300px wide), per user
- `migrations/` — SQLite migrations (photos, public_id, dimensions, users, credentials, sessions, webauthn_challenges, photos_fts, photos_user_scope)

## Authentication & Multi-User (Phase 1.5 + 1.6)

- **Multi-user, open registration**: Anyone can register at `/register`. No admin role; all users equal.
- **Passkey-only, zero fallback**: No passwords. Recovery = register 2+ passkeys per account.
- **Login is `username + passkey`**: User types username, server returns only that user's allowed credentials, browser surfaces matching passkey. Generic 400 for unknown user (no enumeration).
- **Per-user data isolation**: Every photo row carries `user_id NOT NULL`; every photo handler scopes by `user_id`. Files live in `uploads/{user_id}/`. Cross-user URL access returns 404.
- **Per-user dedup**: `UNIQUE(user_id, hash)` — same user uploading same file is a no-op; different users uploading the same bytes get independent rows + files (privacy-preserving, no cross-user leak via existence checks).
- **Server-side sessions in SQLite**: `sessions` table with SHA-256-hashed token. Cookie: `photovault_session`.
- **WebAuthn via `webauthn-rs` 0.5**: configured from `.env` — `WEBAUTHN_RP_ID`, `WEBAUTHN_RP_ORIGIN`, `WEBAUTHN_RP_NAME` (defaults: `localhost`, `http://localhost:8080`, `PhotoVault`).
- **Auth guard in Navbar**: Calls `GET /api/auth/status` on mount; redirects to `/login` if no session. Login page links to `/register`.
- **Protected API routes**: Upload, list, serve thumb/full, delete, all passkey management — Axum middleware injects `user_id` Extension.
- **Public routes**: `/login`, `/register`, `/api/auth/status`, `/api/auth/login/*`, `/api/auth/register/*`. **Photo listing/serving is NOT public** anymore.
- **WebAuthn JS interop**: Uses `document::eval()` with inline JS for `navigator.credentials.create/get` (base64url↔ArrayBuffer conversions).
- **Important**: WebAuthn requires `localhost` (not `127.0.0.1`) in dev — "insecure protocol" error otherwise. In prod, `WEBAUTHN_RP_ORIGIN` must be `https://...`.

### Multi-user migration history (Phase 1.6)
- DB migration `20240109_photos_user_scope.sql` recreates `photos` with `user_id NOT NULL`, `UNIQUE(user_id, hash)`, and rebuilds FTS5 + triggers. Backfills existing photos to `MIN(users.id)`.
- One-time, idempotent file relocator in `db.rs` moves legacy `uploads/{filename}` → `uploads/{user_id}/{filename}` (and same for thumbs) at startup. No-op on fresh installs and after first successful relocation.

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
- Use `ReadSignal<T>` for reactive props that should auto-trigger re-renders in child components (note: `ReadOnlySignal` is deprecated in 0.7.3).
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
- Store originals in `uploads/{user_id}/` and thumbnails in `uploads/{user_id}/thumbs/`.
- Use content-hash filenames to avoid collisions: `{sha256_hex}.{ext}`. Per-user dirs are created lazily by `upload_photo`.

### Database

- SQLite via `sqlx` with compile-time query checking where possible.
- Migrations in `migrations/` directory, run at startup.
- 9 migrations: photos, public_id, dimensions, users, credentials, sessions, webauthn_challenges, photos_fts, photos_user_scope.
- FTS5 virtual table for search (photo name, tags, EXIF data); search queries are `JOIN`ed with `photos.user_id = ?` for per-user scoping.

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

## Current Phase: 1.6 — Multi-user

Phases 1-4, 1.5, and 1.6 complete.

### Phase 1 goals (DONE):
- [x] Project scaffold, route structure, Navbar, SQLite setup, Photo model

### Phase 2 goals (DONE):
- [x] File upload (Axum multipart), thumbnails, gallery grid, file serving, dedup

### Phase 3 goals (DONE):
- [x] Lightbox, lazy loading, metadata, delete, keyboard navigation

### Phase 1.5 goals (DONE — Authentication, WebAuthn/Passkeys):
- [x] `webauthn-rs` 0.5 dependency (server-side, `danger-allow-state-serialisation`)
- [x] DB migrations: users, credentials, sessions, webauthn_challenges tables
- [x] Registration + login flow (navigator.credentials.create/get via eval, session cookie)
- [x] Session management (cookie-based, SHA-256 hashed tokens, 30-day expiry)
- [x] Axum auth middleware on protected routes
- [x] Client-side auth guard in Navbar
- [x] Logout (POST /api/auth/logout destroys session + redirect)
- [x] Passkey management page (list, add, rename, delete with last-passkey guard)

### Phase 1.6 goals (DONE — Multi-user):
- [x] DB: `photos.user_id NOT NULL` + `UNIQUE(user_id, hash)`, FTS5 rebuilt, backfill for legacy data
- [x] Per-user file storage (`uploads/{user_id}/...`) + idempotent legacy file relocator at startup
- [x] Auth: open registration (no first-register-wins), `username + passkey` login, anti-enumeration generic error
- [x] Photos backend: every handler scoped by `user_id`, all photo endpoints behind auth middleware
- [x] `.env` config for WebAuthn (`WEBAUTHN_RP_ID/ORIGIN/NAME`) via `dotenvy`, `.env.example` checked in
- [x] Frontend: username input on login, cross-links Login ⇄ Register, Navbar guard simplified

### Phase 4 goals (DONE — Search):
- [x] SQLite FTS5 search (virtual table + triggers for insert/delete/update sync)
- [x] Debounced search bar (300ms, gloo-timers, clear button)
- [x] Search results update gallery in real time (use_server_future reactive on query signal)

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
| GET | /api/auth/status | Report whether the request has a valid session |
| POST | /api/auth/register/start | Start registration ceremony (`{username}`) |
| POST | /api/auth/register/finish | Complete registration |
| POST | /api/auth/login/start | Start login ceremony (`{username}` — server returns only that user's allowed credentials) |
| POST | /api/auth/login/finish | Complete login |

### Protected (require session — `user_id` injected into Axum extensions)
| Method | Path | Purpose |
|--------|------|---------|
| GET | /api/photos?q= | List/search current user's photos (FTS5 prefix match, scoped) |
| POST | /api/upload | Upload photo to current user's library |
| DELETE | /api/photos/{public_id} | Delete (only the current user's photo) |
| GET | /api/photos/{public_id}/thumb | Serve thumbnail (404 if not owned by current user) |
| GET | /api/photos/{public_id}/full | Serve full image (404 if not owned by current user) |
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
