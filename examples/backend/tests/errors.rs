#[path = "support.rs"]
mod support;

use axum::http::StatusCode;
use axum::http::header;
use jsonapi_axum::testing::{RouterTestExt, TestRequest};
use serde_json::json;

#[tokio::test]
async fn wrong_content_type_on_post_is_415() {
    let app = support::seeded_app().await;
    let res = app
        .send(
            TestRequest::post("/articles")
                .header(header::CONTENT_TYPE, "application/json")
                .raw_body(b"{}".to_vec())
                .build(),
        )
        .await;
    res.assert_error(StatusCode::UNSUPPORTED_MEDIA_TYPE);
}

#[tokio::test]
async fn unacceptable_accept_is_406() {
    let app = support::seeded_app().await;
    let res = app
        .send(
            TestRequest::get("/articles")
                .header(header::ACCEPT, "text/html")
                .build(),
        )
        .await;
    res.assert_error(StatusCode::NOT_ACCEPTABLE);
}

#[tokio::test]
async fn malformed_body_yields_error_with_pointer() {
    let app = support::seeded_app().await;
    // Valid content-type, but missing required attributes (no title/body/author).
    let res = app
        .send(
            TestRequest::post("/articles")
                .body_json(&json!({ "data": { "type": "articles", "attributes": {} } }))
                .build(),
        )
        .await;
    let status = res.status.as_u16();
    assert!(
        status == 400 || status == 422,
        "expected 400 or 422, got {status}"
    );
    let errors = res.errors();
    assert!(
        errors[0]["source"]["pointer"].is_string() || errors[0]["detail"].is_string(),
        "error should carry a pointer or detail; got: {errors}"
    );
}

#[tokio::test]
async fn unknown_route_is_normalized_json_api_404() {
    let app = support::seeded_app().await;
    let res = app
        .send(
            TestRequest::get("/does-not-exist")
                .accept_json_api()
                .build(),
        )
        .await;
    res.assert_error(StatusCode::NOT_FOUND)
        .assert_json_api_content_type();
}

#[tokio::test]
async fn responses_carry_a_request_id() {
    let app = support::seeded_app().await;
    let res = app
        .send(TestRequest::get("/health").accept_json_api().build())
        .await;
    assert!(
        res.header("x-request-id").is_some(),
        "x-request-id should be echoed/generated"
    );
}
