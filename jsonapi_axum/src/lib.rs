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
//!
//! The framework-agnostic tower layers from [`jsonapi_http`] and the most-used
//! [`jsonapi_core`] types are re-exported so handler code can import from
//! `jsonapi_axum` alone.
#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod error;
pub mod extract;
pub mod normalize;
pub mod response;

pub use error::JsonApiError;
pub use extract::{JsonApi, JsonApiQuery, JsonApiQueryValidated};
pub use normalize::{NormalizeErrorsLayer, NormalizeErrorsService, not_found};
pub use response::JsonApiResponse;

// Re-export the framework-agnostic tower layers so axum users get them from one
// place.
pub use jsonapi_http::{AcceptLayer, ContentTypeLayer, JsonApiLayer};

/// Re-exports of the [`jsonapi_core`] types most commonly needed to build
/// handlers, so handler bodies can import from `jsonapi_axum` alone.
///
/// **Deriving [`JsonApi`](jsonapi_core::JsonApi) still requires a direct
/// `jsonapi_core` dependency:** the derive macro expands to absolute
/// `::jsonapi_core::…` paths, so the crate must be nameable in the consumer's
/// crate. Only the runtime types are re-exported here.
pub use jsonapi_core::{
    ApiError, CURSOR_PAGINATION_PROFILE, CursorLinks, CursorPage, Document, DocumentBuilder,
    JsonApiMediaType, Query, Relationship, Resource, SortField, TypeRegistry,
};

/// The full [`jsonapi_core`] crate, for any type not re-exported above.
pub use jsonapi_core;
