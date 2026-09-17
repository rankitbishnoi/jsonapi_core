#[path = "support.rs"]
mod support;
use axum::http::StatusCode;
use jsonapi_axum::testing::{RouterTestExt, TestRequest};

#[tokio::test]
async fn unknown_include_path_is_rejected_400() {
    let app = support::seeded_app().await;
    let res = app
        .send(
            TestRequest::get("/articles?include=bogus")
                .accept_json_api()
                .build(),
        )
        .await;
    res.assert_error(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn known_include_paths_are_accepted() {
    let app = support::seeded_app().await;
    for path in ["author", "tags", "comments", "comments.author"] {
        let res = app
            .clone()
            .send(
                TestRequest::get(format!("/articles?include={path}"))
                    .accept_json_api()
                    .build(),
            )
            .await;
        res.assert_status(StatusCode::OK);
    }
}
