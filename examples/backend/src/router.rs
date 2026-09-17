use axum::Router;
use axum::http::Method;
use axum::http::header::{ACCEPT, CONTENT_TYPE};
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
        // Layers applied last-added = outermost. CORS must be OUTERMOST so that
        // responses short-circuited by inner layers (e.g. JsonApiLayer's 415/406
        // content-negotiation guard) still receive `Access-Control-Allow-Origin`;
        // otherwise the browser blocks those error responses as CORS failures.
        // Order from innermost to outermost:
        .layer(TraceLayer::new_for_http())
        .layer(JsonApiLayer::new().ext([jsonapi_core::atomic::ATOMIC_EXT_URI]))
        .layer(NormalizeErrorsLayer::new())
        .layer(RequestIdLayer::new().generate())
        .layer(cors)
        .with_state(state)
}

/// Build a [`CorsLayer`] from the configured allowed origins.
///
/// An empty list means "permissive" — used in tests so cross-origin requests
/// aren't blocked. A non-empty list restricts to the provided origins.
fn build_cors(origins: &[String]) -> CorsLayer {
    // Enumerate methods/headers explicitly rather than `Any`. A wildcard
    // `Access-Control-Allow-Methods: *` is not reliably honoured by browsers for
    // preflighted verbs like PATCH (and never in credentialed mode), so writes
    // were CORS-blocked in the browser even though the origin was allowed.
    let methods = [
        Method::GET,
        Method::POST,
        Method::PATCH,
        Method::PUT,
        Method::DELETE,
        Method::OPTIONS,
    ];
    let headers = [CONTENT_TYPE, ACCEPT];
    let base = CorsLayer::new()
        .allow_methods(methods)
        .allow_headers(headers);
    if origins.is_empty() {
        base.allow_origin(tower_http::cors::Any)
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
        base.allow_origin(parsed)
    }
}
