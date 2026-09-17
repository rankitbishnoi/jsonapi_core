//! Acceptance (G16 — compound documents / includes): from a downstream
//! consumer's seat, `GET /articles?include=author` returns a compound document
//! whose `included` array carries the shared author exactly once. The consumer
//! writes only a batch [`IncludeResolver`]; the library walks and dedups.

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::Arc;

use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;

use jsonapi_axum::testing::{RouterTestExt, TestRequest};
use jsonapi_axum::{
    DocumentBuilder, IncludeResolver, JsonApiError, JsonApiLayer, JsonApiQuery, JsonApiResponse,
    resolve_includes,
};
use jsonapi_core::{Identity, Relationship, RelationshipData, Resource, ResourceIdentifier};
use serde_json::Value;

#[derive(Debug, Clone, jsonapi_core::JsonApi)]
#[jsonapi(type = "people")]
struct Person {
    #[jsonapi(id)]
    id: String,
    name: String,
}

#[derive(Debug, Clone, jsonapi_core::JsonApi)]
#[jsonapi(type = "articles")]
struct Article {
    #[jsonapi(id)]
    id: String,
    title: String,
    #[jsonapi(relationship)]
    author: Relationship<Person>,
}

#[derive(Clone)]
struct AppState {
    articles: Arc<Vec<Article>>,
    people: Arc<BTreeMap<String, Person>>,
}

/// The consumer's batch loader over the in-memory store.
struct PeopleResolver {
    people: Arc<BTreeMap<String, Person>>,
}

#[derive(Debug)]
struct ResolveError;

impl IncludeResolver for PeopleResolver {
    type Error = ResolveError;

    fn load(
        &self,
        type_name: &str,
        ids: &[String],
    ) -> impl Future<Output = Result<Vec<Resource>, Self::Error>> + Send {
        let result = match type_name {
            "people" => ids
                .iter()
                .filter_map(|id| self.people.get(id))
                .map(|person| Resource::from_typed(person).map_err(|_| ResolveError))
                .collect::<Result<Vec<_>, _>>(),
            _ => Err(ResolveError),
        };
        async move { result }
    }
}

async fn list_articles(
    State(state): State<AppState>,
    JsonApiQuery(query): JsonApiQuery,
) -> Result<impl IntoResponse, JsonApiError> {
    let articles = (*state.articles).clone();
    let primary: Vec<Resource> = articles
        .iter()
        .map(Resource::from_typed)
        .collect::<Result<_, _>>()
        .map_err(|err| JsonApiError::from_core(&err))?;
    let paths: Vec<&str> = query.include.iter().map(String::as_str).collect();
    let resolver = PeopleResolver {
        people: state.people.clone(),
    };
    let included = resolve_includes(&primary, &paths, &resolver)
        .await
        .map_err(|_| JsonApiError::internal("include resolution failed"))?;
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
    Router::new()
        .route("/articles", get(list_articles))
        .layer(JsonApiLayer::new())
        .with_state(AppState {
            articles: Arc::new(articles),
            people: Arc::new(people),
        })
}

#[test]
fn include_author_returns_a_compound_document_with_a_deduped_author() {
    pollster::block_on(async {
        let response = app()
            .send(
                TestRequest::get("/articles?include=author")
                    .accept_json_api()
                    .build(),
            )
            .await
            .assert_status(StatusCode::OK)
            .assert_json_api_content_type();

        assert_eq!(response.data().as_array().unwrap().len(), 2);

        // The shared author appears exactly once under `included`.
        let included = response.json()["included"].as_array().unwrap();
        assert_eq!(included.len(), 1);
        assert_eq!(included[0]["type"], "people");
        assert_eq!(included[0]["id"], "9");
        assert_eq!(included[0]["attributes"]["name"], "Dan Gebhardt");
    });
}

#[test]
fn no_include_yields_no_included_member() {
    pollster::block_on(async {
        let response = app()
            .send(TestRequest::get("/articles").accept_json_api().build())
            .await
            .assert_status(StatusCode::OK);

        // An empty `included` array is fine; the point is no author was fetched.
        assert!(
            response
                .json()
                .get("included")
                .and_then(Value::as_array)
                .is_none_or(|included| included.is_empty())
        );
    });
}
