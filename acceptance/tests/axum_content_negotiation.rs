//! Acceptance: content negotiation through a `jsonapi_axum` router — a client
//! sending the wrong media types walks away with spec-correct 415 / 406 JSON:API
//! error documents, a conforming request succeeds, and a negotiated `ext`
//! parameter round-trips onto the response `Content-Type`.

use axum::Extension;
use axum::Router;
use axum::body::{Body, to_bytes};
use axum::routing::get;
use axum::http::{Request, StatusCode, header};
use tower::ServiceExt;

use jsonapi_axum::{JsonApi, JsonApiLayer, JsonApiResponse};
use jsonapi_core::{DocumentBuilder, JsonApiMediaType};
use serde_json::Value;

const JSON_API: &str = "application/vnd.api+json";
const ATOMIC_EXT: &str = "https://jsonapi.org/ext/atomic";

#[derive(Debug, Clone, jsonapi_core::JsonApi)]
#[jsonapi(type = "articles")]
struct Article {
    #[jsonapi(id)]
    id: String,
    title: String,
}

// Reads the media type the `AcceptLayer` negotiated and stored in the request
// extensions, and echoes it back on the response `Content-Type` — the intended
// way a handler propagates negotiated `ext`/`profile`.
async fn list(Extension(media): Extension<JsonApiMediaType>) -> JsonApiResponse<Article> {
    JsonApiResponse::new(DocumentBuilder::collection(vec![]).build()).media_type(media)
}

async fn create(JsonApi(_doc): JsonApi<Article>) -> StatusCode {
    StatusCode::CREATED
}

fn app() -> Router {
    Router::new()
        .route("/articles", get(list).post(create))
        .layer(JsonApiLayer::new().ext([ATOMIC_EXT]))
}

/// Read status, `Content-Type`, and parsed JSON body in one shot.
async fn read(response: axum::response::Response) -> (StatusCode, Option<String>, Value) {
    let status = response.status();
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, content_type, value)
}

#[test]
fn wrong_content_type_yields_415_error_document() {
    pollster::block_on(async {
        let request = Request::builder()
            .method("POST")
            .uri("/articles")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"data":{"type":"articles"}}"#))
            .unwrap();
        let (status, content_type, json) = read(app().oneshot(request).await.unwrap()).await;

        assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
        assert_eq!(json["errors"][0]["status"], "415");
        // The error document itself must be spec-shaped JSON:API.
        assert_eq!(content_type.as_deref(), Some(JSON_API));
    });
}

#[test]
fn unacceptable_accept_yields_406_error_document() {
    pollster::block_on(async {
        let request = Request::builder()
            .uri("/articles")
            .header(header::ACCEPT, "text/html")
            .body(Body::empty())
            .unwrap();
        let (status, content_type, json) = read(app().oneshot(request).await.unwrap()).await;

        assert_eq!(status, StatusCode::NOT_ACCEPTABLE);
        assert_eq!(json["errors"][0]["status"], "406");
        assert_eq!(content_type.as_deref(), Some(JSON_API));
    });
}

#[test]
fn conforming_request_succeeds() {
    pollster::block_on(async {
        let request = Request::builder()
            .uri("/articles")
            .header(header::ACCEPT, JSON_API)
            .body(Body::empty())
            .unwrap();
        let (status, content_type, json) = read(app().oneshot(request).await.unwrap()).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(content_type.as_deref(), Some(JSON_API));
        // A conforming request returns a real JSON:API collection document.
        assert!(json["data"].is_array(), "body is a collection document");
    });
}

#[test]
fn negotiated_ext_is_echoed_on_the_response_content_type() {
    pollster::block_on(async {
        // Client requests the atomic ext the server advertises; the negotiated
        // media type must round-trip onto the response Content-Type.
        let request = Request::builder()
            .uri("/articles")
            .header(header::ACCEPT, format!("{JSON_API}; ext=\"{ATOMIC_EXT}\""))
            .body(Body::empty())
            .unwrap();
        let (status, content_type, _) = read(app().oneshot(request).await.unwrap()).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            content_type.as_deref(),
            Some(format!("{JSON_API}; ext=\"{ATOMIC_EXT}\"").as_str())
        );
    });
}

#[test]
fn unadvertised_ext_is_dropped_yielding_a_plain_response() {
    pollster::block_on(async {
        // A requested ext the server does not advertise is dropped (not a 406);
        // the response falls back to a plain JSON:API Content-Type.
        let request = Request::builder()
            .uri("/articles")
            .header(header::ACCEPT, format!("{JSON_API}; ext=\"https://unadvertised/ext\""))
            .body(Body::empty())
            .unwrap();
        let (status, content_type, _) = read(app().oneshot(request).await.unwrap()).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(content_type.as_deref(), Some(JSON_API));
    });
}
