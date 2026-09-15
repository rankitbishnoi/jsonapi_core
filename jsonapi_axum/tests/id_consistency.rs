//! G5 — id consistency (`PATCH` body id vs URL id) and client-id policy (`POST`)
//! through the `jsonapi_axum` bindings, driven with `oneshot`.

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::Path;
use axum::http::{Request, StatusCode, header};
use axum::routing::{patch, post};
use tower::ServiceExt;

use jsonapi_axum::{ClientIdPolicy, JsonApi, JsonApiError};
use serde_json::{Value, json};

const JSON_API: &str = "application/vnd.api+json";

#[derive(Debug, Clone, jsonapi_core::JsonApi)]
#[jsonapi(type = "articles")]
struct Article {
    #[jsonapi(id)]
    id: String,
    title: String,
}

#[derive(Debug, Clone, jsonapi_core::JsonApi)]
#[jsonapi(type = "articles")]
struct NewArticle {
    #[jsonapi(id)]
    id: Option<String>,
    title: String,
}

async fn update(Path(id): Path<String>, doc: JsonApi<Article>) -> Result<StatusCode, JsonApiError> {
    doc.require_id(&id)?; // 409 if body id != URL id
    Ok(StatusCode::OK)
}

/// A create endpoint that forbids client-generated ids.
async fn create_forbid(doc: JsonApi<NewArticle>) -> Result<StatusCode, JsonApiError> {
    doc.check_client_id(ClientIdPolicy::Forbid)?; // 403 if a client id is present
    Ok(StatusCode::CREATED)
}

fn app() -> Router {
    Router::new()
        .route("/articles/{id}", patch(update))
        .route("/articles", post(create_forbid))
}

async fn read(response: axum::response::Response) -> (StatusCode, Value) {
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, value)
}

fn body_req(method: &str, uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, JSON_API)
        .body(Body::from(body.to_string()))
        .unwrap()
}

#[test]
fn patch_with_mismatched_id_is_409_with_pointer() {
    pollster::block_on(async {
        let body = json!({"data": {"type": "articles", "id": "2", "attributes": {"title": "x"}}});
        let (status, json) = read(
            app()
                .oneshot(body_req("PATCH", "/articles/1", body))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(json["errors"][0]["status"], "409");
        assert_eq!(json["errors"][0]["source"]["pointer"], "/data/id");
    });
}

#[test]
fn patch_with_matching_id_is_ok() {
    pollster::block_on(async {
        let body = json!({"data": {"type": "articles", "id": "1", "attributes": {"title": "x"}}});
        let (status, _) = read(
            app()
                .oneshot(body_req("PATCH", "/articles/1", body))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    });
}

#[test]
fn post_with_client_id_under_forbid_policy_is_403() {
    pollster::block_on(async {
        let body =
            json!({"data": {"type": "articles", "id": "client-1", "attributes": {"title": "x"}}});
        let (status, json) = read(
            app()
                .oneshot(body_req("POST", "/articles", body))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(json["errors"][0]["status"], "403");
        assert_eq!(json["errors"][0]["source"]["pointer"], "/data/id");
    });
}

#[test]
fn post_without_client_id_under_forbid_policy_is_ok() {
    pollster::block_on(async {
        let body = json!({"data": {"type": "articles", "attributes": {"title": "x"}}});
        let (status, _) = read(
            app()
                .oneshot(body_req("POST", "/articles", body))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
    });
}
