//! The `links` URL/`Links` helpers are part of the 1.0 public API and must be
//! reachable from the crate root (adapters such as `jsonapi_axum` rely on them).

#[test]
fn links_helpers_are_reachable_from_root() {
    assert_eq!(
        jsonapi_core::resource_self("https://x", "posts", "1"),
        jsonapi_core::links::resource_self("https://x", "posts", "1")
    );
    assert_eq!(
        jsonapi_core::collection_self("https://x", "posts"),
        jsonapi_core::links::collection_self("https://x", "posts")
    );
    assert_eq!(
        jsonapi_core::relationship_self("https://x", "posts", "1", "author"),
        jsonapi_core::links::relationship_self("https://x", "posts", "1", "author")
    );
    assert_eq!(
        jsonapi_core::relationship_related("https://x", "posts", "1", "author"),
        jsonapi_core::links::relationship_related("https://x", "posts", "1", "author")
    );
    assert_eq!(
        jsonapi_core::relationship_links("https://x", "posts", "1", "author"),
        jsonapi_core::links::relationship_links("https://x", "posts", "1", "author")
    );
    assert_eq!(
        jsonapi_core::resource_self_links("https://x", "posts", "1"),
        jsonapi_core::links::resource_self_links("https://x", "posts", "1")
    );
    assert_eq!(
        jsonapi_core::collection_self_links("https://x", "posts"),
        jsonapi_core::links::collection_self_links("https://x", "posts")
    );
}
