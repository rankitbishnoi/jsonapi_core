//! Handler for `POST /operations` — JSON:API Atomic Operations extension.
//!
//! All operations in the request are executed inside a single SQLite transaction.
//! If any operation fails the whole transaction is rolled back.
//!
//! Per-type orchestration functions and request-parsing helpers live in
//! [`dispatch`].

mod dispatch;

use std::collections::HashMap;

use axum::extract::State;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::Response;
use jsonapi_axum::{ApiErrorExt, JsonApiError, with_status};
use jsonapi_core::PrimaryData;
use jsonapi_core::atomic::{
    ATOMIC_EXT_URI, AtomicOperation, AtomicRequest, AtomicResponse, AtomicResult,
};

use crate::state::AppState;

use dispatch::{
    add_article, add_author, remove_article, remove_author, resolve_target_id, update_article,
    update_author, validate_resource_type,
};

/// Content-Type / Accept value for atomic operations responses.
fn atomic_content_type() -> HeaderValue {
    HeaderValue::from_str(&format!(
        "application/vnd.api+json; ext=\"{ATOMIC_EXT_URI}\""
    ))
    .expect("static header value is valid")
}

// ── Main handler ──────────────────────────────────────────────────────────────

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

// ── Dispatcher ────────────────────────────────────────────────────────────────

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

            // Validate the resource type string before dispatching.
            validate_resource_type(&resource.r#type, idx)?;

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

            // Validate the ref type string before resolving the target id.
            validate_resource_type(&op_ref.r#type, idx)?;
            let id = resolve_target_id(op_ref, lid_map)?;

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

            // Validate the ref type string before resolving the target id.
            validate_resource_type(&op_ref.r#type, idx)?;
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
