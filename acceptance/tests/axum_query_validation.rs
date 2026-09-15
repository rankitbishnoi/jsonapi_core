//! Acceptance: opting into `JsonApiQueryValidated` gives a consumer include-path
//! validation against a `TypeRegistry` held in application state — a known
//! relationship passes, an unknown one is rejected with a spec-shaped 400 error
//! document naming the offending segment — all through a `jsonapi_axum` router.

use std::sync::Arc;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::routing::get;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use jsonapi_axum::{JsonApiQueryValidated, JsonApiResponse};
use jsonapi_core::{DocumentBuilder, Relationship, TypeRegistry};
use serde_json::Value;

#[derive(Debug, Clone, jsonapi_core::JsonApi)]
#[jsonapi(type = "people")]
struct Person {
    #[jsonapi(id)]
    id: String,
    #[allow(dead_code)]
    name: String,
}

#[derive(Debug, Clone, jsonapi_core::JsonApi)]
#[jsonapi(type = "articles")]
struct Article {
    #[jsonapi(id)]
    id: String,
    #[allow(dead_code)]
    title: String,
    #[jsonapi(relationship, type = "people")]
    #[allow(dead_code)]
    author: Relationship<Person>,
}

async fn list(_query: JsonApiQueryValidated<Article>) -> JsonApiResponse<Article> {
    JsonApiResponse::new(DocumentBuilder::collection(Vec::<Article>::new()).build())
}

fn app() -> Router {
    let mut registry = TypeRegistry::new();
    registry.register::<Article>().register::<Person>();
    Router::new()
        .route("/articles", get(list))
        .with_state(Arc::new(registry))
}

async fn read_json(response: axum::response::Response) -> (StatusCode, Value) {
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, value)
}

#[test]
fn known_include_path_is_accepted() {
    pollster::block_on(async {
        let request = Request::builder()
            .uri("/articles?include=author")
            .body(Body::empty())
            .unwrap();
        let (status, json) = read_json(app().oneshot(request).await.unwrap()).await;
        assert_eq!(status, StatusCode::OK);
        assert!(json["data"].is_array());
    });
}

#[test]
fn unknown_include_path_is_rejected_with_400_naming_the_segment() {
    pollster::block_on(async {
        let request = Request::builder()
            .uri("/articles?include=bogus")
            .body(Body::empty())
            .unwrap();
        let (status, json) = read_json(app().oneshot(request).await.unwrap()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json["errors"][0]["status"], "400");
        assert!(
            json["errors"][0]["detail"]
                .as_str()
                .unwrap()
                .contains("bogus"),
            "detail must name the offending include segment"
        );
    });
}
