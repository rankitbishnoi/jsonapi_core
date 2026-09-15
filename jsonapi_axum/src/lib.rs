//! [axum](https://docs.rs/axum) adapter for [`jsonapi_core`].
//!
//! This crate is a thin binding: it provides axum's `FromRequest` /
//! `FromRequestParts` / `IntoResponse` impls, delegating all JSON:API logic to
//! [`jsonapi_http`]. If a type here grows real logic, it belongs in
//! `jsonapi_http` instead so other framework adapters can reuse it.
//!
//! # Module map
//!
//! - [`error`] — [`JsonApiError`], the shared rejection + error responder.
//! - [`response`] — [`JsonApiResponse`], an `IntoResponse` wrapper for
//!   documents.
//! - [`extract`] — [`JsonApi`], [`JsonApiQuery`], [`JsonApiQueryValidated`]
//!   extractors.
//! - [`normalize`] — [`not_found`] fallback and [`NormalizeErrorsLayer`] to
//!   JSON:API-shape framework-generated error responses.
//! - [`request_id`] — [`RequestIdLayer`] + [`RequestId`] extractor for
//!   correlation ids on error documents (`errors[].id`) and the `x-request-id`
//!   response header.
//!
//! The framework-agnostic tower layers from [`jsonapi_http`] and the most-used
//! [`jsonapi_core`] types are re-exported so handler code can import from
//! `jsonapi_axum` alone.
#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod error;
pub mod extract;
pub mod normalize;
pub mod pagination;
pub mod relationship;
pub mod request_id;
pub mod response;
#[cfg(any(test, feature = "testing"))]
#[cfg_attr(docsrs, doc(cfg(feature = "testing")))]
pub mod testing;
#[cfg(feature = "validator")]
#[cfg_attr(docsrs, doc(cfg(feature = "validator")))]
pub mod validation;

pub use error::{IntoJsonApiError, JsonApiError, ResultExt};
pub use extract::{BaseUrl, JsonApi, JsonApiQuery, JsonApiQueryValidated, NegotiatedMediaType};
pub use normalize::{NormalizeErrorsLayer, NormalizeErrorsService, not_found};
pub use pagination::pagination_links;
pub use relationship::{JsonApiToMany, JsonApiToOne, RelationshipResponse};
pub use request_id::{RequestId, RequestIdLayer, RequestIdService};
pub use response::JsonApiResponse;
#[cfg(feature = "validator")]
#[cfg_attr(docsrs, doc(cfg(feature = "validator")))]
pub use validation::from_validation_errors;

// Re-export the framework-agnostic tower layers so axum users get them from one
// place.
pub use jsonapi_http::{AcceptLayer, ContentTypeLayer, JsonApiLayer};

/// Re-export the fluent [`ApiError`] builder surface so handlers can construct
/// JSON:API errors without importing `jsonapi_http` directly.
pub use jsonapi_http::{ApiErrorExt, ApiErrors, with_status};

/// Re-export the client-id policy so handlers can configure
/// [`JsonApi::check_client_id`](crate::JsonApi::check_client_id) from one place.
pub use jsonapi_http::ClientIdPolicy;

/// Re-export the compound-document `include` resolver so a handler can
/// assemble a deduped `included` array from the requested include paths and a
/// consumer-supplied batch loader.
pub use jsonapi_http::{IncludeResolver, resolve_includes};

/// Re-exports of the [`jsonapi_core`] types most commonly needed to build
/// handlers, so handler bodies can import from `jsonapi_axum` alone.
///
/// **Deriving [`JsonApi`](jsonapi_core::JsonApi) still requires a direct
/// `jsonapi_core` dependency:** the derive macro expands to absolute
/// `::jsonapi_core::…` paths, so the crate must be nameable in the consumer's
/// crate. Only the runtime types are re-exported here.
pub use jsonapi_core::{
    ApiError, CURSOR_PAGINATION_PROFILE, CursorLinks, CursorPage, Document, DocumentBuilder, Field,
    JsonApiMediaType, Query, Relationship, Resource, SortField, TypeRegistry,
};

/// The full [`jsonapi_core`] crate, for any type not re-exported above.
pub use jsonapi_core;
