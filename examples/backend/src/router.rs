use axum::Router;
use axum::routing::get;
use jsonapi_axum::{JsonApiLayer, NormalizeErrorsLayer, RequestIdLayer, not_found};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::routes;
use crate::state::AppState;

/// Build the application router with all routes and middleware layers.
pub fn build_router(state: AppState) -> Router {
    // Read cors_origins before state is moved into `.with_state`.
    let cors = build_cors(&state.cors_origins);

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
        .fallback(not_found)
        // Layers applied last-added = outermost. Order from innermost to outermost:
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .layer(JsonApiLayer::new().ext([jsonapi_core::atomic::ATOMIC_EXT_URI]))
        .layer(NormalizeErrorsLayer::new())
        .layer(RequestIdLayer::new().generate())
        .with_state(state)
}

/// Build a [`CorsLayer`] from the configured allowed origins.
///
/// An empty list means "permissive" — used in tests so cross-origin requests
/// aren't blocked. A non-empty list restricts to the provided origins.
fn build_cors(origins: &[String]) -> CorsLayer {
    if origins.is_empty() {
        CorsLayer::new()
            .allow_origin(tower_http::cors::Any)
            .allow_methods(tower_http::cors::Any)
            .allow_headers(tower_http::cors::Any)
    } else {
        let parsed: Vec<axum::http::HeaderValue> = origins
            .iter()
            .filter_map(|o| match o.parse() {
                Ok(v) => Some(v),
                Err(_) => {
                    tracing::warn!(origin = %o, "ignoring unparseable CORS origin");
                    None
                }
            })
            .collect();
        CorsLayer::new()
            .allow_origin(parsed)
            .allow_methods(tower_http::cors::Any)
            .allow_headers(tower_http::cors::Any)
    }
}
