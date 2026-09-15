//! A runnable JSON:API CRUD server built with `jsonapi_axum`.
//!
//! Demonstrates the whole adapter surface end to end:
//! - [`JsonApiLayer`] for content-type (415) and accept (406) negotiation,
//! - [`JsonApi<T>`] to extract a typed request document,
//! - [`JsonApiQuery`] to read `sort`/`page`/`filter`/`include`,
//! - [`JsonApiResponse`] to serialize responses (with the right media type),
//! - [`JsonApiError`] for spec-shaped error documents.
//!
//! Run it with:
//!
//! ```text
//! cargo run -p jsonapi_axum --example crud_server
//! ```
//!
//! Then, for example:
//!
//! ```text
//! curl -s localhost:3000/articles \
//!   -H 'content-type: application/vnd.api+json' \
//!   -d '{"data":{"type":"articles","attributes":{"title":"Hi","body":"..."}}}'
//! curl -s localhost:3000/articles
//! ```
//!
//! Note the create body omits `id`: the server assigns it. That is why the
//! request type ([`NewArticle`]) declares `id: Option<String>` while the
//! response type ([`Article`]) requires it — JSON:API lets a client omit `id`
//! when creating a resource the server will identify.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::routing::get;
use http::StatusCode;

// Runtime types are re-exported from jsonapi_axum, so a handler crate imports
// from one place. (Deriving `JsonApi` below still needs a direct `jsonapi_core`
// dependency — the derive macro expands to `::jsonapi_core` paths.)
use jsonapi_axum::{
    ApiError, DocumentBuilder, JsonApi, JsonApiError, JsonApiLayer, JsonApiQuery, JsonApiResponse,
    NormalizeErrorsLayer,
};

/// The domain resource, as returned in responses — `id` is always present.
#[derive(Debug, Clone, jsonapi_core::JsonApi)]
#[jsonapi(type = "articles")]
struct Article {
    #[jsonapi(id)]
    id: String,
    title: String,
    body: String,
}

/// The create/replace request body — `id` is optional so a client may omit it
/// and let the server assign one (JSON:API's server-assigned-id create flow).
#[derive(Debug, Clone, jsonapi_core::JsonApi)]
#[jsonapi(type = "articles")]
struct NewArticle {
    #[jsonapi(id)]
    id: Option<String>,
    title: String,
    body: String,
}

/// In-memory application state (an `Arc`-shared store, cloned per request).
#[derive(Clone, Default)]
struct AppState {
    articles: Arc<Mutex<BTreeMap<String, Article>>>,
    next_id: Arc<Mutex<u64>>,
}

impl AppState {
    fn allocate_id(&self) -> String {
        let mut next = self.next_id.lock().unwrap();
        *next += 1;
        next.to_string()
    }
}

/// A `404` JSON:API error document for a missing article.
fn not_found(id: &str) -> JsonApiError {
    JsonApiError::from_api_error(ApiError {
        status: Some("404".to_string()),
        title: Some("Not Found".to_string()),
        detail: Some(format!("article `{id}` does not exist")),
        ..Default::default()
    })
}

/// `GET /articles` — list all articles as a collection document.
async fn list_articles(
    State(state): State<AppState>,
    JsonApiQuery(query): JsonApiQuery,
) -> impl IntoResponse {
    // `query` carries parsed sort/page/filter/include; applying those to the
    // datastore is the consumer's job (a deliberate non-goal of the library).
    // Sparse fieldsets (`?fields[articles]=title`) are applied to the response
    // for free by handing `query.fields` to `JsonApiResponse::fields`.
    let articles: Vec<Article> = state.articles.lock().unwrap().values().cloned().collect();
    JsonApiResponse::new(DocumentBuilder::collection(articles).build()).fields(query.fields)
}

/// `GET /articles/{id}` — fetch a single article.
async fn get_article(
    State(state): State<AppState>,
    Path(id): Path<String>,
    JsonApiQuery(query): JsonApiQuery,
) -> Result<JsonApiResponse<Article>, JsonApiError> {
    let article = state.articles.lock().unwrap().get(&id).cloned();
    match article {
        Some(article) => Ok(JsonApiResponse::new(DocumentBuilder::single(article).build())
            .fields(query.fields)),
        None => Err(not_found(&id)),
    }
}

/// `POST /articles` — create an article from a typed request document. The
/// client may omit `id` (see [`NewArticle`]); the server always assigns one.
async fn create_article(
    State(state): State<AppState>,
    JsonApi(document): JsonApi<NewArticle>,
) -> Result<impl IntoResponse, JsonApiError> {
    let new = document
        .into_single()
        .map_err(|err| JsonApiError::from_core(&err))?;

    // Any client-supplied `id` is ignored; the server owns identity here.
    let article = Article {
        id: state.allocate_id(),
        title: new.title,
        body: new.body,
    };
    state
        .articles
        .lock()
        .unwrap()
        .insert(article.id.clone(), article.clone());

    Ok(JsonApiResponse::new(DocumentBuilder::single(article).build()).status(StatusCode::CREATED))
}

/// `PATCH /articles/{id}` — replace an article's attributes. The id comes from
/// the URL, so the body's `id` is optional and, if present, ignored.
async fn update_article(
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

    Ok(JsonApiResponse::new(DocumentBuilder::single(article).build()))
}

/// `DELETE /articles/{id}` — remove an article.
async fn delete_article(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, JsonApiError> {
    if state.articles.lock().unwrap().remove(&id).is_some() {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(not_found(&id))
    }
}

/// Build the application router. Kept as a function so it can be tested via
/// `tower::ServiceExt::oneshot` without binding a socket.
fn app() -> Router {
    Router::new()
        .route("/articles", get(list_articles).post(create_article))
        .route(
            "/articles/{id}",
            get(get_article).patch(update_article).delete(delete_article),
        )
        // Unmatched routes get a JSON:API 404 instead of axum's plain-text one.
        .fallback(jsonapi_axum::not_found)
        // One layer enforces JSON:API content negotiation for the whole API:
        // 415 for a bad request Content-Type, 406 for an unacceptable Accept.
        .layer(JsonApiLayer::new())
        // Safety net at the router edge: any framework-generated error response
        // (405 method-not-allowed, 413 body-limit, built-in extractor rejections)
        // is re-shaped into a JSON:API error document, so nothing leaks as
        // text/plain. Already-JSON:API responses pass through unchanged.
        .layer(NormalizeErrorsLayer::new())
        .with_state(AppState::default())
}

#[tokio::main]
async fn main() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .expect("bind 127.0.0.1:3000");
    println!(
        "JSON:API CRUD server listening on http://{}",
        listener.local_addr().unwrap()
    );
    axum::serve(listener, app()).await.expect("server error");
}
