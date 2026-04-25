//! Structural accessor traits for resources that carry resource-level
//! `links` and / or `meta` blocks.
//!
//! These traits give consumers a uniform way to read those blocks without
//! pattern-matching on concrete types. The `#[derive(JsonApi)]` macro
//! auto-implements [`HasLinks`] whenever a `#[jsonapi(links)]` field is
//! present, and [`HasMeta`] whenever a `#[jsonapi(meta)]` field is present.
//!
//! The implementations are deliberately *not* defaulted on
//! [`ResourceObject`](super::ResourceObject): a default returning `None`
//! would silently misrepresent every resource that has no links/meta field
//! at all. Consumers that want to bound on these capabilities should write
//! `T: ResourceObject + HasLinks` (or `+ HasMeta`) explicitly.
//!
//! The dynamic [`Resource`](super::Resource) fallback also implements both
//! traits.

use super::{Links, Meta};

/// Read access to a resource's resource-level `links` block.
pub trait HasLinks {
    /// Borrow the resource's `links` block, if present.
    fn links(&self) -> Option<&Links>;
}

/// Read access to a resource's resource-level `meta` block.
pub trait HasMeta {
    /// Borrow the resource's `meta` block, if present.
    fn meta(&self) -> Option<&Meta>;
}
