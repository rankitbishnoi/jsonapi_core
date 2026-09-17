use axum::http::StatusCode;
use jsonapi_axum::testing::{RouterTestExt, TestRequest};
use jsonapi_showcase_backend::router::build_router;
use jsonapi_showcase_backend::state::AppState;

#[tokio::test]
async fn health_returns_ok_json_api() {
    let state = AppState::in_memory().await.expect("state");
    let app = build_router(state);
    let res = app
        .send(TestRequest::get("/health").accept_json_api().build())
        .await;
    let res = res
        .assert_status(StatusCode::OK)
        .assert_json_api_content_type();
    assert_eq!(res.json()["meta"]["status"], "ok");
}
