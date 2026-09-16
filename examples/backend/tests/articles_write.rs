#[path = "support.rs"]
mod support;

use axum::http::StatusCode;
use jsonapi_axum::testing::{RouterTestExt, TestRequest};
use serde_json::json;

/// POST /articles returns 201 with a Location header pointing at the new resource.
#[tokio::test]
async fn create_returns_201_with_location() {
    let app = support::seeded_app().await;

    let body = json!({
        "data": {
            "type": "articles",
            "attributes": {
                "title": "New Article",
                "body": "Some body text."
            },
            "relationships": {
                "author": {
                    "data": { "type": "authors", "id": "a1" }
                }
            }
        }
    });

    let res = app
        .send(TestRequest::post("/articles").body_json(&body).build())
        .await;

    res.clone().assert_status(StatusCode::CREATED);

    let location = res.header("location").expect("Location header present");
    assert!(
        location.contains("/articles/"),
        "Location should contain /articles/, got: {location}"
    );

    assert_eq!(res.json()["data"]["type"], "articles");
    assert_eq!(res.json()["data"]["attributes"]["title"], "New Article");
}

/// PATCH /articles/art-01 with only `title` must leave `body` unchanged.
#[tokio::test]
async fn patch_partial_update_changes_only_supplied_fields() {
    let app = support::seeded_app().await;

    // art-01 body is "Body of article 1." from the seed fixture
    let original_body = "Body of article 1.";

    let patch_body = json!({
        "data": {
            "type": "articles",
            "id": "art-01",
            "attributes": {
                "title": "Updated Title"
            }
        }
    });

    let res = app
        .send(
            TestRequest::patch("/articles/art-01")
                .body_json(&patch_body)
                .build(),
        )
        .await;

    res.clone().assert_status(StatusCode::OK);
    assert_eq!(res.json()["data"]["attributes"]["title"], "Updated Title");
    // body must not have changed
    assert_eq!(res.json()["data"]["attributes"]["body"], original_body);
}

/// PATCH with a body `id` that differs from the URL id must return 409 Conflict.
#[tokio::test]
async fn patch_with_mismatched_id_is_409() {
    let app = support::seeded_app().await;

    let patch_body = json!({
        "data": {
            "type": "articles",
            "id": "art-99",
            "attributes": {
                "title": "Mismatched"
            }
        }
    });

    let res = app
        .send(
            TestRequest::patch("/articles/art-01")
                .body_json(&patch_body)
                .build(),
        )
        .await;

    res.assert_status(StatusCode::CONFLICT).assert_error(409);
}

/// POST /articles with both title and body empty returns 422 with >=2 aggregated errors.
#[tokio::test]
async fn create_with_multiple_invalid_fields_returns_aggregated_422() {
    let app = support::seeded_app().await;
    let res = app
        .send(
            TestRequest::post("/articles")
                .body_json(&json!({
                    "data": {
                        "type": "articles",
                        "attributes": { "title": "", "body": "" },
                        "relationships": {
                            "author": { "data": { "type": "authors", "id": "a1" } }
                        }
                    }
                }))
                .build(),
        )
        .await;

    let res = res.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    let errors = res.errors().as_array().expect("errors array");
    assert!(
        errors.len() >= 2,
        "expected >=2 aggregated errors, got {}",
        errors.len()
    );
    // Each error must carry a source.pointer to the offending attribute.
    assert!(
        errors.iter().all(|e| e["source"]["pointer"].is_string()),
        "every error must have source.pointer"
    );
}

/// DELETE /articles/art-01 returns 204; subsequent GET returns 404.
#[tokio::test]
async fn delete_returns_204_then_404() {
    let app = support::seeded_app().await;

    let res = app
        .clone()
        .send(TestRequest::delete("/articles/art-01").build())
        .await;
    res.assert_status(StatusCode::NO_CONTENT);

    let res = app
        .send(
            TestRequest::get("/articles/art-01")
                .accept_json_api()
                .build(),
        )
        .await;
    res.assert_status(StatusCode::NOT_FOUND).assert_error(404);
}
