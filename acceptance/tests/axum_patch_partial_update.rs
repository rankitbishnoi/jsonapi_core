//! Acceptance (G4): PATCH partial-update from a downstream consumer's seat.
//!
//! Create an article, PATCH a single attribute, and GET it back to prove only
//! that attribute changed; then PATCH `null` to clear a nullable attribute.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::{Path, State};
use axum::http::{Request, StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use tower::ServiceExt;

use jsonapi_axum::{DocumentBuilder, Field, JsonApi, JsonApiError, JsonApiLayer, JsonApiResponse};
use serde_json::{Value, json};

const JSON_API: &str = "application/vnd.api+json";

#[derive(Debug, Clone, jsonapi_core::JsonApi)]
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
struct NewArticle {
    #[jsonapi(id)]
    id: Option<String>,
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

#[derive(Clone, Default)]
struct AppState {
    articles: Arc<Mutex<BTreeMap<String, Article>>>,
    next_id: Arc<Mutex<u64>>,
}

async fn create(
    State(state): State<AppState>,
    JsonApi(document): JsonApi<NewArticle>,
) -> Result<impl IntoResponse, JsonApiError> {
    let new = document
        .into_single()
        .map_err(|err| JsonApiError::from_core(&err))?;
    let id = {
        let mut next = state.next_id.lock().unwrap();
        *next += 1;
        next.to_string()
    };
    let article = Article {
        id: id.clone(),
        title: new.title,
        body: new.body,
        summary: new.summary,
    };
    state.articles.lock().unwrap().insert(id, article.clone());
    Ok(JsonApiResponse::new(DocumentBuilder::single(article).build()).status(StatusCode::CREATED))
}

async fn get_one(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<JsonApiResponse<Article>, JsonApiError> {
    match state.articles.lock().unwrap().get(&id).cloned() {
        Some(article) => Ok(JsonApiResponse::new(
            DocumentBuilder::single(article).build(),
        )),
        None => Err(JsonApiError::from_api_error(jsonapi_core::ApiError {
            status: Some("404".into()),
            ..Default::default()
        })),
    }
}

async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    JsonApi(document): JsonApi<ArticlePatch>,
) -> Result<JsonApiResponse<Article>, JsonApiError> {
    let patch = document
        .into_single()
        .map_err(|err| JsonApiError::from_core(&err))?;
    let mut store = state.articles.lock().unwrap();
    let article = store.get_mut(&id).expect("article exists");
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

fn app() -> Router {
    Router::new()
        .route("/articles", post(create))
        .route("/articles/{id}", get(get_one).patch(update))
        .layer(JsonApiLayer::new())
        .with_state(AppState::default())
}

async fn read_json(response: axum::response::Response) -> (StatusCode, Value) {
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, value)
}

fn req(method: &str, uri: &str, body: Option<Value>) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(uri);
    if body.is_some() {
        builder = builder.header(header::CONTENT_TYPE, JSON_API);
    }
    builder
        .body(body.map_or(Body::empty(), |v| Body::from(v.to_string())))
        .unwrap()
}

#[test]
fn create_then_partial_patch_changes_only_the_patched_member() {
    pollster::block_on(async {
        let app = app();

        // 1. Create with all three attributes populated.
        let create_body = json!({"data": {"type": "articles",
            "attributes": {"title": "First", "body": "Hello", "summary": "A summary"}}});
        let (status, json) = read_json(
            app.clone()
                .oneshot(req("POST", "/articles", Some(create_body)))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let id = json["data"]["id"].as_str().unwrap().to_owned();

        // 2. PATCH only `title`.
        let patch_body =
            json!({"data": {"type": "articles", "id": id, "attributes": {"title": "Second"}}});
        let (status, _) = read_json(
            app.clone()
                .oneshot(req("PATCH", &format!("/articles/{id}"), Some(patch_body)))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        // 3. GET: only `title` changed; `body`/`summary` intact.
        let (status, json) = read_json(
            app.clone()
                .oneshot(req("GET", &format!("/articles/{id}"), None))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["data"]["attributes"]["title"], "Second");
        assert_eq!(json["data"]["attributes"]["body"], "Hello");
        assert_eq!(json["data"]["attributes"]["summary"], "A summary");

        // 4. PATCH `summary: null` clears the nullable attribute.
        let clear_body =
            json!({"data": {"type": "articles", "id": id, "attributes": {"summary": null}}});
        let (status, _) = read_json(
            app.clone()
                .oneshot(req("PATCH", &format!("/articles/{id}"), Some(clear_body)))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        let (_status, json) = read_json(
            app.oneshot(req("GET", &format!("/articles/{id}"), None))
                .await
                .unwrap(),
        )
        .await;
        // Cleared nullable attribute serializes as absent (the derive omits
        // `None` optionals), and title stays as last patched.
        assert!(json["data"]["attributes"].get("summary").is_none());
        assert_eq!(json["data"]["attributes"]["title"], "Second");
    });
}
