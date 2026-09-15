//! G8 — self-link generation via the `BaseUrl` state extractor + core link
//! assembly, exercised through a router with `oneshot`.

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::{FromRef, Path, State};
use axum::http::{Request, StatusCode};
use axum::routing::get;
use tower::ServiceExt;

use jsonapi_axum::{BaseUrl, DocumentBuilder, JsonApiResponse};
use jsonapi_core::{Link, links};
use serde_json::Value;

#[derive(Debug, Clone, jsonapi_core::JsonApi)]
#[jsonapi(type = "articles")]
struct Article {
    #[jsonapi(id)]
    id: String,
    title: String,
}

#[derive(Clone)]
struct AppState {
    base_url: BaseUrl,
}

impl FromRef<AppState> for BaseUrl {
    fn from_ref(state: &AppState) -> BaseUrl {
        state.base_url.clone()
    }
}

async fn get_article(
    State(_state): State<AppState>,
    BaseUrl(base): BaseUrl,
    Path(id): Path<String>,
) -> JsonApiResponse<Article> {
    let article = Article {
        id: id.clone(),
        title: "Hi".into(),
    };
    // Attach a top-level self link + a per-relationship links object using the
    // core assembly functions and the app-configured base URL.
    let self_url = links::resource_self(&base, "articles", &id);
    JsonApiResponse::new(
        DocumentBuilder::single(article)
            .link("self", Link::String(self_url))
            .build(),
    )
}

fn app() -> Router {
    Router::new()
        .route("/articles/{id}", get(get_article))
        .with_state(AppState {
            base_url: BaseUrl("https://api.test".into()),
        })
}

#[test]
fn resource_response_carries_self_link_from_base_url() {
    pollster::block_on(async {
        let request = Request::builder()
            .uri("/articles/1")
            .body(Body::empty())
            .unwrap();
        let response = app().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["links"]["self"], "https://api.test/articles/1");
        assert_eq!(json["data"]["attributes"]["title"], "Hi");
    });
}
