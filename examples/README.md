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
