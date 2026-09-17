//! Shared harness: an app over a fresh in-memory DB, optionally seeded.
//! Included via `#[path = "support.rs"] mod support;` in each test file.
#![allow(dead_code)]

use axum::Router;
use jsonapi_showcase_backend::router::build_router;
use jsonapi_showcase_backend::state::AppState;

pub async fn app() -> Router {
    let state = AppState::in_memory().await.expect("state");
    build_router(state)
}

pub async fn seeded_app() -> Router {
    let state = AppState::in_memory().await.expect("state");
    jsonapi_showcase_backend::repo::seed::seed(&state.pool)
        .await
        .expect("seed");
    build_router(state)
}
