//! Integration tests for server-side query parsing (public API surface).

use jsonapi_core::{Query, SortField, TypeInfo, TypeRegistry};

#[test]
fn parses_a_realistic_query_string() {
    let q = Query::from_query_string(
        "?include=author,comments&sort=-created,title&page[size]=10&filter[status]=published&fields[articles]=title,body",
    )
    .unwrap();
    assert_eq!(
        q.include,
        vec!["author".to_string(), "comments".to_string()]
    );
    assert_eq!(
        q.sort[0],
        SortField {
            field: "created".into(),
            descending: true
        }
    );
    assert_eq!(q.page.get("size").map(String::as_str), Some("10"));
    assert_eq!(
        q.filter.get("status").unwrap(),
        &vec!["published".to_string()]
    );
}

#[test]
fn include_paths_compose_with_type_registry_validation() {
    let q = Query::from_pairs(&[("include", "author")]).unwrap();
    let mut reg = TypeRegistry::new();
    reg.register_info(TypeInfo::new(
        "articles",
        &["title", "author"],
        &[("author", "people")],
    ));
    let paths: Vec<&str> = q.include.iter().map(String::as_str).collect();
    assert!(reg.validate_include_paths("articles", &paths).is_ok());
}

#[test]
fn from_pairs_and_from_query_string_agree() {
    let a = Query::from_pairs(&[("sort", "title"), ("filter[x]", "y")]).unwrap();
    let b = Query::from_query_string("sort=title&filter[x]=y").unwrap();
    assert_eq!(a, b);
}
