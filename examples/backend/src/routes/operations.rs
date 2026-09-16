//! Handler for `POST /operations` — JSON:API Atomic Operations extension.
//!
//! All operations in the request are executed inside a single SQLite transaction.
//! If any operation fails the whole transaction is rolled back.

use std::collections::{BTreeMap, HashMap};

use axum::extract::State;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::Response;
use jsonapi_axum::{ApiErrorExt, JsonApiError, with_status};
use jsonapi_core::atomic::{
    ATOMIC_EXT_URI, AtomicOperation, AtomicRequest, AtomicResponse, AtomicResult,
};
use jsonapi_core::{Identity, PrimaryData, RelationshipData, Resource, ResourceIdentifier};
use serde_json::Value;
use sqlx::SqliteConnection;

use crate::state::AppState;
use crate::util::{mint_id, now};

/// Content-Type / Accept value for atomic operations responses.
fn atomic_content_type() -> HeaderValue {
    HeaderValue::from_str(&format!(
        "application/vnd.api+json; ext=\"{ATOMIC_EXT_URI}\""
    ))
    .expect("static header value is valid")
}

// ── Per-type operations on &mut SqliteConnection ─────────────────────────────

async fn add_author(
    conn: &mut SqliteConnection,
    resource: &Resource,
    lid_map: &mut HashMap<String, String>,
) -> Result<AtomicResult, JsonApiError> {
    let name = string_attr(&resource.attributes, "name", "authors")?;
    let email = string_attr(&resource.attributes, "email", "authors")?;

    let id = mint_id();
    let ts = now();

    sqlx::query("INSERT INTO authors (id, name, email, created_at) VALUES (?, ?, ?, ?)")
        .bind(&id)
        .bind(&name)
        .bind(&email)
        .bind(&ts)
        .execute(&mut *conn)
        .await
        .map_err(sqlx_err)?;

    // Record lid -> real id so later ops can reference it.
    if let Some(lid) = &resource.lid {
        lid_map.insert(lid.clone(), id.clone());
    }

    let mut attrs = serde_json::Map::new();
    attrs.insert("name".into(), Value::String(name));
    attrs.insert("email".into(), Value::String(email));
    attrs.insert("createdAt".into(), Value::String(ts));

    Ok(AtomicResult {
        data: Some(PrimaryData::Single(Box::new(Resource {
            r#type: "authors".into(),
            id: Some(id),
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

async fn update_author(
    conn: &mut SqliteConnection,
    resource: &Resource,
    id: &str,
) -> Result<AtomicResult, JsonApiError> {
    if let Some(name) = resource.attributes.get("name").and_then(Value::as_str) {
        sqlx::query("UPDATE authors SET name = ? WHERE id = ?")
            .bind(name)
            .bind(id)
            .execute(&mut *conn)
            .await
            .map_err(sqlx_err)?;
    }
    if let Some(email) = resource.attributes.get("email").and_then(Value::as_str) {
        sqlx::query("UPDATE authors SET email = ? WHERE id = ?")
            .bind(email)
            .bind(id)
            .execute(&mut *conn)
            .await
            .map_err(sqlx_err)?;
    }

    let updated = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT id, name, email, created_at FROM authors WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&mut *conn)
    .await
    .map_err(sqlx_err)?
    .ok_or_else(|| {
        JsonApiError::from_api_error(
            with_status(StatusCode::NOT_FOUND).detail(format!("author `{id}` not found")),
        )
    })?;

    let (rid, name, email, created_at) = updated;
    let mut attrs = serde_json::Map::new();
    attrs.insert("name".into(), Value::String(name));
    attrs.insert("email".into(), Value::String(email));
    attrs.insert("createdAt".into(), Value::String(created_at));

    Ok(AtomicResult {
        data: Some(PrimaryData::Single(Box::new(Resource {
            r#type: "authors".into(),
            id: Some(rid),
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

async fn remove_author(
    conn: &mut SqliteConnection,
    id: &str,
) -> Result<AtomicResult, JsonApiError> {
    let result = sqlx::query("DELETE FROM authors WHERE id = ?")
        .bind(id)
        .execute(&mut *conn)
        .await
        .map_err(sqlx_err)?;

    if result.rows_affected() == 0 {
        return Err(JsonApiError::from_api_error(
            with_status(StatusCode::NOT_FOUND).detail(format!("author `{id}` not found")),
        ));
    }
    Ok(AtomicResult::default())
}

async fn add_article(
    conn: &mut SqliteConnection,
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

    sqlx::query(
        "INSERT INTO articles (id, title, body, author_id, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&title)
    .bind(&body)
    .bind(&author_id)
    .bind(&ts)
    .bind(&ts)
    .execute(&mut *conn)
    .await
    .map_err(sqlx_err)?;

    if let Some(lid) = &resource.lid {
        lid_map.insert(lid.clone(), id.clone());
    }

    let mut attrs = serde_json::Map::new();
    attrs.insert("title".into(), Value::String(title));
    attrs.insert("body".into(), Value::String(body));
    attrs.insert("createdAt".into(), Value::String(ts.clone()));
    attrs.insert("updatedAt".into(), Value::String(ts));

    // Build author relationship linkage in the response.
    let author_rid = ResourceIdentifier {
        r#type: "authors".into(),
        identity: Identity::Id(author_id),
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
            id: Some(id),
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

async fn update_article(
    conn: &mut SqliteConnection,
    resource: &Resource,
    id: &str,
) -> Result<AtomicResult, JsonApiError> {
    let ts = now();

    if let Some(title) = resource.attributes.get("title").and_then(Value::as_str) {
        sqlx::query("UPDATE articles SET title = ?, updated_at = ? WHERE id = ?")
            .bind(title)
            .bind(&ts)
            .bind(id)
            .execute(&mut *conn)
            .await
            .map_err(sqlx_err)?;
    }
    if let Some(body) = resource.attributes.get("body").and_then(Value::as_str) {
        sqlx::query("UPDATE articles SET body = ?, updated_at = ? WHERE id = ?")
            .bind(body)
            .bind(&ts)
            .bind(id)
            .execute(&mut *conn)
            .await
            .map_err(sqlx_err)?;
    }

    let updated = sqlx::query_as::<_, (String, String, String, String, String, String)>(
        "SELECT id, title, body, author_id, created_at, updated_at FROM articles WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&mut *conn)
    .await
    .map_err(sqlx_err)?
    .ok_or_else(|| {
        JsonApiError::from_api_error(
            with_status(StatusCode::NOT_FOUND).detail(format!("article `{id}` not found")),
        )
    })?;

    let (rid, title, body, author_id, created_at, updated_at) = updated;
    let mut attrs = serde_json::Map::new();
    attrs.insert("title".into(), Value::String(title));
    attrs.insert("body".into(), Value::String(body));
    attrs.insert("createdAt".into(), Value::String(created_at));
    attrs.insert("updatedAt".into(), Value::String(updated_at));

    let author_rid = ResourceIdentifier {
        r#type: "authors".into(),
        identity: Identity::Id(author_id),
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
            id: Some(rid),
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

async fn remove_article(
    conn: &mut SqliteConnection,
    id: &str,
) -> Result<AtomicResult, JsonApiError> {
    let result = sqlx::query("DELETE FROM articles WHERE id = ?")
        .bind(id)
        .execute(&mut *conn)
        .await
        .map_err(sqlx_err)?;

    if result.rows_affected() == 0 {
        return Err(JsonApiError::from_api_error(
            with_status(StatusCode::NOT_FOUND).detail(format!("article `{id}` not found")),
        ));
    }
    Ok(AtomicResult::default())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Extract a required string attribute from a `Resource`'s attributes `Value`.
fn string_attr(attrs: &Value, key: &str, type_name: &str) -> Result<String, JsonApiError> {
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
fn resolve_author_id(
    resource: &Resource,
    lid_map: &HashMap<String, String>,
) -> Result<String, JsonApiError> {
    let rel = resource.relationships.get("author").ok_or_else(|| {
        JsonApiError::from_api_error(
            with_status(StatusCode::UNPROCESSABLE_ENTITY)
                .pointer("/data/relationships/author")
                .detail("articles require an `author` relationship"),
        )
    })?;

    let rid = rel
        .data
        .as_ref()
        .and_then(|d| {
            if let RelationshipData::ToOne(Some(r)) = d {
                Some(r)
            } else {
                None
            }
        })
        .ok_or_else(|| {
            JsonApiError::from_api_error(
                with_status(StatusCode::UNPROCESSABLE_ENTITY)
                    .pointer("/data/relationships/author/data")
                    .detail("author must be a to-one resource identifier"),
            )
        })?;

    match &rid.identity {
        Identity::Id(id) => Ok(id.clone()),
        Identity::Lid(lid) => lid_map.get(lid).cloned().ok_or_else(|| {
            JsonApiError::from_api_error(
                with_status(StatusCode::UNPROCESSABLE_ENTITY)
                    .pointer("/data/relationships/author/data/lid")
                    .detail(format!(
                        "lid `{lid}` in author relationship has not been resolved; \
                         ensure the corresponding `add` operation appears earlier in the request"
                    )),
            )
        }),
        _ => Err(JsonApiError::from_api_error(
            with_status(StatusCode::UNPROCESSABLE_ENTITY)
                .detail("author identity must be an id or lid".to_string()),
        )),
    }
}

/// Resolve the target id from an `OperationTarget`, checking both `id` and `lid`.
fn resolve_target_id(
    op_ref: &jsonapi_core::atomic::OperationRef,
    lid_map: &HashMap<String, String>,
) -> Result<String, JsonApiError> {
    match &op_ref.identity {
        Identity::Id(id) => Ok(id.clone()),
        Identity::Lid(lid) => lid_map.get(lid).cloned().ok_or_else(|| {
            // validate_lid_refs should have caught forward references, but
            // a lid that was introduced as an add but we can't resolve still
            // needs a clear message.
            JsonApiError::from_api_error(with_status(StatusCode::UNPROCESSABLE_ENTITY).detail(
                format!("lid `{lid}` in target ref could not be resolved to a server id"),
            ))
        }),
        _ => Err(JsonApiError::from_api_error(
            with_status(StatusCode::UNPROCESSABLE_ENTITY)
                .detail("target ref identity must be an id or lid".to_string()),
        )),
    }
}

/// Map a sqlx error to a JSON:API error: `RowNotFound` → 404, all others → 500
/// (message scrubbed from the response body).
fn sqlx_err(e: sqlx::Error) -> JsonApiError {
    JsonApiError::from(e)
}

// ── Main handler ─────────────────────────────────────────────────────────────

pub async fn operations(
    State(state): State<AppState>,
    body: axum::body::Bytes,
) -> Result<Response, JsonApiError> {
    // 1. Parse.
    let request: AtomicRequest = serde_json::from_slice(&body).map_err(|e| {
        JsonApiError::from_api_error(with_status(StatusCode::BAD_REQUEST).detail(e.to_string()))
    })?;

    // 2. Validate lid ref ordering (forward references, duplicates, etc.).
    request.validate_lid_refs().map_err(JsonApiError::from)?;

    // 3. Begin transaction.
    let mut tx = state.pool.begin().await.map_err(JsonApiError::from)?;

    let mut lid_map: HashMap<String, String> = HashMap::new();
    let mut results: Vec<AtomicResult> = Vec::with_capacity(request.operations.len());

    for (idx, op) in request.operations.iter().enumerate() {
        let result = execute_op(&mut tx, op, &mut lid_map, idx).await;

        match result {
            Ok(atomic_result) => results.push(atomic_result),
            Err(err) => {
                // Roll back and surface the error.
                let _ = tx.rollback().await;
                return Err(err);
            }
        }
    }

    // 4. Commit.
    tx.commit().await.map_err(JsonApiError::from)?;

    // 5. Serialize response.
    let response_body = AtomicResponse {
        results,
        ..Default::default()
    };
    let bytes =
        serde_json::to_vec(&response_body).map_err(|e| JsonApiError::internal(e.to_string()))?;

    let mut response = Response::new(axum::body::Body::from(bytes));
    *response.status_mut() = StatusCode::OK;
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, atomic_content_type());

    Ok(response)
}

/// Execute a single atomic operation on the transaction connection.
async fn execute_op(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    op: &AtomicOperation,
    lid_map: &mut HashMap<String, String>,
    idx: usize,
) -> Result<AtomicResult, JsonApiError> {
    match op {
        AtomicOperation::Add { target, data } => {
            // We only support top-level creation (no target.ref and no target.href).
            if target.r#ref.is_some() {
                return Err(JsonApiError::from_api_error(
                    with_status(StatusCode::UNPROCESSABLE_ENTITY).detail(format!(
                        "operation {idx}: `add` to a relationship target is not supported; \
                             only top-level resource creation (empty target) is supported"
                    )),
                ));
            }
            if target.href.is_some() {
                return Err(JsonApiError::from_api_error(
                    with_status(StatusCode::UNPROCESSABLE_ENTITY)
                        .detail(format!("operation {idx}: `href` targets are not supported")),
                ));
            }

            let resource = match data {
                PrimaryData::Single(r) => r.as_ref(),
                _ => {
                    return Err(JsonApiError::from_api_error(
                        with_status(StatusCode::UNPROCESSABLE_ENTITY).detail(format!(
                            "operation {idx}: `add` data must be a single resource"
                        )),
                    ));
                }
            };

            match resource.r#type.as_str() {
                "authors" => add_author(tx, resource, lid_map).await,
                "articles" => add_article(tx, resource, lid_map).await,
                other => Err(JsonApiError::from_api_error(
                    with_status(StatusCode::UNPROCESSABLE_ENTITY).detail(format!(
                        "operation {idx}: unsupported resource type `{other}` for `add`"
                    )),
                )),
            }
        }

        AtomicOperation::Update { target, data } => {
            let op_ref = target.r#ref.as_ref().ok_or_else(|| {
                JsonApiError::from_api_error(
                    with_status(StatusCode::UNPROCESSABLE_ENTITY)
                        .detail(format!("operation {idx}: `update` requires a `ref` target")),
                )
            })?;

            if op_ref.relationship.is_some() {
                return Err(JsonApiError::from_api_error(
                    with_status(StatusCode::UNPROCESSABLE_ENTITY).detail(format!(
                        "operation {idx}: `update` targeting a relationship is not supported"
                    )),
                ));
            }

            let id = resolve_target_id(op_ref, lid_map)?;

            let resource = match data {
                PrimaryData::Single(r) => r.as_ref(),
                _ => {
                    return Err(JsonApiError::from_api_error(
                        with_status(StatusCode::UNPROCESSABLE_ENTITY).detail(format!(
                            "operation {idx}: `update` data must be a single resource"
                        )),
                    ));
                }
            };

            match op_ref.r#type.as_str() {
                "authors" => update_author(tx, resource, &id).await,
                "articles" => update_article(tx, resource, &id).await,
                other => Err(JsonApiError::from_api_error(
                    with_status(StatusCode::UNPROCESSABLE_ENTITY).detail(format!(
                        "operation {idx}: unsupported resource type `{other}` for `update`"
                    )),
                )),
            }
        }

        AtomicOperation::Remove { target } => {
            let op_ref = target.r#ref.as_ref().ok_or_else(|| {
                if target.href.is_some() {
                    JsonApiError::from_api_error(
                        with_status(StatusCode::UNPROCESSABLE_ENTITY).detail(format!(
                            "operation {idx}: `remove` with `href` target is not supported"
                        )),
                    )
                } else {
                    JsonApiError::from_api_error(
                        with_status(StatusCode::UNPROCESSABLE_ENTITY)
                            .detail(format!("operation {idx}: `remove` requires a `ref` target")),
                    )
                }
            })?;

            if op_ref.relationship.is_some() {
                return Err(JsonApiError::from_api_error(
                    with_status(StatusCode::UNPROCESSABLE_ENTITY).detail(format!(
                        "operation {idx}: `remove` targeting a relationship is not supported"
                    )),
                ));
            }

            let id = resolve_target_id(op_ref, lid_map)?;

            match op_ref.r#type.as_str() {
                "authors" => remove_author(tx, &id).await,
                "articles" => remove_article(tx, &id).await,
                other => Err(JsonApiError::from_api_error(
                    with_status(StatusCode::UNPROCESSABLE_ENTITY).detail(format!(
                        "operation {idx}: unsupported resource type `{other}` for `remove`"
                    )),
                )),
            }
        }

        // AtomicOperation is #[non_exhaustive]; future variants fall through here.
        _ => Err(JsonApiError::from_api_error(
            with_status(StatusCode::UNPROCESSABLE_ENTITY)
                .detail(format!("operation {idx}: unsupported operation type")),
        )),
    }
}
