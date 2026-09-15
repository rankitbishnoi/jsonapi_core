//! Framework-agnostic HTTP integration for [`jsonapi_core`].
//!
//! This crate turns the pieces `jsonapi_core` already provides (content-type
//! validation, `Accept` negotiation, query parsing, document building) into
//! plain functions and [`tower`](https://docs.rs/tower) middleware over the
//! [`http`] crate's request/response types. It contains **no** web-framework
//! dependency; adapters such as `jsonapi_axum` provide the thin
//! `FromRequest`/`IntoResponse` bindings on top.
//!
//! # Module map
//!
//! - [`error`] — map [`jsonapi_core::Error`] to an HTTP status and a JSON:API
//!   error document response.
//! - [`id`] — resource-`id` consistency and client-id policy checks.
//! - [`request`] — parse an incoming request (content type, accept, query,
//!   typed body) into typed values.
//! - [`response`] — build a JSON:API [`http::Response`] from a document.
//! - [`layer`] — `tower` layers that reject non-conforming requests early.
#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod error;
pub mod id;
pub mod layer;
pub mod request;
pub mod response;

pub use error::{
    api_error_for_status, error_response, error_response_for, error_response_for_status, status_for,
    to_api_error,
};
pub use id::{ClientIdPolicy, check_client_id, check_id_matches, id_conflict};
pub use layer::{AcceptLayer, ContentTypeLayer, GuardService, JsonApiLayer};
pub use request::{check_content_type, deserialize_body, negotiate, parse_query};
pub use response::{
    content_type_value, document_response, json_api_response, json_api_response_filtered,
};

/// The JSON:API media type: `application/vnd.api+json`.
pub const JSON_API_MEDIA_TYPE: &str = "application/vnd.api+json";
