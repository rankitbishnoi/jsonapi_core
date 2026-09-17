use axum::response::IntoResponse;
use jsonapi_axum::JsonApiResponse;
use jsonapi_core::{Document, Resource};

/// `GET /health` — returns a meta-only JSON:API document with `meta.status = "ok"`.
pub async fn health() -> impl IntoResponse {
    let mut meta = serde_json::Map::new();
    meta.insert("status".to_string(), serde_json::json!("ok"));
    let doc: Document<Resource> = Document::meta_only(meta);
    JsonApiResponse::new(doc)
}
