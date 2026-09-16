use axum::Router;
use axum::routing::get;
use jsonapi_axum::JsonApiLayer;

use crate::routes::health::health;
use crate::state::AppState;

/// Build the application router with all routes and middleware layers.
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .layer(JsonApiLayer::new())
        .with_state(state)
}
