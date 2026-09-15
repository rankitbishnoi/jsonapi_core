//! Resource-`id` consistency and client-generated-id policy (JSON:API write
//! rules), as framework-agnostic checks producing typed [`ApiError`]s.
//!
//! The adapter binds these onto the request (comparing a body against an
//! `axum::extract::Path` id); the rules themselves — and the exact status and
//! `source.pointer` each produces — live here so every adapter behaves alike.

use http::StatusCode;

use jsonapi_core::{ApiError, ErrorSource};

/// Policy for a client-supplied `id` on a **create** (`POST`).
///
/// JSON:API lets a server choose whether clients may assign resource ids.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClientIdPolicy {
    /// The server owns identity: any client-supplied `id` is ignored (the
    /// handler assigns one). This is the default.
    #[default]
    Assign,
    /// Client-supplied ids are accepted as-is.
    Accept,
    /// Client-supplied ids are unsupported: a request carrying one is rejected
    /// with **403 Forbidden**.
    Forbid,
}

/// Build an [`ApiError`] with `status`, its canonical reason as `title`, a
/// `detail`, and a `source.pointer`.
fn pointer_error(status: StatusCode, detail: String, pointer: &str) -> ApiError {
    ApiError {
        status: Some(status.as_u16().to_string()),
        title: status.canonical_reason().map(str::to_string),
        detail: Some(detail),
        source: Some(ErrorSource {
            pointer: Some(pointer.to_string()),
            ..Default::default()
        }),
        ..Default::default()
    }
}

/// Assert a request body's `data.id` matches the endpoint (URL) id, per the
/// JSON:API `PATCH` rule.
///
/// - `body_id == path_id` → `Ok`.
/// - `body_id != path_id` → **409 Conflict** with `source.pointer` `/data/id`.
/// - `body_id` absent (`None`) → `Ok`: the handler takes identity from the URL.
///   (A required `id` field on the body type makes absence impossible anyway.)
///
/// # Errors
/// Returns a 409 [`ApiError`] when a present body id differs from `path_id`.
/// Boxed because [`ApiError`] is large (`clippy::result_large_err`), matching
/// [`deserialize_body`](crate::deserialize_body).
pub fn check_id_matches(body_id: Option<&str>, path_id: &str) -> Result<(), Box<ApiError>> {
    match body_id {
        Some(id) if id != path_id => Err(Box::new(pointer_error(
            StatusCode::CONFLICT,
            format!("resource `id` \"{id}\" does not match the endpoint id \"{path_id}\""),
            "/data/id",
        ))),
        _ => Ok(()),
    }
}

/// Apply a [`ClientIdPolicy`] to a create request's client-supplied `id`.
///
/// Under [`ClientIdPolicy::Forbid`], a present `body_id` is rejected with
/// **403 Forbidden** (`source.pointer` `/data/id`). [`Assign`](ClientIdPolicy::Assign)
/// and [`Accept`](ClientIdPolicy::Accept) always succeed — enforcing "assign"
/// (ignoring the id) is the handler's job.
///
/// # Errors
/// Returns a 403 [`ApiError`] when the policy is `Forbid` and a client id is
/// present. Boxed for the same reason as [`check_id_matches`].
pub fn check_client_id(
    policy: ClientIdPolicy,
    body_id: Option<&str>,
) -> Result<(), Box<ApiError>> {
    match (policy, body_id) {
        (ClientIdPolicy::Forbid, Some(id)) => Err(Box::new(pointer_error(
            StatusCode::FORBIDDEN,
            format!("client-generated ids are not supported; remove `id` \"{id}\""),
            "/data/id",
        ))),
        _ => Ok(()),
    }
}

/// Build a **409 Conflict** [`ApiError`] for an id collision (a client-supplied
/// or otherwise chosen id that already exists). Collision *detection* is the
/// application's job; this produces the spec-shaped error to return.
#[must_use]
pub fn id_conflict(detail: impl Into<String>) -> ApiError {
    pointer_error(StatusCode::CONFLICT, detail.into(), "/data/id")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_id_is_ok() {
        assert!(check_id_matches(Some("1"), "1").is_ok());
    }

    #[test]
    fn absent_body_id_is_ok() {
        assert!(check_id_matches(None, "1").is_ok());
    }

    #[test]
    fn mismatched_id_is_409_with_pointer() {
        let err = check_id_matches(Some("2"), "1").unwrap_err();
        assert_eq!(err.status.as_deref(), Some("409"));
        assert_eq!(
            err.source.as_ref().and_then(|s| s.pointer.as_deref()),
            Some("/data/id")
        );
        assert!(err.detail.as_deref().unwrap().contains("\"2\""));
    }

    #[test]
    fn forbid_policy_rejects_client_id_with_403() {
        let err = check_client_id(ClientIdPolicy::Forbid, Some("client-1")).unwrap_err();
        assert_eq!(err.status.as_deref(), Some("403"));
        assert_eq!(
            err.source.as_ref().and_then(|s| s.pointer.as_deref()),
            Some("/data/id")
        );
    }

    #[test]
    fn forbid_policy_allows_absent_client_id() {
        assert!(check_client_id(ClientIdPolicy::Forbid, None).is_ok());
    }

    #[test]
    fn assign_and_accept_always_ok() {
        assert!(check_client_id(ClientIdPolicy::Assign, Some("x")).is_ok());
        assert!(check_client_id(ClientIdPolicy::Accept, Some("x")).is_ok());
    }

    #[test]
    fn id_conflict_is_409_with_pointer() {
        let err = id_conflict("id `1` already exists");
        assert_eq!(err.status.as_deref(), Some("409"));
        assert_eq!(
            err.source.as_ref().and_then(|s| s.pointer.as_deref()),
            Some("/data/id")
        );
    }
}
