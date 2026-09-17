//! Acceptance: Tier 0 hardening of the `jsonapi_axum` stack, driven through a
//! router from a downstream consumer's seat.
//!
//! - **G1** sparse fieldsets: `?fields[articles]=title` trims a real response's
//!   attributes while retaining `type`/`id`.
//! - **G3** framework-error shaping: an unmatched route yields a JSON:API `404`
//!   document (via the `not_found` fallback), and a wrong method yields a
//!   JSON:API `405` document (via the `NormalizeErrorsLayer` safety net) — never
//!   a `text/plain` body.

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::State;
use axum::http::{Request, StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::get;
use std::sync::Arc;
use tower::ServiceExt;

use jsonapi_axum::{JsonApiLayer, JsonApiQuery, JsonApiResponse, NormalizeErrorsLayer, not_found};
use jsonapi_core::DocumentBuilder;
use serde_json::Value;

const JSON_API: &str = "application/vnd.api+json";

#[derive(Debug, Clone, jsonapi_core::JsonApi)]
#[jsonapi(type = "articles")]
struct Article {
    #[jsonapi(id)]
    id: String,
    title: String,
    body: String,
}

/// A fixed one-article store, so the list response is deterministic.
async fn list(
    State(article): State<Arc<Article>>,
    JsonApiQuery(query): JsonApiQuery,
) -> impl IntoResponse {
    JsonApiResponse::new(DocumentBuilder::collection(vec![(*article).clone()]).build())
        .fields(query.fields)
}

fn app() -> Router {
    let article = Arc::new(Article {
        id: "1".to_string(),
        title: "Hi".to_string(),
        body: "World".to_string(),
    });
    Router::new()
        .route("/articles", get(list))
        .fallback(not_found)
        .layer(JsonApiLayer::new())
        .layer(NormalizeErrorsLayer::new())
        .with_state(article)
}

async fn read_json(response: axum::response::Response) -> (StatusCode, String, Value) {
    let status = response.status();
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, content_type, value)
}

#[test]
fn g1_sparse_fieldset_trims_the_list_response() {
    pollster::block_on(async {
        let request = Request::builder()
            .uri("/articles?fields[articles]=title")
            .header(header::ACCEPT, JSON_API)
            .body(Body::empty())
            .unwrap();
        let (status, content_type, json) = read_json(app().oneshot(request).await.unwrap()).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(content_type, JSON_API);
        let item = &json["data"][0];
        assert_eq!(item["type"], "articles");
        assert_eq!(item["id"], "1");
        assert_eq!(item["attributes"]["title"], "Hi");
        assert!(
            item["attributes"].get("body").is_none(),
            "the unrequested `body` attribute must be trimmed"
        );
    });
}

#[test]
fn g1_absent_fieldset_returns_every_attribute() {
    pollster::block_on(async {
        let request = Request::builder()
            .uri("/articles")
            .header(header::ACCEPT, JSON_API)
            .body(Body::empty())
            .unwrap();
        let (status, _ct, json) = read_json(app().oneshot(request).await.unwrap()).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["data"][0]["attributes"]["title"], "Hi");
        assert_eq!(json["data"][0]["attributes"]["body"], "World");
    });
}

#[test]
fn g3_unmatched_route_yields_json_api_404() {
    pollster::block_on(async {
        let request = Request::builder()
            .uri("/no-such-route")
            .body(Body::empty())
            .unwrap();
        let (status, content_type, json) = read_json(app().oneshot(request).await.unwrap()).await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(content_type, JSON_API);
        assert_eq!(json["errors"][0]["status"], "404");
    });
}

#[test]
fn g3_wrong_method_yields_json_api_405() {
    pollster::block_on(async {
        // `/articles` exists for GET only; a POST is a framework 405, which the
        // normalize layer re-shapes from text/plain into a JSON:API document.
        let request = Request::builder()
            .method("POST")
            .uri("/articles")
            .header(header::CONTENT_TYPE, JSON_API)
            .body(Body::empty())
            .unwrap();
        let (status, content_type, json) = read_json(app().oneshot(request).await.unwrap()).await;

        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(content_type, JSON_API);
        assert_eq!(json["errors"][0]["status"], "405");
    });
}
