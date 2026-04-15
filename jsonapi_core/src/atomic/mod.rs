//! JSON:API Atomic Operations extension (v1.1).
//!
//! This module provides types for building and parsing atomic operations
//! requests and responses per the [Atomic Operations extension](https://jsonapi.org/ext/atomic/).
//!
//! The module is gated behind the `atomic-ops` feature (off by default).
//!
//! # Example
//!
//! ```
//! use jsonapi_core::atomic::{AtomicRequest, ATOMIC_EXT_URI};
//!
//! assert_eq!(ATOMIC_EXT_URI, "https://jsonapi.org/ext/atomic");
//! let req = AtomicRequest::default();
//! assert!(req.operations.is_empty());
//! ```

mod operation;
mod result;

pub use operation::{AtomicOperation, AtomicRequest, OperationRef, OperationTarget};
pub use result::{AtomicResponse, AtomicResult};

/// URI identifying the JSON:API Atomic Operations extension.
pub const ATOMIC_EXT_URI: &str = "https://jsonapi.org/ext/atomic";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_ext_uri_constant() {
        assert_eq!(ATOMIC_EXT_URI, "https://jsonapi.org/ext/atomic");
    }
}
