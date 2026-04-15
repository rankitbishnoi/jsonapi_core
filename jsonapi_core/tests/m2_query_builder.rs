//! Integration tests for M2: Query Builder.
//!
//! Tests the public API surface as an external consumer would use it.

use jsonapi_core::QueryBuilder;

#[test]
fn realistic_jsonapi_query() {
    let qs = QueryBuilder::new()
        .filter("author", "1")
        .filter("status", "published")
        .include(&["author", "comments", "comments.author"])
        .fields("articles", &["title", "body", "created"])
        .fields("people", &["name"])
        .sort(&["-created", "title"])
        .page("size", "25")
        .page("number", "1")
        .build();

    assert_eq!(
        qs,
        "filter[author]=1\
         &filter[status]=published\
         &include=author,comments,comments.author\
         &fields[articles]=title,body,created\
         &fields[people]=name\
         &sort=-created,title\
         &page[size]=25\
         &page[number]=1"
    );
}

#[test]
fn query_with_encoded_values() {
    let qs = QueryBuilder::new()
        .filter("title", "hello world")
        .filter("tag", "rust&go")
        .param("callback", "http://example.com/cb?x=1")
        .build();

    assert_eq!(
        qs,
        "filter[title]=hello%20world\
         &filter[tag]=rust%26go\
         &callback=http%3A%2F%2Fexample.com%2Fcb%3Fx%3D1"
    );
}

#[test]
fn default_trait() {
    let qs = QueryBuilder::default().build();
    assert_eq!(qs, "");
}
