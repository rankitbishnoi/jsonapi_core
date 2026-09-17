#[path = "support.rs"]
mod support;
use axum::http::StatusCode;
use jsonapi_axum::testing::{RouterTestExt, TestRequest};

#[tokio::test]
async fn sparse_fieldset_limits_attributes() {
    let app = support::seeded_app().await;
    let res = app
        .send(
            TestRequest::get("/articles/art-01?fields[articles]=title")
                .accept_json_api()
                .build(),
        )
        .await;
    let res = res.assert_status(StatusCode::OK);
    let attrs = &res.json()["data"]["attributes"];
    assert!(attrs.get("title").is_some());
    assert!(attrs.get("body").is_none());
}

#[tokio::test]
async fn sort_by_title_desc_orders_results() {
    let app = support::seeded_app().await;
    let res = app
        .send(
            TestRequest::get("/articles?sort=-title&page[size]=50")
                .accept_json_api()
                .build(),
        )
        .await;
    let res = res.assert_status(StatusCode::OK);
    assert_eq!(res.json()["data"][0]["attributes"]["title"], "Article 9");
}

#[tokio::test]
async fn unknown_sort_field_is_rejected_400() {
    let app = support::seeded_app().await;
    let res = app
        .send(
            TestRequest::get("/articles?sort=bogus")
                .accept_json_api()
                .build(),
        )
        .await;
    res.assert_error(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn filter_by_author_narrows_the_collection() {
    let app = support::seeded_app().await;
    let res = app
        .send(
            TestRequest::get("/articles?filter[author]=a1&page[size]=50")
                .accept_json_api()
                .build(),
        )
        .await;
    let res = res.assert_status(StatusCode::OK);
    assert_eq!(res.json()["data"].as_array().unwrap().len(), 6);
}
