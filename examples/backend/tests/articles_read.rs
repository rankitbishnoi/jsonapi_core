#[path = "support.rs"]
mod support;
use axum::http::StatusCode;
use jsonapi_axum::testing::{RouterTestExt, TestRequest};

#[tokio::test]
async fn get_single_article_has_self_link_and_type() {
    let app = support::seeded_app().await;
    let res = app
        .send(
            TestRequest::get("/articles/art-01")
                .accept_json_api()
                .build(),
        )
        .await;
    let res = res
        .assert_status(StatusCode::OK)
        .assert_json_api_content_type();
    assert_eq!(res.json()["data"]["type"], "articles");
    assert_eq!(res.json()["data"]["id"], "art-01");
    assert_eq!(
        res.json()["links"]["self"],
        "http://api.test/articles/art-01"
    );
}

#[tokio::test]
async fn get_missing_article_returns_404() {
    let app = support::seeded_app().await;
    let res = app
        .send(TestRequest::get("/articles/nope").accept_json_api().build())
        .await;
    res.assert_status(StatusCode::NOT_FOUND)
        .assert_error(StatusCode::NOT_FOUND);
}
