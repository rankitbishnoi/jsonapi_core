//! JSON:API 1.1 media-type parsing and content negotiation.
//!
//! Provides [`validate_content_type()`] for incoming request validation (415 on
//! unknown parameters), [`negotiate_accept()`] for response media-type selection
//! (406 on failure), and [`JsonApiMediaType`] for parsing and building
//! `application/vnd.api+json` header values with `ext` and `profile` parameters.

mod negotiation;
mod parser;

pub use negotiation::{JsonApiMediaType, negotiate_accept, validate_content_type};
