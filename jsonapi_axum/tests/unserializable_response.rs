//! I-1 — a response payload that cannot be serialized to JSON must produce a
//! JSON:API `500` error document, never a panic at the `IntoResponse` boundary.

use std::collections::BTreeMap;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::Request;
use axum::http::{StatusCode, header};
use axum::routing::get;
use tower::ServiceExt;

use jsonapi_axum::{DocumentBuilder, JsonApiResponse};
use serde_json::Value;

/// A resource whose `dimensions` attribute is a map keyed by a tuple. Such a map
/// cannot be encoded as JSON (`serde_json` rejects non-string map keys with
/// "key must be a string"), so serializing this resource fails at runtime — the
/// exact condition I-1 guards against.
#[derive(Debug, Clone, jsonapi_core::JsonApi)]
#[jsonapi(type = "widgets")]
struct Widget {
    #[jsonapi(id)]
    id: String,
    dimensions: BTreeMap<(u8, u8), String>,
}

async fn get_widget() -> JsonApiResponse<Widget> {
    let mut dimensions = BTreeMap::new();
    dimensions.insert((1, 2), "unserializable".to_string());
    JsonApiResponse::new(
        DocumentBuilder::single(Widget {
            id: "1".into(),
            dimensions,
        })
        .build(),
    )
}

fn app() -> Router {
    Router::new().route("/widgets/{id}", get(get_widget))
}

#[test]
fn unserializable_response_yields_jsonapi_500_not_panic() {
    pollster::block_on(async {
        let request = Request::builder()
            .uri("/widgets/1")
            .body(Body::empty())
            .unwrap();
        let response = app().oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok()),
            Some("application/vnd.api+json"),
        );

        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["errors"][0]["status"], "500");
        // Default build (no `debug-errors`): the raw serializer message must not
        // leak into the response body. With `debug-errors`, `internal()`
        // deliberately surfaces the detail (covered in `error.rs`), so this
        // assertion only applies to the default build.
        #[cfg(not(feature = "debug-errors"))]
        assert!(json["errors"][0]["detail"].is_null());
    });
}
