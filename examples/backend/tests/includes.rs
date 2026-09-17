#[path = "support.rs"]
mod support;
use axum::http::StatusCode;
use jsonapi_axum::testing::{RouterTestExt, TestRequest};

#[tokio::test]
async fn include_author_and_tags_produces_compound_document() {
    let app = support::seeded_app().await;
    let res = app
        .send(
            TestRequest::get("/articles/art-01?include=author,tags")
                .accept_json_api()
                .build(),
        )
        .await;
    let res = res.assert_status(StatusCode::OK);
    let included = res.json()["included"]
        .as_array()
        .expect("included array")
        .clone();
    let types: Vec<&str> = included
        .iter()
        .map(|r| r["type"].as_str().unwrap())
        .collect();
    assert!(types.contains(&"authors"));
    assert!(types.contains(&"tags"));
    // art-01: 1 author + 2 tags = 3 unique included resources, deduped by (type, id)
    assert_eq!(included.len(), 3);
}

#[tokio::test]
async fn transitive_include_comments_author_resolves() {
    let app = support::seeded_app().await;
    let res = app
        .send(
            TestRequest::get("/articles/art-01?include=comments.author")
                .accept_json_api()
                .build(),
        )
        .await;
    let res = res.assert_status(StatusCode::OK);
    let types: Vec<String> = res.json()["included"]
        .as_array()
        .expect("included array")
        .iter()
        .map(|r| r["type"].as_str().unwrap().to_string())
        .collect();
    assert!(types.contains(&"comments".to_string()));
    assert!(types.contains(&"authors".to_string()));
}

#[tokio::test]
async fn no_include_has_linkage_but_no_included() {
    let app = support::seeded_app().await;
    let res = app
        .send(
            TestRequest::get("/articles/art-01")
                .accept_json_api()
                .build(),
        )
        .await;
    let res = res.assert_status(StatusCode::OK);
    // Relationship linkage is present even without ?include (linkage is independent
    // of inclusion).
    assert_eq!(
        res.json()["data"]["relationships"]["author"]["data"]["type"],
        "authors"
    );
    assert!(
        !res.json()["data"]["relationships"]["tags"]["data"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    // No `included` key when nothing was requested.
    assert!(res.json().get("included").is_none());
}
