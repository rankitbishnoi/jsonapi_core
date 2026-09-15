//! Compound documents / `include` resolution (G16), end to end.
//!
//! `GET /articles?include=author` returns the articles as primary data plus a
//! deduped `included` array of their authors. The library
//! ([`resolve_includes`]) walks the linkage, batches the load (one call per type
//! per level), and dedups by `(type, id)`; the consumer's [`IncludeResolver`]
//! (`PeopleResolver` below) is a dumb batch loader over the in-memory store.
//!
//! Run it with:
//!
//! ```text
//! cargo run -p jsonapi_axum --example compound_document
//! ```
//!
//! Then:
//!
//! ```text
//! curl -sg 'localhost:3001/articles?include=author'
//! ```
//!
//! Both articles share author `people/9`, so it appears exactly once under
//! `included`.

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::Arc;

use axum::Router;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::get;

use jsonapi_axum::{
    ApiErrorExt, DocumentBuilder, IncludeResolver, IntoJsonApiError, JsonApiError, JsonApiLayer,
    JsonApiQuery, JsonApiResponse, ResultExt, resolve_includes, with_status,
};
use jsonapi_core::{Identity, Relationship, RelationshipData, Resource, ResourceIdentifier};

/// An author resource.
#[derive(Debug, Clone, jsonapi_core::JsonApi)]
#[jsonapi(type = "people")]
struct Person {
    #[jsonapi(id)]
    id: String,
    name: String,
}

/// An article with a to-one `author` relationship. Serializing it to a dynamic
/// [`Resource`] gives the linkage the resolver walks.
#[derive(Debug, Clone, jsonapi_core::JsonApi)]
#[jsonapi(type = "articles")]
struct Article {
    #[jsonapi(id)]
    id: String,
    title: String,
    #[jsonapi(relationship)]
    author: Relationship<Person>,
}

/// In-memory application state.
#[derive(Clone)]
struct AppState {
    articles: Arc<Vec<Article>>,
    people: Arc<BTreeMap<String, Person>>,
}

impl AppState {
    /// A batch loader bound to this state's people store.
    fn resolver(&self) -> PeopleResolver {
        PeopleResolver {
            people: self.people.clone(),
        }
    }
}

/// The consumer's [`IncludeResolver`]: fetch resources by identity from the
/// in-memory store. This is the only piece the library asks the consumer to
/// write — no linkage-walking or dedup here.
struct PeopleResolver {
    people: Arc<BTreeMap<String, Person>>,
}

/// A load error, mapped to a JSON:API `500` at the handler edge via
/// [`IntoJsonApiError`] + [`ResultExt::or_json_api`].
#[derive(Debug)]
struct ResolveError {
    type_name: String,
}

impl IntoJsonApiError for ResolveError {
    fn into_json_api_error(self) -> JsonApiError {
        JsonApiError::from_api_error(
            with_status(500)
                .detail(format!("cannot resolve includes of type `{}`", self.type_name)),
        )
    }
}

impl IncludeResolver for PeopleResolver {
    type Error = ResolveError;

    fn load(
        &self,
        type_name: &str,
        ids: &[String],
    ) -> impl Future<Output = Result<Vec<Resource>, Self::Error>> + Send {
        // Fetch synchronously from the store, then return a ready (Send) future.
        let result = match type_name {
            "people" => ids
                .iter()
                .filter_map(|id| self.people.get(id))
                .map(|person| {
                    Resource::from_typed(person).map_err(|_| ResolveError {
                        type_name: "people".to_string(),
                    })
                })
                .collect::<Result<Vec<_>, _>>(),
            other => Err(ResolveError {
                type_name: other.to_string(),
            }),
        };
        async move { result }
    }
}

/// `GET /articles?include=...` — a compound document. The `include` paths come
/// straight from the parsed query; the resolver's error maps to a JSON:API error
/// with `?` via [`ResultExt::or_json_api`].
async fn list_articles(
    State(state): State<AppState>,
    JsonApiQuery(query): JsonApiQuery,
) -> Result<impl IntoResponse, JsonApiError> {
    let articles = (*state.articles).clone();

    // The walk reads linkage from the dynamic representation of the primaries.
    let primary: Vec<Resource> = articles
        .iter()
        .map(Resource::from_typed)
        .collect::<Result<_, _>>()
        .map_err(|err| JsonApiError::from_core(&err))?;

    let paths: Vec<&str> = query.include.iter().map(String::as_str).collect();
    let included = resolve_includes(&primary, &paths, &state.resolver())
        .await
        .or_json_api()?;

    // Primary stays typed; the deduped `included` is attached alongside it.
    let document = DocumentBuilder::collection(articles)
        .include_many(included)
        .build();
    Ok(JsonApiResponse::new(document))
}

fn author(id: &str) -> Relationship<Person> {
    Relationship::new(RelationshipData::ToOne(Some(ResourceIdentifier {
        r#type: "people".to_string(),
        identity: Identity::Id(id.to_string()),
        meta: None,
    })))
}

fn app() -> Router {
    let people = BTreeMap::from([(
        "9".to_string(),
        Person {
            id: "9".to_string(),
            name: "Dan Gebhardt".to_string(),
        },
    )]);
    // Both articles reference the same author, to show dedup in `included`.
    let articles = vec![
        Article {
            id: "1".to_string(),
            title: "JSON:API paints my bikeshed!".to_string(),
            author: author("9"),
        },
        Article {
            id: "2".to_string(),
            title: "Rails is Omakase".to_string(),
            author: author("9"),
        },
    ];
    let state = AppState {
        articles: Arc::new(articles),
        people: Arc::new(people),
    };

    Router::new()
        .route("/articles", get(list_articles))
        .layer(JsonApiLayer::new())
        .with_state(state)
}

#[tokio::main]
async fn main() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3001")
        .await
        .expect("bind 127.0.0.1:3001");
    println!(
        "JSON:API compound-document server listening on http://{}",
        listener.local_addr().unwrap()
    );
    axum::serve(listener, app()).await.expect("server error");
}
