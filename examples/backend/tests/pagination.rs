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
