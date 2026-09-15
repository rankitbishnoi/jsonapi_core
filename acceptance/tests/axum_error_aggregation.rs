//! Acceptance: a create request with two invalid attributes comes back as a
//! single JSON:API `422` document carrying one error member per violation, each
//! with the right `source.pointer` — built with the fluent `ApiError` builder
//! and the `ApiErrors` accumulator, driven through a `jsonapi_axum` router.

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::response::IntoResponse;
use axum::routing::post;
use axum::http::{Request, StatusCode, header};
use tower::ServiceExt;

use jsonapi_axum::{ApiErrorExt, ApiErrors, JsonApi, JsonApiError, JsonApiLayer, with_status};
use serde_json::Value;

const JSON_API: &str = "application/vnd.api+json";

#[derive(Debug, Clone, jsonapi_core::JsonApi)]
#[jsonapi(type = "articles")]
struct NewArticle {
    #[jsonapi(id)]
    id: Option<String>,
    title: String,
    body: String,
}

/// Accumulate one `422` per blank attribute, then return them as one document.
async fn create(JsonApi(document): JsonApi<NewArticle>) -> Result<impl IntoResponse, JsonApiError> {
    let new = document
        .into_single()
        .map_err(|err| JsonApiError::from_core(&err))?;

    let mut errors = ApiErrors::new();
    if new.title.trim().is_empty() {
        errors.push(
            with_status(422)
                .pointer("/data/attributes/title")
                .detail("title must not be empty"),
        );
    }
    if new.body.trim().is_empty() {
        errors.push(
            with_status(422)
                .pointer("/data/attributes/body")
                .detail("body must not be empty"),
        );
    }
    if !errors.is_empty() {
        return Err(errors.into());
    }

    Ok(StatusCode::CREATED)
}

fn app() -> Router {
    Router::new()
        .route("/articles", post(create))
        .layer(JsonApiLayer::new())
}

async fn read_json(response: axum::response::Response) -> (StatusCode, Value) {
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[test]
fn create_with_two_invalid_attributes_returns_one_422_with_two_members() {
    pollster::block_on(async {
        let body = serde_json::json!({
            "data": { "type": "articles", "attributes": { "title": "", "body": "" } }
        });
        let request = Request::builder()
            .method("POST")
            .uri("/articles")
            .header(header::CONTENT_TYPE, JSON_API)
            .header(header::ACCEPT, JSON_API)
            .body(Body::from(body.to_string()))
            .unwrap();

        let (status, json) = read_json(app().oneshot(request).await.unwrap()).await;

        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        let members = json["errors"].as_array().unwrap();
        assert_eq!(members.len(), 2);
        assert_eq!(members[0]["status"], "422");
        assert_eq!(members[0]["source"]["pointer"], "/data/attributes/title");
        assert_eq!(members[1]["source"]["pointer"], "/data/attributes/body");
    });
}
