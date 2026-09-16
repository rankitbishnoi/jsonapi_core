//! Per-type operation orchestration and request-parsing helpers.
//!
//! Each function receives a live transaction connection and calls
//! `crate::repo::atomic_repo` for the actual SQL. Nothing here touches the
//! HTTP layer beyond constructing `JsonApiError` values.

use std::collections::{BTreeMap, HashMap};

use axum::http::StatusCode;
use jsonapi_axum::{ApiErrorExt, JsonApiError, with_status};
use jsonapi_core::atomic::OperationRef;
use jsonapi_core::{Identity, PrimaryData, RelationshipData, Resource, ResourceIdentifier};
use serde_json::Value;

use crate::repo::atomic_repo;
use crate::util::{mint_id, now};

use super::AtomicResult;

// ── Per-type operation handlers ───────────────────────────────────────────────

pub(super) async fn add_author(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    resource: &Resource,
    lid_map: &mut HashMap<String, String>,
) -> Result<AtomicResult, JsonApiError> {
    let name = string_attr(&resource.attributes, "name", "authors")?;
    let email = string_attr(&resource.attributes, "email", "authors")?;

    let id = mint_id();
    let ts = now();

    let author = atomic_repo::create_author(tx, &id, &name, &email, &ts)
        .await
        .map_err(sqlx_err)?;

    // Record lid -> real id so later ops can reference it.
    if let Some(lid) = &resource.lid {
        lid_map.insert(lid.clone(), author.id.clone());
    }

    let mut attrs = serde_json::Map::new();
    attrs.insert("name".into(), Value::String(author.name));
    attrs.insert("email".into(), Value::String(author.email));
    attrs.insert("createdAt".into(), Value::String(author.created_at));

    Ok(AtomicResult {
        data: Some(PrimaryData::Single(Box::new(Resource {
            r#type: "authors".into(),
            id: Some(author.id),
            lid: None,
            attributes: Value::Object(attrs),
            relationships: BTreeMap::new(),
            links: None,
            meta: None,
        }))),
        meta: None,
        links: None,
    })
}

pub(super) async fn update_author(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    resource: &Resource,
    id: &str,
) -> Result<AtomicResult, JsonApiError> {
    let name = resource.attributes.get("name").and_then(Value::as_str);
    let email = resource.attributes.get("email").and_then(Value::as_str);

    let author = atomic_repo::update_author(tx, id, name, email)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => JsonApiError::from_api_error(
                with_status(StatusCode::NOT_FOUND).detail(format!("author `{id}` not found")),
            ),
            other => sqlx_err(other),
        })?;

    let mut attrs = serde_json::Map::new();
    attrs.insert("name".into(), Value::String(author.name));
    attrs.insert("email".into(), Value::String(author.email));
    attrs.insert("createdAt".into(), Value::String(author.created_at));

    Ok(AtomicResult {
        data: Some(PrimaryData::Single(Box::new(Resource {
            r#type: "authors".into(),
            id: Some(author.id),
            lid: None,
            attributes: Value::Object(attrs),
            relationships: BTreeMap::new(),
            links: None,
            meta: None,
        }))),
        meta: None,
        links: None,
    })
}

pub(super) async fn remove_author(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    id: &str,
) -> Result<AtomicResult, JsonApiError> {
    atomic_repo::remove_author(tx, id)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => JsonApiError::from_api_error(
                with_status(StatusCode::NOT_FOUND).detail(format!("author `{id}` not found")),
            ),
            other => sqlx_err(other),
        })?;
    Ok(AtomicResult::default())
}

pub(super) async fn add_article(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    resource: &Resource,
    lid_map: &mut HashMap<String, String>,
) -> Result<AtomicResult, JsonApiError> {
    let title = string_attr(&resource.attributes, "title", "articles")?;
    let body = string_attr(&resource.attributes, "body", "articles")?;

    // Resolve author identity from relationships.
    let author_id = resolve_author_id(resource, lid_map)?;

    // Honour a client-supplied id; fall back to a server-minted one.
    let id = resource.id.clone().unwrap_or_else(mint_id);
    let ts = now();

    let article = atomic_repo::create_article(tx, &id, &title, &body, &author_id, &ts)
        .await
        .map_err(sqlx_err)?;

    if let Some(lid) = &resource.lid {
        lid_map.insert(lid.clone(), article.id.clone());
    }

    let mut attrs = serde_json::Map::new();
    attrs.insert("title".into(), Value::String(article.title));
    attrs.insert("body".into(), Value::String(article.body));
    attrs.insert("createdAt".into(), Value::String(article.created_at));
    attrs.insert("updatedAt".into(), Value::String(article.updated_at));

    // Build author relationship linkage in the response.
    let author_rid = ResourceIdentifier {
        r#type: "authors".into(),
        identity: Identity::Id(article.author_id),
        meta: None,
    };
    let mut relationships = BTreeMap::new();
    relationships.insert(
        "author".into(),
        jsonapi_core::ResourceRelationship::new(RelationshipData::ToOne(Some(author_rid))),
    );

    Ok(AtomicResult {
        data: Some(PrimaryData::Single(Box::new(Resource {
            r#type: "articles".into(),
            id: Some(article.id),
            lid: None,
            attributes: Value::Object(attrs),
            relationships,
            links: None,
            meta: None,
        }))),
        meta: None,
        links: None,
    })
}

pub(super) async fn update_article(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    resource: &Resource,
    id: &str,
) -> Result<AtomicResult, JsonApiError> {
    let ts = now();
    let title = resource.attributes.get("title").and_then(Value::as_str);
    let body = resource.attributes.get("body").and_then(Value::as_str);

    let article = atomic_repo::update_article(tx, id, title, body, &ts)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => JsonApiError::from_api_error(
                with_status(StatusCode::NOT_FOUND).detail(format!("article `{id}` not found")),
            ),
            other => sqlx_err(other),
        })?;

    let mut attrs = serde_json::Map::new();
    attrs.insert("title".into(), Value::String(article.title));
    attrs.insert("body".into(), Value::String(article.body));
    attrs.insert("createdAt".into(), Value::String(article.created_at));
    attrs.insert("updatedAt".into(), Value::String(article.updated_at));

    let author_rid = ResourceIdentifier {
        r#type: "authors".into(),
        identity: Identity::Id(article.author_id),
        meta: None,
    };
    let mut relationships = BTreeMap::new();
    relationships.insert(
        "author".into(),
        jsonapi_core::ResourceRelationship::new(RelationshipData::ToOne(Some(author_rid))),
    );

    Ok(AtomicResult {
        data: Some(PrimaryData::Single(Box::new(Resource {
            r#type: "articles".into(),
            id: Some(article.id),
            lid: None,
            attributes: Value::Object(attrs),
            relationships,
            links: None,
            meta: None,
        }))),
        meta: None,
        links: None,
    })
}

pub(super) async fn remove_article(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    id: &str,
) -> Result<AtomicResult, JsonApiError> {
    atomic_repo::remove_article(tx, id)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => JsonApiError::from_api_error(
                with_status(StatusCode::NOT_FOUND).detail(format!("article `{id}` not found")),
            ),
            other => sqlx_err(other),
        })?;
    Ok(AtomicResult::default())
}

// ── Request-parsing helpers ───────────────────────────────────────────────────

/// Extract a required string attribute from a `Resource`'s attributes `Value`.
pub(super) fn string_attr(
    attrs: &Value,
    key: &str,
    type_name: &str,
) -> Result<String, JsonApiError> {
    attrs
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            JsonApiError::from_api_error(
                with_status(StatusCode::UNPROCESSABLE_ENTITY)
                    .pointer(format!("/data/attributes/{key}"))
                    .detail(format!("`{key}` is required for {type_name}")),
            )
        })
}

/// Resolve the author id from an article's relationships, honouring lid -> id mapping.
pub(super) fn resolve_author_id(
    resource: &Resource,
    lid_map: &HashMap<String, String>,
) -> Result<String, JsonApiError> {
    let identity = resource
        .relationships
        .get("author")
        .and_then(|r| r.to_one_identity())
        .ok_or_else(|| {
            JsonApiError::unprocessable_with_pointer(
                "/data/relationships/author",
                "article requires an `author` to-one relationship",
            )
        })?;

    match identity {
        Identity::Id(id) => Ok(id.clone()),
        Identity::Lid(lid) => lid_map.get(lid).cloned().ok_or_else(|| {
            JsonApiError::unprocessable_with_pointer(
                "/data/relationships/author/data/lid",
                format!(
                    "unresolved author lid `{lid}`; \
                     ensure the corresponding `add` operation appears earlier in the request"
                ),
            )
        }),
        _ => Err(JsonApiError::unprocessable_with_pointer(
            "/data/relationships/author/data",
            "author identity must be an id or lid",
        )),
    }
}

/// Resolve the target id from an `OperationRef`, checking both `id` and `lid`.
pub(super) fn resolve_target_id(
    op_ref: &OperationRef,
    lid_map: &HashMap<String, String>,
) -> Result<String, JsonApiError> {
    match &op_ref.identity {
        Identity::Id(id) => Ok(id.clone()),
        Identity::Lid(lid) => lid_map.get(lid).cloned().ok_or_else(|| {
            // validate_lid_refs should have caught forward references, but
            // a lid that was introduced as an add but we can't resolve still
            // needs a clear message.
            JsonApiError::unprocessable(format!(
                "lid `{lid}` in target ref could not be resolved to a server id"
            ))
        }),
        _ => Err(JsonApiError::unprocessable(
            "target ref identity must be an id or lid",
        )),
    }
}

/// Validate a client-supplied resource type string as a JSON:API member name.
/// Returns 400 when the name is syntactically invalid — distinct from the 422
/// returned for a valid-but-unsupported type.
pub(super) fn validate_resource_type(type_str: &str, idx: usize) -> Result<(), JsonApiError> {
    jsonapi_core::validate_member_name(type_str).map_err(|_| {
        JsonApiError::from_api_error(
            with_status(StatusCode::BAD_REQUEST)
                .pointer("/data/type")
                .detail(format!(
                    "operation {idx}: `{type_str}` is not a valid JSON:API member name"
                )),
        )
    })?;
    Ok(())
}

/// Map a sqlx error to a JSON:API error: `RowNotFound` → 404, all others → 500.
pub(super) fn sqlx_err(e: sqlx::Error) -> JsonApiError {
    JsonApiError::from(e)
}
