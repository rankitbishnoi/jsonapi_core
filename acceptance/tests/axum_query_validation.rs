//! Acceptance: opting into `JsonApiQueryValidated` gives a consumer include-path
//! validation against a `TypeRegistry` held in application state — a known
//! relationship passes, an unknown one is rejected with a spec-shaped 400 error
//! document naming the offending segment — all through a `jsonapi_axum` router.

use std::sync::Arc;

use axum::Router;
use axum::routing::get;
use axum::http::StatusCode;

use jsonapi_axum::testing::{RouterTestExt, TestRequest};
use jsonapi_axum::{JsonApiQueryValidated, JsonApiResponse};
use jsonapi_core::{DocumentBuilder, Relationship, TypeRegistry};

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

#[test]
fn known_include_path_is_accepted() {
    pollster::block_on(async {
        let response = app()
            .send(TestRequest::get("/articles?include=author").build())
            .await
            .assert_status(StatusCode::OK);
        assert!(response.data().is_array());
    });
}

#[test]
fn unknown_include_path_is_rejected_with_400_naming_the_segment() {
    pollster::block_on(async {
        let response = app()
            .send(TestRequest::get("/articles?include=bogus").build())
            .await
            .assert_status(StatusCode::BAD_REQUEST)
            .assert_error(400);
        assert!(
            response.errors()[0]["detail"]
                .as_str()
                .unwrap()
                .contains("bogus"),
            "detail must name the offending include segment"
        );
    });
}
