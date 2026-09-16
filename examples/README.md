# JSON:API Showcase Backend

An axum-based JSON:API server backed by an embedded SQLite database. It exercises the full
`jsonapi_core` / `jsonapi_axum` stack: compound documents, sparse fieldsets, sort/filter,
three pagination strategies, `Field<T>`-aware partial updates, relationship endpoints, atomic
operations, content-type negotiation, and per-request tracing IDs. A Leptos frontend is planned
and will be added to the `examples/` workspace alongside this crate.

## Running

```sh
# from the examples/ directory

# start the backend (SQLite db is created and seeded on first run)
just run-backend

# run all tests (in-memory db, no external services)
just test

# lint: clippy -D warnings + rustfmt check
just lint

# reformat
just fmt
```

Without `just` installed, the equivalent raw commands are:

```sh
DATABASE_URL="sqlite://showcase.db?mode=rwc" cargo run --manifest-path backend/Cargo.toml
cargo test -p jsonapi-showcase-backend
cargo clippy -p jsonapi-showcase-backend --all-targets -- -D warnings
cargo fmt --check
```

## Docker

Build context is the **repo root** (the crate path-depends on parent crates):

```sh
# from the repo root
docker compose -f examples/docker/compose.yml up
```

## Environment variables

| Variable | Default | Description |
|---|---|---|
| `APP_BIND` | `127.0.0.1:8080` | Listen address |
| `DATABASE_URL` | `sqlite://showcase.db?mode=rwc` | SQLite connection string |
| `APP_BASE_URL` | `http://<APP_BIND>` | Base URL for pagination `links` in responses |
| `APP_CORS_ORIGINS` | `http://127.0.0.1:8081` | Comma-separated allowed CORS origins |
| `APP_SEED` | `true` | Set to `false` or `0` to skip seeding on startup |

## Endpoints

| Method | Path | What it showcases |
|---|---|---|
| `GET` | `/health` | Meta-only `Document`, content-type negotiation |
| `GET` | `/articles` | Page-number pagination, includes, sparse fieldsets, sort, filter |
| `GET` | `/articles/offset` | Offset/limit pagination strategy |
| `GET` | `/articles/cursor` | Cursor-based pagination (opaque stable cursor) |
| `POST` | `/articles` | Resource creation, client-supplied or server-generated ID |
| `GET` | `/articles/{id}` | Single resource fetch, compound document with `?include=` |
| `PATCH` | `/articles/{id}` | Partial update via `Field<T>` (absent fields are left unchanged) |
| `DELETE` | `/articles/{id}` | Resource deletion |
| `GET` | `/articles/{id}/relationships/tags` | To-many relationship document |
| `PATCH` | `/articles/{id}/relationships/tags` | Replace full tag set |
| `POST` | `/articles/{id}/relationships/tags` | Append tags |
| `DELETE` | `/articles/{id}/relationships/tags` | Remove tags |
| `GET` | `/articles/{id}/relationships/author` | To-one relationship document |
| `PATCH` | `/articles/{id}/relationships/author` | Replace author |
| `POST` | `/operations` | Atomic operations extension (bulk create/update/delete) |

Every response carries a `X-Request-Id` header (via `RequestIdLayer`).

## Seed data

On first startup (when `APP_SEED=true`) the database is populated deterministically:

- 2 authors: `a1`, `a2`
- 12 articles: `art-01` through `art-12`, split between the two authors
- Tags and comments associated with the articles

The seed is idempotent — re-running the server against an existing database will not duplicate
records.

## Member Name Validation

JSON:API member names (resource types, attribute keys, relationship names) are validated at two points:

**Compile time** — the `#[derive(JsonApi)]` macro calls `validate_member_name` during macro
expansion. Every `#[jsonapi(type = "...")]` value and every renamed field is checked before the
binary is produced. An invalid name (e.g. one that starts or ends with a hyphen) is a compile
error, not a runtime surprise.

**Runtime** — client-supplied names that arrive in request bodies are validated with
`jsonapi_core::validate_member_name` before any database work begins. In the atomic operations
handler (`POST /operations`), the resource `type` string on each `add` and `update` operation is
validated and rejected with `400 Bad Request` when it is not a valid JSON:API member name. This is
distinct from the `422 Unprocessable Entity` returned for a syntactically valid but unsupported
type such as `"widgets"`: a malformed name never reaches the dispatch table.

## Frontend (Leptos CSR SPA)

A client-side-rendered SPA in `examples/frontend` that drives every backend endpoint and captures
the raw JSON:API wire exchange in a persistent right-side inspector — method, URL, request/response
headers, pretty-printed body, HTTP status, duration, and `X-Request-Id` — all self-captured,
independent of browser devtools.

### Prerequisites

```sh
rustup target add wasm32-unknown-unknown
cargo install trunk --locked wasm-bindgen-cli
```

### Run locally

```sh
# terminal 1 — from examples/
just run-backend

# terminal 2 — from examples/
just run-frontend
```

Open http://127.0.0.1:8081.

### Feature pages

| Page | What it exercises |
|---|---|
| Read (single) | Fetch one resource by ID, compound document with `?include=` |
| Read (typed list) | Typed collection, content-type negotiation |
| Pagination | Page-number, offset/limit, and cursor strategies side-by-side |
| Includes | Transitive compound documents (`?include=author,tags`) |
| Sparse fieldsets | `?fields[articles]=title,body` reducing wire payload |
| Sort & filter | `?sort=`, `?filter[…]=` query params |
| Create / edit | Resource creation and `Field<T>`-aware PATCH (absent fields left unchanged) |
| Relationships | To-one and to-many relationship endpoints (GET/PATCH/POST/DELETE) |
| Errors | 415 / 406 / 404 / 422 error aggregation and pointer mapping |
| Atomic ops | `POST /operations` with `lid` cross-references |

### Tests

```sh
# from examples/
just test-frontend
```

Runs native unit tests (`cargo test --lib`) plus the wasm store smoke test under node
(`wasm-bindgen-test-runner`) — no browser required.

### Config

The API base URL is resolved at runtime from `window.__SHOWCASE_CONFIG__.apiBase` (set in
`examples/frontend/index.html`) and falls back to `http://127.0.0.1:8080`. For custom deployments
set the `API_BASE` environment variable at build time (`trunk build`) to bake a different default.

### Docker

```sh
# from the repo root — starts backend on :8080 and frontend on :8081
docker compose -f examples/docker/compose.yml up
```
