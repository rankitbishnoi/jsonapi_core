//! Acceptance (0.2 "complete CRUD"): from a downstream consumer's seat —
//! a create returning `201` + `Location`, a paginated list with `first`/`next`
//! links, and a relationship-endpoint replace.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::{FromRef, Path, State};
use axum::http::{Request, StatusCode, Uri, header};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use tower::ServiceExt;

use jsonapi_axum::{
    BaseUrl, DocumentBuilder, JsonApi, JsonApiError, JsonApiLayer, JsonApiQuery, JsonApiResponse,
    JsonApiToMany, RelationshipResponse, pagination_links,
};
use jsonapi_core::{Link, PageNumberPage, PageStrategy, RelationshipData, ResourceIdentifier, links};
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

#[derive(Clone)]
struct AppState {
    base_url: BaseUrl,
    articles: Arc<Mutex<BTreeMap<String, Article>>>,
    tags: Arc<Mutex<BTreeMap<String, Vec<ResourceIdentifier>>>>,
    next_id: Arc<Mutex<u64>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            base_url: BaseUrl("http://api.test".into()),
            articles: Arc::default(),
            tags: Arc::default(),
            next_id: Arc::default(),
        }
    }
}

impl FromRef<AppState> for BaseUrl {
    fn from_ref(state: &AppState) -> BaseUrl {
        state.base_url.clone()
    }
}

async fn create(
    State(state): State<AppState>,
    BaseUrl(base): BaseUrl,
    document: JsonApi<NewArticle>,
) -> Result<impl IntoResponse, JsonApiError> {
    let new = document
        .0
        .into_single()
        .map_err(|err| JsonApiError::from_core(&err))?;
    let id = {
        let mut n = state.next_id.lock().unwrap();
        *n += 1;
        n.to_string()
    };
    let article = Article {
        id: id.clone(),
        title: new.title,
    };
    state.articles.lock().unwrap().insert(id.clone(), article.clone());
    let self_link = links::resource_self(&base, "articles", &id);
    Ok(JsonApiResponse::new(
        DocumentBuilder::single(article)
            .link("self", Link::String(self_link.clone()))
            .build(),
    )
    .created(self_link))
}

async fn list(
    State(state): State<AppState>,
    uri: Uri,
    JsonApiQuery(query): JsonApiQuery,
) -> Result<impl IntoResponse, JsonApiError> {
    let page = PageNumberPage::from_query(&query).map_err(|e| JsonApiError::from_core(&e))?;
    let number = page.number.max(1);
    let size = page.size.unwrap_or(2).max(1);
    let all: Vec<Article> = state.articles.lock().unwrap().values().cloned().collect();
    let total = all.len() as u64;
    let start = ((number - 1) * size) as usize;
    let items: Vec<Article> = all.into_iter().skip(start).take(size as usize).collect();
    let l = pagination_links(&uri, PageStrategy::PageNumber { number, size }, Some(total));
    Ok(JsonApiResponse::new(DocumentBuilder::collection(items).links(l).build()))
}

async fn replace_tags(
    State(state): State<AppState>,
    Path(id): Path<String>,
    JsonApiToMany(incoming): JsonApiToMany,
) -> RelationshipResponse {
    state.tags.lock().unwrap().insert(id.clone(), incoming.clone());
    let l = links::relationship_links(&state.base_url.0, "articles", &id, "tags");
    RelationshipResponse::new(RelationshipData::ToMany(incoming)).links(l)
}

fn app() -> Router {
    Router::new()
        .route("/articles", get(list).post(create))
        .route("/articles/{id}/relationships/tags", post(replace_tags).patch(replace_tags))
        .layer(JsonApiLayer::new())
        .with_state(AppState::default())
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

async fn seed(app: &Router, title: &str) {
    let req = Request::builder()
        .method("POST")
        .uri("/articles")
        .header(header::CONTENT_TYPE, JSON_API)
        .body(Body::from(
            json!({"data": {"type": "articles", "attributes": {"title": title}}}).to_string(),
        ))
        .unwrap();
    let status = app.clone().oneshot(req).await.unwrap().status();
    assert_eq!(status, StatusCode::CREATED);
}

#[test]
fn create_returns_201_with_location_and_self_link() {
    pollster::block_on(async {
        let req = Request::builder()
            .method("POST")
            .uri("/articles")
            .header(header::CONTENT_TYPE, JSON_API)
            .body(Body::from(
                json!({"data": {"type": "articles", "attributes": {"title": "First"}}}).to_string(),
            ))
            .unwrap();
        let response = app().oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let location = response
            .headers()
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .unwrap()
            .to_string();
        assert_eq!(location, "http://api.test/articles/1");
        let (_s, json) = read(response).await;
        assert_eq!(json["links"]["self"], "http://api.test/articles/1");
    });
}

#[test]
fn paginated_list_has_first_and_next_links() {
    pollster::block_on(async {
        let app = app();
        for t in ["a", "b", "c", "d", "e"] {
            seed(&app, t).await;
        }
        // 5 articles, page 1 of size 2 → first + next + last, but no prev.
        let req = Request::builder()
            .uri("/articles?page[number]=1&page[size]=2")
            .header(header::ACCEPT, JSON_API)
            .body(Body::empty())
            .unwrap();
        let (status, json) = read(app.oneshot(req).await.unwrap()).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["data"].as_array().unwrap().len(), 2);
        assert_eq!(json["links"]["first"], "/articles?page[number]=1&page[size]=2");
        assert_eq!(json["links"]["next"], "/articles?page[number]=2&page[size]=2");
        assert!(json["links"].get("prev").is_none(), "first page omits prev");
        assert_eq!(json["links"]["last"], "/articles?page[number]=3&page[size]=2");
    });
}

#[test]
fn relationship_replace_sets_the_linkage_and_returns_a_relationship_document() {
    pollster::block_on(async {
        let app = app();
        seed(&app, "with-tags").await;
        let req = Request::builder()
            .method("PATCH")
            .uri("/articles/1/relationships/tags")
            .header(header::CONTENT_TYPE, JSON_API)
            .body(Body::from(
                json!({"data": [{"type": "tags", "id": "rust"}, {"type": "tags", "id": "web"}]})
                    .to_string(),
            ))
            .unwrap();
        let (status, json) = read(app.oneshot(req).await.unwrap()).await;
        assert_eq!(status, StatusCode::OK);
        let data = json["data"].as_array().unwrap();
        assert_eq!(data.len(), 2);
        assert_eq!(data[0]["type"], "tags");
        assert_eq!(data[0]["id"], "rust");
        assert_eq!(
            json["links"]["self"],
            "http://api.test/articles/1/relationships/tags"
        );
        assert_eq!(json["links"]["related"], "http://api.test/articles/1/tags");
    });
}
