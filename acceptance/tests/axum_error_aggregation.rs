//! Acceptance: a create request with two invalid attributes comes back as a
//! single JSON:API `422` document carrying one error member per violation, each
//! with the right `source.pointer` — built with the fluent `ApiError` builder
//! and the `ApiErrors` accumulator, driven through a `jsonapi_axum` router.

use axum::Router;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::post;

use jsonapi_axum::testing::{RouterTestExt, TestRequest};
use jsonapi_axum::{ApiErrorExt, ApiErrors, JsonApi, JsonApiError, JsonApiLayer, with_status};

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

#[test]
fn create_with_two_invalid_attributes_returns_one_422_with_two_members() {
    pollster::block_on(async {
        let body = serde_json::json!({
            "data": { "type": "articles", "attributes": { "title": "", "body": "" } }
        });
        let response = app()
            .send(
                TestRequest::post("/articles")
                    .accept_json_api()
                    .body_json(&body)
                    .build(),
            )
            .await
            .assert_status(StatusCode::UNPROCESSABLE_ENTITY)
            .assert_error_count(2)
            .assert_error(422)
            .assert_error_pointer("/data/attributes/title");

        // The second member is bespoke — drop to the raw document for it.
        assert_eq!(
            response.errors()[1]["source"]["pointer"],
            "/data/attributes/body"
        );
    });
}
