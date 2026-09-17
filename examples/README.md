# JSON:API Showcase (axum + Leptos)

A full-stack feature tour of `jsonapi_core` / `jsonapi_axum`:

- **Backend** — an axum JSON:API server backed by an embedded SQLite database (no external
  services). Exercises compound documents, sparse fieldsets, sort/filter, three pagination
  strategies, `Field<T>` partial updates, relationship endpoints, atomic operations, content-type
  negotiation, and per-request tracing IDs.
- **Frontend** — a Leptos CSR single-page app that drives every endpoint and captures the raw
  `application/vnd.api+json` exchange in a **persistent, self-captured inspector panel** (method,
  URL, request/response headers, pretty-printed body, status, duration, `X-Request-Id`). The
  inspector is the star: every demo action fires a real cross-origin HTTP call you can inspect.

The two run as **separate origins** (backend `:8080`, frontend `:8081`), so the browser's network
tab and the in-app inspector show real JSON:API traffic over real CORS.

```
examples/
  resources/   shared #[derive(JsonApi)] wire types (used by backend AND the WASM frontend)
  backend/     axum + sqlx (SQLite) JSON:API server        -> http://127.0.0.1:8080
  frontend/    Leptos 0.8 CSR SPA + live inspector (Trunk)  -> http://127.0.0.1:8081
  docker/      Dockerfiles + compose for both services
  justfile     run / test / lint targets
```

---

## Run it locally

### Prerequisites

- **Rust** (edition 2024, `rustc` ≥ 1.94) — https://rustup.rs
- For the **frontend** only, add the WASM target and the build tools:

  ```sh
  rustup target add wasm32-unknown-unknown
  cargo install trunk --locked wasm-bindgen-cli
  ```

  > `--locked` on `trunk` is required — without it a transitive dependency currently fails to
  > compile.

- Optional: [`just`](https://github.com/casey/just) for the shorthand commands below. Every `just`
  target has a raw-command equivalent, shown after each step.

All commands below are run from the **`examples/`** directory unless noted.

### 1. Start the backend (terminal 1)

```sh
just run-backend
# raw equivalent:
DATABASE_URL="sqlite://showcase.db?mode=rwc" cargo run --manifest-path backend/Cargo.toml
```

It listens on `http://127.0.0.1:8080`, creates `showcase.db`, and seeds it deterministically on
first run (idempotent — safe to restart). Health check: `curl http://127.0.0.1:8080/health`.

### 2. Start the frontend (terminal 2)

```sh
just run-frontend
# raw equivalent:
cd frontend && trunk serve
```

Trunk compiles the WASM bundle and serves it on `http://127.0.0.1:8081` with hot reload.

### 3. Open the app

Open **http://127.0.0.1:8081** in your browser. Pick a feature from the left nav, click a demo
action, and watch the exact request/response appear in the inspector on the right. Click any
inspector row to expand its full detail.

> Both `http://127.0.0.1:8081` and `http://localhost:8081` work out of the box (both are in the
> backend's default CORS allow-list). See [Troubleshooting](#troubleshooting) if a write action
> shows `Failed to fetch`.

---

## Using the showcase

Each page maps to one crate feature and fires the real call(s) behind it:

| Page | What it exercises |
|---|---|
| **Read** | Fetch one resource by ID; and a typed list parsed via `jsonapi_core::Document::parse_many` |
| **Pagination** | Page-number (`/articles`), offset (`/articles/offset`), cursor (`/articles/cursor`) — compare the `links` object per strategy |
| **Includes** | Transitive compound documents (`?include=author,comments.author,tags`) |
| **Sparse fieldsets** | `?fields[articles]=…&fields[authors]=…` shrinking the payload |
| **Sort & filter** | `?sort=-created_at,title` and `?filter[author]=…` |
| **Create / edit** | Typed `NewArticleResource` create, and a `Field<T>` PATCH — unchecked fields are *absent* on the wire (true partial update) |
| **Relationships** | To-one author (GET/PATCH) and to-many tags (GET/POST/DELETE) |
| **Errors** | Trigger `415` / `406` / `404` / `422` (multi-error aggregation) and inspect the `errors` array + pointers |
| **Atomic ops** | `POST /operations` with two ops and an `lid` cross-reference, plus a client-side `validate_lid_refs` pre-flight |

---

## Run with Docker

Builds and serves **both** services (backend on `:8080`, frontend on `:8081`). The build context is
the **repo root** because the example path-depends on the parent crates:

```sh
# from the repo root
docker compose -f examples/docker/compose.yml up
```

---

## Configuration

### Backend (environment variables)

| Variable | Default | Description |
|---|---|---|
| `APP_BIND` | `127.0.0.1:8080` | Listen address |
| `DATABASE_URL` | `sqlite://showcase.db?mode=rwc` | SQLite connection string |
| `APP_BASE_URL` | `http://<APP_BIND>` | Base URL used for pagination `links` in responses |
| `APP_CORS_ORIGINS` | `http://127.0.0.1:8081,http://localhost:8081` | Comma-separated allowed CORS origins |
| `APP_SEED` | `true` | Set to `false`/`0` to skip seeding on startup |

### Frontend (backend base URL)

Resolved in this order (see `frontend/src/config.rs`):

1. `window.__SHOWCASE_CONFIG__.apiBase` — a runtime override. There is a commented-out example in
   `frontend/index.html`; uncomment and edit it to retarget a prebuilt bundle without rebuilding.
2. The `API_BASE` environment variable **baked at `trunk build` time**.
3. The default `http://127.0.0.1:8080`.

---

## Backend reference

### Endpoints

| Method | Path | What it showcases |
|---|---|---|
| `GET` | `/health` | Meta-only `Document`, content-type negotiation |
| `GET` | `/articles` | Page-number pagination, includes, sparse fieldsets, sort, filter |
| `GET` | `/articles/offset` | Offset/limit pagination strategy |
| `GET` | `/articles/cursor` | Cursor-based pagination (opaque stable cursor) |
| `POST` | `/articles` | Resource creation, client-supplied or server-generated ID |
| `GET` | `/articles/{id}` | Single resource fetch, compound document with `?include=` |
| `PATCH` | `/articles/{id}` | Partial update via `Field<T>` (absent fields left unchanged) |
| `DELETE` | `/articles/{id}` | Resource deletion |
| `GET` | `/articles/{id}/relationships/tags` | To-many relationship document |
| `PATCH` | `/articles/{id}/relationships/tags` | Replace full tag set |
| `POST` | `/articles/{id}/relationships/tags` | Append tags |
| `DELETE` | `/articles/{id}/relationships/tags` | Remove tags |
| `GET` | `/articles/{id}/relationships/author` | To-one relationship document |
| `PATCH` | `/articles/{id}/relationships/author` | Replace author |
| `POST` | `/operations` | Atomic operations extension (bulk create/update/delete) |

Every response carries an `X-Request-Id` header (via `RequestIdLayer`).

### Seed data

On first startup (when `APP_SEED=true`) the database is populated deterministically:

- 2 authors: `a1`, `a2`
- 12 articles: `art-01` … `art-12`, split between the two authors
- 3 tags (`t-rust`, `t-web`, `t-api`) and comments (`c1`, `c2`) associated with the articles

The seed is idempotent — restarting against an existing database does not duplicate records.

### Member-name validation

JSON:API member names (resource types, attribute keys, relationship names) are validated at two
points:

- **Compile time** — the `#[derive(JsonApi)]` macro calls `validate_member_name` during expansion.
  Every `#[jsonapi(type = "...")]` and every renamed field is checked before the binary is produced;
  an invalid name (e.g. leading/trailing hyphen) is a compile error.
- **Runtime** — client-supplied names in request bodies are validated before any database work. In
  `POST /operations`, each `add`/`update` operation's `type` is validated and rejected with
  `400 Bad Request` when malformed — distinct from the `422 Unprocessable Entity` returned for a
  syntactically valid but unsupported type (e.g. `"widgets"`).

---

## Testing

```sh
# backend acceptance tests (in-memory db, no external services)
just test                # cargo test -p jsonapi-showcase-backend

# frontend: native unit tests + wasm smoke tests (under node, no browser)
just test-frontend

# everything green: backend + frontend tests, trunk build, fmt, clippy (both targets)
just check-all

# lint / format
just lint                # clippy -D warnings + rustfmt --check
just fmt
```

The frontend wasm tests run via `wasm-bindgen-test-runner` under Node (no browser or `wasm-pack`
needed) and cover the inspector store and the typed `Field<T>` PATCH serialization.

---

## Troubleshooting

**A write action (`POST`/`PATCH`/`DELETE`) shows `network-error: Failed to fetch` / a CORS error.**
Writes are preflighted (they carry the `application/vnd.api+json` content-type), so CORS must be
correct end-to-end. Checklist:

- **Origin mismatch.** The SPA's origin must be in `APP_CORS_ORIGINS`. The default allows both
  `http://127.0.0.1:8081` and `http://localhost:8081` — browsers treat those as *distinct* origins.
  If you serve the SPA elsewhere, add that exact origin (scheme + host + port).
- **Stale preflight cache.** If you hit a CORS error *before* fixing the origin/config, the browser
  may cache the failed preflight. A normal hard-refresh does **not** clear it — open a
  **private/incognito window**, or open DevTools → **Network → "Disable cache"** and reload.
- **Backend not running / wrong URL.** Confirm `curl http://127.0.0.1:8080/health` returns `200` and
  that the frontend's resolved API base (see [Configuration](#frontend-backend-base-url)) points at
  it.

**Deploying behind a browser?** When combining the `jsonapi_axum` `JsonApiLayer` (the 415/406
content-negotiation guard) with a CORS layer, apply **CORS as the outermost layer**. Otherwise a
guard rejection is produced *before* the CORS layer runs and the response lacks
`Access-Control-Allow-Origin`, so the browser blocks it — even though handler-produced errors
(404/422) work. See `backend/src/router.rs` for the correct ordering.

**`trunk` fails to compile.** Install it with `cargo install trunk --locked wasm-bindgen-cli`.
