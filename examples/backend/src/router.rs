use axum::Router;
use axum::routing::get;
use jsonapi_axum::JsonApiLayer;

use crate::routes;
use crate::state::AppState;

/// Build the application router with all routes and middleware layers.
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(routes::health::health))
        .route(
            "/articles",
            get(routes::articles::list).post(routes::articles::create),
        )
        .route("/articles/offset", get(routes::articles::list_offset))
        .route("/articles/cursor", get(routes::articles::list_cursor))
        .route(
            "/articles/{id}",
            get(routes::articles::get)
                .patch(routes::articles::patch)
                .delete(routes::articles::delete),
        )
        .layer(JsonApiLayer::new())
        .with_state(state)
}
