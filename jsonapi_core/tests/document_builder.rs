//! Integration tests for DocumentBuilder and the Document constructors.

use jsonapi_core::{ApiError, Document, DocumentBuilder, Link, Resource};

fn res(type_: &str, id: &str) -> Resource {
    Resource::new(type_)
        .with_id(id)
        .with_attributes(serde_json::json!({"k": "v"}))
}

#[test]
fn builds_and_serializes_a_compound_document() {
    let doc = DocumentBuilder::single(res("articles", "1"))
        .include(res("people", "9"))
        .link("self", Link::String("/articles/1".into()))
        .build();

    let v = serde_json::to_value(&doc).unwrap();
    assert_eq!(v["data"]["type"], "articles");
    assert_eq!(v["included"][0]["type"], "people");
    assert_eq!(v["links"]["self"], "/articles/1");
}

#[test]
fn errors_document_serializes() {
    let doc: Document<Resource> = Document::errors([ApiError::new()
        .with_status("422")
        .with_title("Unprocessable")]);
    let v = serde_json::to_value(&doc).unwrap();
    assert_eq!(v["errors"][0]["status"], "422");
    assert!(v.get("data").is_none());
}
