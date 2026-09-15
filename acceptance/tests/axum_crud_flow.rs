//! Acceptance: the full JSON:API CRUD lifecycle driven through a `jsonapi_axum`
//! router from a downstream consumer's seat — create (server-assigned id), read,
//! list, update, and delete — plus the spec-shaped error documents for a missing
//! resource (404) and an invalid body (422).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::{Path, State};
use axum::http::{Request, StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::get;
use tower::ServiceExt;

use jsonapi_axum::{JsonApi, JsonApiError, JsonApiLayer, JsonApiQuery, JsonApiResponse};
use jsonapi_core::{ApiError, DocumentBuilder};
use serde_json::Value;

const JSON_API: &str = "application/vnd.api+json";

#[derive(Debug, Clone, jsonapi_core::JsonApi)]
#[jsonapi(type = "articles")]
struct Article {
    #[jsonapi(id)]
    id: String,
    title: String,
    body: String,
}

/// Create/replace request body: `id` is optional so a client may omit it and
/// let the server assign one (JSON:API's server-assigned-id create flow).
#[derive(Debug, Clone, jsonapi_core::JsonApi)]
#[jsonapi(type = "articles")]
struct NewArticle {
    #[jsonapi(id)]
    id: Option<String>,
    title: String,
    body: String,
}

#[derive(Clone, Default)]
struct AppState {
    articles: Arc<Mutex<BTreeMap<String, Article>>>,
    next_id: Arc<Mutex<u64>>,
}

fn not_found(id: &str) -> JsonApiError {
    JsonApiError::from_api_error(ApiError {
        status: Some("404".to_string()),
        title: Some("Not Found".to_string()),
        detail: Some(format!("article `{id}` does not exist")),
        ..Default::default()
    })
}

async fn list(State(state): State<AppState>, JsonApiQuery(_q): JsonApiQuery) -> impl IntoResponse {
    let articles: Vec<Article> = state.articles.lock().unwrap().values().cloned().collect();
    JsonApiResponse::new(DocumentBuilder::collection(articles).build())
}

async fn get_one(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<JsonApiResponse<Article>, JsonApiError> {
    match state.articles.lock().unwrap().get(&id).cloned() {
        Some(article) => Ok(JsonApiResponse::new(
            DocumentBuilder::single(article).build(),
        )),
        None => Err(not_found(&id)),
    }
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
    };
    state.articles.lock().unwrap().insert(id, article.clone());
    Ok(JsonApiResponse::new(DocumentBuilder::single(article).build()).status(StatusCode::CREATED))
}

async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    JsonApi(document): JsonApi<NewArticle>,
) -> Result<JsonApiResponse<Article>, JsonApiError> {
    let incoming = document
        .into_single()
        .map_err(|err| JsonApiError::from_core(&err))?;
    let mut store = state.articles.lock().unwrap();
    if !store.contains_key(&id) {
        return Err(not_found(&id));
    }
    let article = Article {
        id: id.clone(),
        title: incoming.title,
        body: incoming.body,
    };
    store.insert(id, article.clone());
    Ok(JsonApiResponse::new(
        DocumentBuilder::single(article).build(),
    ))
}

async fn delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, JsonApiError> {
    if state.articles.lock().unwrap().remove(&id).is_some() {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(not_found(&id))
    }
}

fn app() -> Router {
    Router::new()
        .route("/articles", get(list).post(create))
        .route("/articles/{id}", get(get_one).patch(update).delete(delete))
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

/// Create one article through the router and return its server-assigned id.
async fn seed_article(app: &Router, title: &str, body: &str) -> String {
    let req = Request::builder()
        .method("POST")
        .uri("/articles")
        .header(header::CONTENT_TYPE, JSON_API)
        .body(Body::from(format!(
            r#"{{"data":{{"type":"articles","attributes":{{"title":"{title}","body":"{body}"}}}}}}"#
        )))
        .unwrap();
    let (status, json) = read_json(app.clone().oneshot(req).await.unwrap()).await;
    assert_eq!(status, StatusCode::CREATED);
    json["data"]["id"].as_str().unwrap().to_owned()
}

#[test]
fn create_read_list_and_not_found() {
    pollster::block_on(async {
        let app = app();

        // 1. Create — the client omits `id`; the server assigns one and
        //    responds 201 with the resource (JSON:API server-assigned-id flow).
        let create_req = Request::builder()
            .method("POST")
            .uri("/articles")
            .header(header::CONTENT_TYPE, JSON_API)
            .body(Body::from(
                r#"{"data":{"type":"articles",
                    "attributes":{"title":"First","body":"Hello"}}}"#,
            ))
            .unwrap();
        let (status, json) = read_json(app.clone().oneshot(create_req).await.unwrap()).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(json["data"]["type"], "articles");
        assert_eq!(json["data"]["attributes"]["title"], "First");
        // Server assigns the id — capture it rather than assuming its value.
        let assigned_id = json["data"]["id"]
            .as_str()
            .expect("server must assign a string id")
            .to_owned();
        assert!(!assigned_id.is_empty(), "server assigns a non-empty id");

        // 2. Read the created resource back by its assigned id.
        let get_req = Request::builder()
            .uri(format!("/articles/{assigned_id}"))
            .body(Body::empty())
            .unwrap();
        let (status, json) = read_json(app.clone().oneshot(get_req).await.unwrap()).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["data"]["id"], assigned_id);
        assert_eq!(json["data"]["attributes"]["body"], "Hello");

        // 3. List with sort + page params — parses through the stack, returns a
        //    collection (applying them to a datastore is the consumer's job).
        let list_req = Request::builder()
            .uri("/articles?sort=-title&page[size]=10")
            .header(header::ACCEPT, JSON_API)
            .body(Body::empty())
            .unwrap();
        let (status, json) = read_json(app.clone().oneshot(list_req).await.unwrap()).await;
        assert_eq!(status, StatusCode::OK);
        let data = json["data"]
            .as_array()
            .expect("collection data is an array");
        assert_eq!(data.len(), 1, "exactly the one created article");
        assert_eq!(data[0]["id"], assigned_id);

        // 4. A missing resource yields a spec-shaped 404 error document.
        let missing_req = Request::builder()
            .uri("/articles/999")
            .body(Body::empty())
            .unwrap();
        let (status, json) = read_json(app.oneshot(missing_req).await.unwrap()).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(json["errors"][0]["status"], "404");
        assert!(
            json["errors"][0]["detail"]
                .as_str()
                .unwrap()
                .contains("999")
        );
    });
}

#[test]
fn create_ignores_a_client_supplied_id() {
    pollster::block_on(async {
        // A client MAY send an `id`; this server owns identity, so it is ignored
        // and replaced with the server-assigned one.
        let request = Request::builder()
            .method("POST")
            .uri("/articles")
            .header(header::CONTENT_TYPE, JSON_API)
            .body(Body::from(
                r#"{"data":{"type":"articles","id":"client-temp",
                    "attributes":{"title":"First","body":"Hello"}}}"#,
            ))
            .unwrap();
        let (status, json) = read_json(app().oneshot(request).await.unwrap()).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_ne!(
            json["data"]["id"], "client-temp",
            "client id must be replaced"
        );
    });
}

#[test]
fn patch_updates_an_existing_article() {
    pollster::block_on(async {
        let app = app();
        let id = seed_article(&app, "Before", "Old body").await;

        // PATCH replaces the attributes; the response reflects the new state.
        let patch_req = Request::builder()
            .method("PATCH")
            .uri(format!("/articles/{id}"))
            .header(header::CONTENT_TYPE, JSON_API)
            .body(Body::from(format!(
                r#"{{"data":{{"type":"articles","id":"{id}",
                    "attributes":{{"title":"After","body":"New body"}}}}}}"#
            )))
            .unwrap();
        let (status, json) = read_json(app.clone().oneshot(patch_req).await.unwrap()).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["data"]["attributes"]["title"], "After");

        // Round-trip: read it back to prove the update persisted, not just echoed.
        let get_req = Request::builder()
            .uri(format!("/articles/{id}"))
            .body(Body::empty())
            .unwrap();
        let (status, json) = read_json(app.oneshot(get_req).await.unwrap()).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["data"]["attributes"]["title"], "After");
        assert_eq!(json["data"]["attributes"]["body"], "New body");
    });
}

#[test]
fn patch_on_missing_article_yields_404() {
    pollster::block_on(async {
        let request = Request::builder()
            .method("PATCH")
            .uri("/articles/404")
            .header(header::CONTENT_TYPE, JSON_API)
            .body(Body::from(
                r#"{"data":{"type":"articles","id":"404",
                    "attributes":{"title":"x","body":"y"}}}"#,
            ))
            .unwrap();
        let (status, json) = read_json(app().oneshot(request).await.unwrap()).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(json["errors"][0]["status"], "404");
    });
}

#[test]
fn delete_removes_an_article_then_get_is_404() {
    pollster::block_on(async {
        let app = app();
        let id = seed_article(&app, "Doomed", "body").await;

        let delete_req = Request::builder()
            .method("DELETE")
            .uri(format!("/articles/{id}"))
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(delete_req).await.unwrap();
        // 204 No Content — no body.
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        let (status, _) = read_json(response).await;
        assert_eq!(status, StatusCode::NO_CONTENT);

        // Round-trip: the resource is really gone.
        let get_req = Request::builder()
            .uri(format!("/articles/{id}"))
            .body(Body::empty())
            .unwrap();
        let (status, _) = read_json(app.oneshot(get_req).await.unwrap()).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    });
}

#[test]
fn delete_on_missing_article_yields_404() {
    pollster::block_on(async {
        let request = Request::builder()
            .method("DELETE")
            .uri("/articles/404")
            .body(Body::empty())
            .unwrap();
        let (status, json) = read_json(app().oneshot(request).await.unwrap()).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(json["errors"][0]["status"], "404");
    });
}

#[test]
fn create_with_missing_attribute_yields_422_with_pointer() {
    pollster::block_on(async {
        // `body` is a required attribute; omitting it must surface a spec-shaped
        // 422 with a JSON pointer to the offending member, through the full stack.
        let request = Request::builder()
            .method("POST")
            .uri("/articles")
            .header(header::CONTENT_TYPE, JSON_API)
            .body(Body::from(
                r#"{"data":{"type":"articles","attributes":{"title":"No body"}}}"#,
            ))
            .unwrap();
        let (status, json) = read_json(app().oneshot(request).await.unwrap()).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(json["errors"][0]["status"], "422");
        assert_eq!(
            json["errors"][0]["source"]["pointer"],
            "/data/attributes/body"
        );
    });
}
