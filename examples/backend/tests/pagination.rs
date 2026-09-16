#[path = "support.rs"]
mod support;
use axum::http::StatusCode;
use jsonapi_axum::testing::{RouterTestExt, TestRequest};

#[tokio::test]
async fn page_number_first_page_has_next_and_last_no_prev() {
    let app = support::seeded_app().await;
    let res = app
        .send(
            TestRequest::get("/articles?page[number]=1&page[size]=5")
                .accept_json_api()
                .build(),
        )
        .await;
    let res = res.assert_status(StatusCode::OK);
    assert_eq!(res.json()["data"].as_array().unwrap().len(), 5);
    assert!(res.json()["links"]["next"].is_string());
    assert!(res.json()["links"]["last"].is_string());
    assert!(res.json()["links"].get("prev").is_none());
}

#[tokio::test]
async fn offset_pagination_returns_window() {
    let app = support::seeded_app().await;
    let res = app
        .send(
            TestRequest::get("/articles/offset?page[offset]=10&page[limit]=5")
                .accept_json_api()
                .build(),
        )
        .await;
    let res = res.assert_status(StatusCode::OK);
    // 12 total articles, offset 10 -> 2 remaining
    assert_eq!(res.json()["data"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn cursor_pagination_advances_and_sets_profile() {
    let app = support::seeded_app().await;
    let first = app
        .clone()
        .send(
            TestRequest::get("/articles/cursor?page[size]=5")
                .accept_json_api()
                .build(),
        )
        .await;
    let first = first.assert_status(StatusCode::OK);
    let ct = first
        .header("content-type")
        .expect("content-type header must be present");
    assert!(
        ct.contains("ethanresnick/cursor-pagination"),
        "content-type was {ct}"
    );
    assert_eq!(first.json()["data"].as_array().unwrap().len(), 5);

    // Follow the next link — cursor links are relative paths
    let next = first.json()["links"]["next"]
        .as_str()
        .expect("next link must be present")
        .to_string();
    let second = app
        .send(TestRequest::get(&next).accept_json_api().build())
        .await;
    let second = second.assert_status(StatusCode::OK);
    assert_eq!(second.json()["data"][0]["id"], "art-06");
}

#[tokio::test]
async fn cursor_pagination_last_page_omits_next() {
    let app = support::seeded_app().await;
    let res = app
        .send(
            TestRequest::get("/articles/cursor?page[after]=art-10&page[size]=5")
                .accept_json_api()
                .build(),
        )
        .await;
    let res = res.assert_status(StatusCode::OK);
    // art-11 and art-12 are the only remaining articles after art-10
    assert_eq!(res.json()["data"].as_array().unwrap().len(), 2);
    assert!(
        res.json()["links"].get("next").is_none(),
        "last page must omit next"
    );
}
