//! Crate-level error types.
//!
//! [`Error`] is the unified error type for all `jsonapi_core` operations.
//! [`Result<T>`](Result) is a convenience alias for `std::result::Result<T, Error>`.

/// Crate-level error type.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A serde_json serialization or deserialization error.
    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    /// A member name violates JSON:API 1.1 naming rules.
    #[error("invalid member name: {name} — {reason}")]
    InvalidMemberName {
        /// The offending member name.
        name: String,
        /// Human-readable explanation of why validation failed.
        reason: String,
    },

    /// A resource was not found in the [`Registry`](crate::Registry).
    #[error("resource not found in registry: type={type_}, id={id}")]
    RegistryLookup {
        /// The JSON:API type string that was queried.
        type_: String,
        /// The resource id that was not found.
        id: String,
    },

    /// A to-one relationship is null; cannot look up a concrete resource.
    #[error("null relationship: cannot look up resource")]
    NullRelationship,

    /// Caller used `get()` on a to-many relationship or `get_many()` on a to-one.
    #[error("relationship cardinality mismatch: expected {expected}")]
    RelationshipCardinalityMismatch {
        /// The expected cardinality (`"to-one"` or `"to-many"`).
        expected: &'static str,
    },

    /// Registry does not support lookup by local identifier (lid).
    #[error("registry does not index by lid")]
    LidNotIndexed,

    /// Base media type does not match `application/vnd.api+json`.
    #[error("media type mismatch: expected {expected}, got {got}")]
    MediaTypeMismatch {
        /// The expected media type.
        expected: String,
        /// The media type that was received.
        got: String,
    },

    /// A media-type parameter other than `ext` or `profile` was present.
    #[error("unsupported media type parameter: {param}")]
    UnsupportedMediaTypeParam {
        /// The unsupported parameter name.
        param: String,
    },

    /// A media-type string could not be parsed (syntax error).
    #[error("media type parse error: {0}")]
    MediaTypeParse(String),

    /// No acceptable JSON:API media type found in the Accept header.
    #[error("no acceptable JSON:API media type found in Accept header")]
    NoAcceptableMediaType,

    /// All JSON:API entries in Accept have unsupported parameters (406 semantics).
    #[error("all JSON:API media type instances in Accept have unsupported parameters")]
    AllMediaTypesUnsupportedParams,

    /// A document violates structural rules (e.g. `data` + `errors` both present).
    #[error("document structure error: {0}")]
    Structure(String),

    /// An include path references a relationship that does not exist on the type.
    #[error(
        "invalid include path '{path}': relationship '{segment}' not found on type '{type_name}'"
    )]
    InvalidIncludePath {
        /// The full dot-separated include path.
        path: String,
        /// The path segment that could not be resolved.
        segment: String,
        /// The type on which the segment was not found.
        type_name: String,
    },

    /// Structural violation in an atomic operations payload (e.g. dangling
    /// `lid` reference, duplicate `lid` introduction, or a target with both
    /// `ref` and `href` set).
    #[error("invalid atomic operation at index {index}: {reason}")]
    InvalidAtomicOperation {
        /// Zero-based index of the offending operation within the request.
        index: usize,
        /// Human-readable explanation.
        reason: String,
    },
}

/// Convenience alias.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = Error::RegistryLookup {
            type_: "people".into(),
            id: "99".into(),
        };
        assert_eq!(
            err.to_string(),
            "resource not found in registry: type=people, id=99"
        );
    }

    #[test]
    fn test_error_from_serde_json() {
        let json_err = serde_json::from_str::<String>("not json").unwrap_err();
        let err: Error = json_err.into();
        assert!(matches!(err, Error::Json(_)));
    }

    #[test]
    fn test_invalid_include_path_display() {
        let err = Error::InvalidIncludePath {
            path: "author.posts".into(),
            segment: "posts".into(),
            type_name: "people".into(),
        };
        assert_eq!(
            err.to_string(),
            "invalid include path 'author.posts': relationship 'posts' not found on type 'people'"
        );
    }
}
