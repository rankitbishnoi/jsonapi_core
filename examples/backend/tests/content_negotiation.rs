//! Tests demonstrating that `NegotiatedMediaType` flows from the `AcceptLayer`
//! through the handler and into the response `Content-Type`.
#[path = "support.rs"]
mod support;

use axum::http::StatusCode;
use axum::http::header;
use jsonapi_axum::testing::{RouterTestExt, TestRequest};

/// A plain `Accept: application/vnd.api+json` request negotiates to the plain
/// media type, so the response `Content-Type` is exactly
/// `application/vnd.api+json`.
#[tokio::test]
async fn get_article_with_plain_accept_returns_plain_content_type() {
    let app = support::seeded_app().await;
    let res = app
        .send(
            TestRequest::get("/articles/art-01")
                .header(header::ACCEPT, "application/vnd.api+json")
                .build(),
        )
        .await;
    let res = res.assert_status(StatusCode::OK);
    let ct = res
        .header("content-type")
        .expect("content-type header present");
    assert!(
        ct.starts_with("application/vnd.api+json"),
        "expected application/vnd.api+json content-type, got: {ct}"
    );
}
