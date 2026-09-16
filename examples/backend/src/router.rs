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
        .route(
            "/articles/{id}/relationships/tags",
            get(routes::article_relationships::get_tags)
                .patch(routes::article_relationships::replace_tags)
                .post(routes::article_relationships::add_tags)
                .delete(routes::article_relationships::remove_tags),
        )
        .route(
            "/articles/{id}/relationships/author",
            get(routes::article_relationships::get_author)
                .patch(routes::article_relationships::set_author),
        )
        .route(
            "/operations",
            axum::routing::post(routes::operations::operations),
        )
        .layer(JsonApiLayer::new().ext([jsonapi_core::atomic::ATOMIC_EXT_URI]))
        .with_state(state)
}
