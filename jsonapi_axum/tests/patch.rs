//! G4 — PATCH partial-update semantics through the `jsonapi_axum` extractor.
//!
//! Drives a stateful router with `oneshot` and asserts the *stored* entity after
//! each PATCH, proving: only present attributes change, an explicit `null` clears
//! a nullable attribute, and a PATCH omitting a create-required attribute
//! succeeds (no 422) because the patch type uses `Field<T>`.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::{Path, State};
use axum::http::{Request, StatusCode, header};
use axum::routing::patch;
use tower::ServiceExt;

use jsonapi_axum::{DocumentBuilder, Field, JsonApi, JsonApiError, JsonApiResponse};
use serde_json::{Value, json};

const JSON_API: &str = "application/vnd.api+json";

#[derive(Debug, Clone, PartialEq, jsonapi_core::JsonApi)]
#[jsonapi(type = "articles")]
struct Article {
    #[jsonapi(id)]
    id: String,
    title: String,
    body: String,
    summary: Option<String>,
}

#[derive(Debug, Clone, jsonapi_core::JsonApi)]
#[jsonapi(type = "articles")]
struct ArticlePatch {
    #[jsonapi(id)]
    id: String,
    title: Field<String>,
    body: Field<String>,
    summary: Field<String>,
}

type Store = Arc<Mutex<BTreeMap<String, Article>>>;

async fn update(
    State(store): State<Store>,
    Path(id): Path<String>,
    JsonApi(document): JsonApi<ArticlePatch>,
) -> Result<JsonApiResponse<Article>, JsonApiError> {
    let patch = document
        .into_single()
        .map_err(|err| JsonApiError::from_core(&err))?;

    let mut guard = store.lock().unwrap();
    let article = guard.get_mut(&id).expect("seeded article exists");

    if let Some(title) = patch.title.into_set() {
        article.title = title;
    }
    if let Some(body) = patch.body.into_set() {
        article.body = body;
    }
    patch.summary.apply(&mut article.summary);

    Ok(JsonApiResponse::new(
        DocumentBuilder::single(article.clone()).build(),
    ))
}

/// A router seeded with one article, so we can assert the stored state after a
/// PATCH via the shared `Store` handle.
fn app_with_seed(seed: Article) -> (Router, Store) {
    let store: Store = Arc::new(Mutex::new(BTreeMap::new()));
    store.lock().unwrap().insert(seed.id.clone(), seed);
    let router = Router::new()
        .route("/articles/{id}", patch(update))
        .with_state(store.clone());
    (router, store)
}

fn patch_request(id: &str, attributes: Value) -> Request<Body> {
    let body = json!({"data": {"type": "articles", "id": id, "attributes": attributes}});
    Request::builder()
        .method("PATCH")
        .uri(format!("/articles/{id}"))
        .header(header::CONTENT_TYPE, JSON_API)
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn seed() -> Article {
    Article {
        id: "1".to_string(),
        title: "Original title".to_string(),
        body: "Original body".to_string(),
        summary: Some("Original summary".to_string()),
    }
}

async fn status_and_json(response: axum::response::Response) -> (StatusCode, Value) {
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, value)
}

#[test]
fn patch_with_only_title_leaves_other_attributes_unchanged() {
    pollster::block_on(async {
        let (app, store) = app_with_seed(seed());
        let response = app
            .oneshot(patch_request("1", json!({"title": "New title"})))
            .await
            .unwrap();
        let (status, json) = status_and_json(response).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["data"]["attributes"]["title"], "New title");

        // Assert the *stored* entity: only title changed.
        let stored = store.lock().unwrap().get("1").cloned().unwrap();
        assert_eq!(stored.title, "New title");
        assert_eq!(stored.body, "Original body");
        assert_eq!(stored.summary, Some("Original summary".to_string()));
    });
}

#[test]
fn patch_with_null_clears_the_nullable_attribute() {
    pollster::block_on(async {
        let (app, store) = app_with_seed(seed());
        let response = app
            .oneshot(patch_request("1", json!({"summary": null})))
            .await
            .unwrap();
        let (status, _json) = status_and_json(response).await;
        assert_eq!(status, StatusCode::OK);

        let stored = store.lock().unwrap().get("1").cloned().unwrap();
        // summary cleared; title/body untouched.
        assert_eq!(stored.summary, None);
        assert_eq!(stored.title, "Original title");
        assert_eq!(stored.body, "Original body");
    });
}

#[test]
fn patch_omitting_required_on_create_attribute_succeeds_without_422() {
    pollster::block_on(async {
        // `title`/`body` are required when creating, but this PATCH omits them
        // entirely (empty attributes). A full-typed extractor would 422; the
        // Field<T> patch type resolves them all to Absent and 200s.
        let (app, store) = app_with_seed(seed());
        let response = app.oneshot(patch_request("1", json!({}))).await.unwrap();
        let (status, _json) = status_and_json(response).await;
        assert_eq!(status, StatusCode::OK);

        // Nothing changed.
        let stored = store.lock().unwrap().get("1").cloned().unwrap();
        assert_eq!(stored, seed());
    });
}
