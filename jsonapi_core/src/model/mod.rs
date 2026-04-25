//! Core JSON:API types.
//!
//! This module contains the full type model for JSON:API v1.1 documents:
//! [`Document`], [`PrimaryData`], [`Resource`], [`ResourceIdentifier`],
//! [`Relationship`], [`Link`], [`ApiError`], and supporting types.
//!
//! Use [`Document<T>`] as the top-level type for serialization and deserialization,
//! where `T` implements [`ResourceObject`]. For typed resources, derive `T` with
//! `#[derive(JsonApi)]`. For open-set handling of unknown types, use
//! [`Document<Resource>`].

mod accessors;
mod document;
mod error;
mod identifier;
mod jsonapi_object;
mod link;
mod meta;
mod relationship;
mod resource;

pub use accessors::{HasLinks, HasMeta};
pub use document::{Document, PrimaryData};
pub use error::{ApiError, ErrorLinks, ErrorSource};
pub use identifier::{Identity, ResourceIdentifier};
pub use jsonapi_object::JsonApiObject;
pub use link::{Hreflang, Link, LinkObject, Links};
pub use meta::Meta;
pub use relationship::{Relationship, RelationshipData};
pub use resource::{Resource, ResourceObject};
