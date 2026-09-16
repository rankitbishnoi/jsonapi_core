#[path = "support.rs"]
mod support;
use axum::http::StatusCode;
use jsonapi_axum::testing::{RouterTestExt, TestRequest};
use serde_json::json;

#[tokio::test]
async fn get_tags_returns_linkage_with_links() {
    let app = support::seeded_app().await;
    let res = app
        .send(
            TestRequest::get("/articles/art-01/relationships/tags")
                .accept_json_api()
                .build(),
        )
        .await;
    let res = res.assert_status(StatusCode::OK);
    let data = res.json()["data"].as_array().unwrap().clone();
    assert!(!data.is_empty());
    assert_eq!(data[0]["type"], "tags");
    assert_eq!(
        res.json()["links"]["self"],
        "http://api.test/articles/art-01/relationships/tags"
    );
    assert_eq!(
        res.json()["links"]["related"],
        "http://api.test/articles/art-01/tags"
    );
}

#[tokio::test]
async fn replace_tags_sets_linkage() {
    let app = support::seeded_app().await;
    let res = app
        .send(
            TestRequest::patch("/articles/art-01/relationships/tags")
                .content_type_json_api()
                .body_json(&json!({ "data": [{ "type": "tags", "id": "t-web" }] }))
                .build(),
        )
        .await;
    let res = res.assert_status(StatusCode::OK);
    let data = res.json()["data"].as_array().unwrap().clone();
    assert_eq!(data.len(), 1);
    assert_eq!(data[0]["id"], "t-web");
}

#[tokio::test]
async fn set_author_updates_to_one_linkage() {
    let app = support::seeded_app().await;
    let res = app
        .send(
            TestRequest::patch("/articles/art-01/relationships/author")
                .content_type_json_api()
                .body_json(&json!({ "data": { "type": "authors", "id": "a2" } }))
                .build(),
        )
        .await;
    let res = res.assert_status(StatusCode::OK);
    assert_eq!(res.json()["data"]["id"], "a2");
    assert_eq!(res.json()["data"]["type"], "authors");
}

#[tokio::test]
async fn to_one_endpoint_rejects_array_payload_400() {
    let app = support::seeded_app().await;
    let res = app
        .send(
            TestRequest::patch("/articles/art-01/relationships/author")
                .content_type_json_api()
                .body_json(&json!({ "data": [{ "type": "authors", "id": "a2" }] }))
                .build(),
        )
        .await;
    res.assert_error(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn add_and_remove_tags_adjust_linkage() {
    let app = support::seeded_app().await;
    // art-01 is odd (i=1) so it has t-rust + t-api
    let add = app
        .clone()
        .send(
            TestRequest::post("/articles/art-01/relationships/tags")
                .content_type_json_api()
                .body_json(&json!({ "data": [{ "type": "tags", "id": "t-web" }] }))
                .build(),
        )
        .await;
    let add = add.assert_status(StatusCode::OK);
    assert_eq!(add.json()["data"].as_array().unwrap().len(), 3);
    // remove t-api
    let rm = app
        .send(
            TestRequest::delete("/articles/art-01/relationships/tags")
                .content_type_json_api()
                .body_json(&json!({ "data": [{ "type": "tags", "id": "t-api" }] }))
                .build(),
        )
        .await;
    let rm = rm.assert_status(StatusCode::OK);
    let ids: Vec<&str> = rm.json()["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&"t-rust") && ids.contains(&"t-web") && !ids.contains(&"t-api"));
}

#[tokio::test]
async fn set_author_rejects_null_data_400() {
    let app = support::seeded_app().await;
    let res = app
        .send(
            TestRequest::patch("/articles/art-01/relationships/author")
                .content_type_json_api()
                .body_json(&json!({ "data": null }))
                .build(),
        )
        .await;
    res.assert_error(StatusCode::BAD_REQUEST);
}
