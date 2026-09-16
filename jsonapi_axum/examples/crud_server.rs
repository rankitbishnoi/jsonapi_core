//! A runnable JSON:API CRUD server built with `jsonapi_axum`.
//!
//! Demonstrates the whole adapter surface end to end:
//! - [`JsonApiLayer`] for content-type (415) and accept (406) negotiation,
//! - [`NormalizeErrorsLayer`] + [`not_found`](jsonapi_axum::not_found) to
//!   JSON:API-shape framework errors (404/405/…),
//! - [`JsonApi<T>`] to extract typed request documents (and PATCH partial
//!   updates via [`Field`]),
//! - [`JsonApiQuery`] for `sort`/`page`/`filter`/`include`/`fields`,
//! - [`JsonApiResponse`] to serialize responses — with sparse fieldsets,
//!   self links, the negotiated media type, and `201 Created` + `Location`,
//! - [`pagination_links`](jsonapi_axum::pagination_links) for a paginated list,
//! - id consistency ([`JsonApi::require_id`]) and a client-id policy
//!   ([`JsonApi::check_client_id`]),
//! - a `/articles/{id}/relationships/tags` endpoint via [`JsonApiToMany`] +
//!   [`RelationshipResponse`],
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
//!   -d '{"data":{"type":"articles","attributes":{"title":"Hi","body":"..."}}}' -i
//! curl -sg 'localhost:3000/articles?page[number]=1&page[size]=2'
//! curl -s localhost:3000/articles/1/relationships/tags \
//!   -X POST -H 'content-type: application/vnd.api+json' \
//!   -d '{"data":[{"type":"tags","id":"rust"}]}'
//! ```

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::{FromRef, Path, State};
use axum::response::IntoResponse;
use axum::routing::get;
use http::{StatusCode, Uri};

// Runtime types are re-exported from jsonapi_axum, so a handler crate imports
// from one place. (Deriving `JsonApi` below still needs a direct `jsonapi_core`
// dependency — the derive macro expands to `::jsonapi_core` paths.)
use jsonapi_axum::{
    ApiErrorExt, BaseUrl, ClientIdPolicy, DocumentBuilder, Field, IntoJsonApiError, JsonApi,
    JsonApiError, JsonApiLayer, JsonApiQuery, JsonApiResponse, JsonApiToMany, NegotiatedMediaType,
    NormalizeErrorsLayer, RelationshipResponse, RequestIdLayer, ResultExt, pagination_links,
    with_status,
};
use jsonapi_core::{
    Link, PageNumberPage, PageStrategy, RelationshipData, ResourceIdentifier, links,
};
use tower_http::request_id::{MakeRequestUuid, SetRequestIdLayer};

/// The domain resource, as returned in responses — `id` is always present.
/// `summary` is nullable, to show a PATCH clearing it.
#[derive(Debug, Clone, jsonapi_core::JsonApi)]
#[jsonapi(type = "articles")]
struct Article {
    #[jsonapi(id)]
    id: String,
    title: String,
    body: String,
    summary: Option<String>,
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
    summary: Option<String>,
}

/// The PATCH request body — every attribute is a tri-state [`Field`], so the
/// handler can tell "leave unchanged" (absent) from "clear" (null) from "set".
/// This is what makes `update_article` a genuine JSON:API partial update.
#[derive(Debug, Clone, jsonapi_core::JsonApi)]
#[jsonapi(type = "articles")]
struct ArticlePatch {
    #[jsonapi(id)]
    id: String,
    title: Field<String>,
    body: Field<String>,
    summary: Field<String>,
}

/// In-memory application state (an `Arc`-shared store, cloned per request).
#[derive(Clone)]
struct AppState {
    base_url: BaseUrl,
    articles: Arc<Mutex<BTreeMap<String, Article>>>,
    /// Out-of-band `tags` linkage per article, driven by the relationship endpoint.
    tags: Arc<Mutex<BTreeMap<String, Vec<ResourceIdentifier>>>>,
    next_id: Arc<Mutex<u64>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            base_url: BaseUrl("http://localhost:3000".to_string()),
            articles: Arc::default(),
            tags: Arc::default(),
            next_id: Arc::default(),
        }
    }
}

// The base URL is provided from state via `FromRef`, so the `BaseUrl` extractor
// works in any handler (Shared Decision 1: explicit, proxy-safe).
impl FromRef<AppState> for BaseUrl {
    fn from_ref(state: &AppState) -> BaseUrl {
        state.base_url.clone()
    }
}

impl AppState {
    fn allocate_id(&self) -> String {
        let mut next = self.next_id.lock().unwrap();
        *next += 1;
        next.to_string()
    }
}

/// A `404` JSON:API error document for a missing article, built with the fluent
/// [`with_status`] + [`ApiErrorExt`] builder instead of a raw struct literal.
fn not_found(id: &str) -> JsonApiError {
    JsonApiError::from_api_error(
        with_status(StatusCode::NOT_FOUND).detail(format!("article `{id}` does not exist")),
    )
}

/// A domain-level validation error. Implementing [`IntoJsonApiError`] lets a
/// handler propagate it with `?` via [`ResultExt::or_json_api`], keeping the
/// mapping-to-HTTP concern out of the handler body.
struct BlankTitle;

impl IntoJsonApiError for BlankTitle {
    fn into_json_api_error(self) -> JsonApiError {
        JsonApiError::from_api_error(
            with_status(StatusCode::UNPROCESSABLE_ENTITY)
                .pointer("/data/attributes/title")
                .detail("title must not be empty"),
        )
    }
}

/// A trivial domain rule: an article title may not be blank.
fn validate_title(title: &str) -> Result<(), BlankTitle> {
    if title.trim().is_empty() {
        Err(BlankTitle)
    } else {
        Ok(())
    }
}

/// `GET /articles` — a paginated collection with `self`/`first`/`prev`/`next`/
/// `last` links, honoring `page[number]`/`page[size]` and preserving other query
/// params. Sparse fieldsets and the negotiated media type are applied too.
async fn list_articles(
    State(state): State<AppState>,
    NegotiatedMediaType(media): NegotiatedMediaType,
    uri: Uri,
    JsonApiQuery(query): JsonApiQuery,
) -> Result<impl IntoResponse, JsonApiError> {
    let page = PageNumberPage::from_query(&query).map_err(|err| JsonApiError::from_core(&err))?;
    let number = page.number.max(1);
    let size = page.size.unwrap_or(2).max(1);

    let all: Vec<Article> = state.articles.lock().unwrap().values().cloned().collect();
    let total = all.len() as u64;
    // Slicing the page is the consumer's job; the library only represents it.
    let start = ((number - 1) * size) as usize;
    let items: Vec<Article> = all.into_iter().skip(start).take(size as usize).collect();

    let strategy = PageStrategy::PageNumber { number, size };
    let links = pagination_links(&uri, strategy, Some(total));

    Ok(
        JsonApiResponse::new(DocumentBuilder::collection(items).links(links).build())
            .fields(query.fields)
            .media_type(media),
    )
}

/// `GET /articles/{id}` — a single article with a `self` link.
async fn get_article(
    State(state): State<AppState>,
    BaseUrl(base): BaseUrl,
    NegotiatedMediaType(media): NegotiatedMediaType,
    Path(id): Path<String>,
    JsonApiQuery(query): JsonApiQuery,
) -> Result<JsonApiResponse<Article>, JsonApiError> {
    let article = state.articles.lock().unwrap().get(&id).cloned();
    match article {
        Some(article) => {
            let self_link = links::resource_self(&base, "articles", &id);
            Ok(JsonApiResponse::new(
                DocumentBuilder::single(article)
                    .link("self", Link::String(self_link))
                    .build(),
            )
            .fields(query.fields)
            .media_type(media))
        }
        None => Err(not_found(&id)),
    }
}

/// `POST /articles` — create, responding `201 Created` with a `Location` header.
/// This server owns identity, so client-supplied ids are rejected (403).
async fn create_article(
    State(state): State<AppState>,
    BaseUrl(base): BaseUrl,
    NegotiatedMediaType(media): NegotiatedMediaType,
    document: JsonApi<NewArticle>,
) -> Result<impl IntoResponse, JsonApiError> {
    document.check_client_id(ClientIdPolicy::Forbid)?;
    let new = document
        .0
        .into_single()
        .map_err(|err| JsonApiError::from_core(&err))?;

    // Domain validation propagated with `?` via the `IntoJsonApiError` mapping.
    validate_title(&new.title).or_json_api()?;

    let article = Article {
        id: state.allocate_id(),
        title: new.title,
        body: new.body,
        summary: new.summary,
    };
    state
        .articles
        .lock()
        .unwrap()
        .insert(article.id.clone(), article.clone());

    let self_link = links::resource_self(&base, "articles", &article.id);
    Ok(JsonApiResponse::new(
        DocumentBuilder::single(article)
            .link("self", Link::String(self_link.clone()))
            .build(),
    )
    .created(self_link)
    .media_type(media))
}

/// `PATCH /articles/{id}` — a genuine JSON:API **partial** update. The body id
/// must match the URL id (else 409). Only present attributes change; `null`
/// clears the nullable `summary`; absent members are left as they were.
async fn update_article(
    State(state): State<AppState>,
    BaseUrl(base): BaseUrl,
    NegotiatedMediaType(media): NegotiatedMediaType,
    Path(id): Path<String>,
    document: JsonApi<ArticlePatch>,
) -> Result<JsonApiResponse<Article>, JsonApiError> {
    document.require_id(&id)?; // 409 Conflict if data.id != URL id
    let patch = document
        .0
        .into_single()
        .map_err(|err| JsonApiError::from_core(&err))?;

    let mut store = state.articles.lock().unwrap();
    let Some(article) = store.get_mut(&id) else {
        return Err(not_found(&id));
    };

    if let Some(title) = patch.title.into_set() {
        article.title = title;
    }
    if let Some(body) = patch.body.into_set() {
        article.body = body;
    }
    patch.summary.apply(&mut article.summary);

    let updated = article.clone();
    drop(store);

    let self_link = links::resource_self(&base, "articles", &id);
    Ok(JsonApiResponse::new(
        DocumentBuilder::single(updated)
            .link("self", Link::String(self_link))
            .build(),
    )
    .media_type(media))
}

/// `DELETE /articles/{id}` — remove an article.
async fn delete_article(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, JsonApiError> {
    if state.articles.lock().unwrap().remove(&id).is_some() {
        state.tags.lock().unwrap().remove(&id);
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(not_found(&id))
    }
}

// --- Relationship endpoint: /articles/{id}/relationships/tags (to-many) -----
//
// Per JSON:API, on a to-many relationship: POST appends, PATCH replaces the
// whole set, DELETE removes the given members. The extractor yields the linkage;
// applying these semantics is the handler's job (below).

fn tags_response(state: &AppState, id: &str) -> RelationshipResponse {
    let current = state
        .tags
        .lock()
        .unwrap()
        .get(id)
        .cloned()
        .unwrap_or_default();
    let links = links::relationship_links(&state.base_url.0, "articles", id, "tags");
    RelationshipResponse::new(RelationshipData::ToMany(current)).links(links)
}

async fn get_tags(State(state): State<AppState>, Path(id): Path<String>) -> RelationshipResponse {
    tags_response(&state, &id)
}

/// `POST` — append members that are not already present.
async fn append_tags(
    State(state): State<AppState>,
    Path(id): Path<String>,
    JsonApiToMany(incoming): JsonApiToMany,
) -> RelationshipResponse {
    {
        let mut store = state.tags.lock().unwrap();
        let set = store.entry(id.clone()).or_default();
        for rid in incoming {
            if !set.contains(&rid) {
                set.push(rid);
            }
        }
    }
    tags_response(&state, &id)
}

/// `PATCH` — replace the whole set.
async fn replace_tags(
    State(state): State<AppState>,
    Path(id): Path<String>,
    JsonApiToMany(incoming): JsonApiToMany,
) -> RelationshipResponse {
    state.tags.lock().unwrap().insert(id.clone(), incoming);
    tags_response(&state, &id)
}

/// `DELETE` — remove the given members.
async fn remove_tags(
    State(state): State<AppState>,
    Path(id): Path<String>,
    JsonApiToMany(remove): JsonApiToMany,
) -> RelationshipResponse {
    {
        let mut store = state.tags.lock().unwrap();
        if let Some(set) = store.get_mut(&id) {
            set.retain(|rid| !remove.contains(rid));
        }
    }
    tags_response(&state, &id)
}

/// Build the application router. Kept as a function so it can be tested via
/// `tower::ServiceExt::oneshot` without binding a socket.
fn app() -> Router {
    Router::new()
        .route("/articles", get(list_articles).post(create_article))
        .route(
            "/articles/{id}",
            get(get_article)
                .patch(update_article)
                .delete(delete_article),
        )
        .route(
            "/articles/{id}/relationships/tags",
            get(get_tags)
                .post(append_tags)
                .patch(replace_tags)
                .delete(remove_tags),
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
        // Correlation: stamp the request id onto every error document's
        // `errors[].id` and echo it as `x-request-id`. Outermost of the two so it
        // also stamps documents the normalize layer synthesizes (e.g. a 404).
        .layer(RequestIdLayer::new())
        // Mint/propagate the `x-request-id` the layer above reads. Outermost of
        // all, so the header is present on the request before anything runs.
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
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
