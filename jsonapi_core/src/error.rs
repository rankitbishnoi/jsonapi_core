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

    /// Caller asked the document for a specific shape (e.g.
    /// [`Document::into_single`](crate::Document::into_single)) but the
    /// document carries a different shape.
    #[error("expected {expected} document, got {found}")]
    UnexpectedDocumentShape {
        /// What the caller expected, e.g. `"single resource"`,
        /// `"resource collection"`, `"meta-only"`.
        expected: &'static str,
        /// What the document actually is, e.g. `"resource collection"`,
        /// `"errors document"`, `"meta-only document"`, `"null primary data"`.
        found: &'static str,
    },

    /// The wire-side resource `type` does not match the type declared by the
    /// Rust type the document is being deserialized into. Surfaced by
    /// [`Document::from_str`](crate::Document::from_str),
    /// [`Document::from_slice`](crate::Document::from_slice), and
    /// [`Document::from_value`](crate::Document::from_value).
    #[error("type mismatch at {location}: expected `{expected}`, got `{got}`")]
    TypeMismatch {
        /// The Rust-side `#[jsonapi(type = "...")]` value.
        expected: &'static str,
        /// The `type` value found on the wire.
        got: String,
        /// JSON pointer-style path to the offending resource, e.g. `"data"`,
        /// `"data[3]"`, or `"included[2]"`.
        location: String,
    },

    /// A relationship object on the wire is structurally malformed
    /// (e.g. missing the required `data` member, or a `data` value that is
    /// neither `null`, an object, nor an array).
    #[error("malformed relationship `{name}` at {location}: {reason}")]
    MalformedRelationship {
        /// Wire name of the offending relationship.
        name: String,
        /// JSON pointer-style path to the resource carrying the relationship.
        location: String,
        /// Human-readable explanation of what is wrong.
        reason: String,
    },

    /// A required attribute is absent from the wire `attributes` block.
    /// Surfaced by [`Document::from_str`](crate::Document::from_str),
    /// [`Document::from_slice`](crate::Document::from_slice), and
    /// [`Document::from_value`](crate::Document::from_value).
    ///
    /// "Required" here means a field declared on the consumer's resource
    /// struct as a non-`Option`, non-`Vec` attribute. `Option<T>` attributes
    /// are silently absent on the wire; `Vec<T>` attributes default to empty.
    #[error("missing required attribute `{attribute}` on `{resource_type}` at {location}")]
    MissingAttribute {
        /// The Rust-side `#[jsonapi(type = "...")]` value of the resource
        /// carrying the missing attribute.
        resource_type: &'static str,
        /// Wire name of the attribute that the consumer declared as required
        /// but the payload did not include.
        attribute: &'static str,
        /// JSON pointer-style path to the offending resource, e.g. `"data"`
        /// or `"data[3]"`.
        location: String,
    },

    /// A relationship references a `(type, id)` pair that is not present in
    /// the wire `included` array. Surfaced by
    /// [`Document::from_str`](crate::Document::from_str),
    /// [`Document::from_slice`](crate::Document::from_slice), and
    /// [`Document::from_value`](crate::Document::from_value).
    ///
    /// References that use only `lid` (no `id`) are intentionally skipped:
    /// those are atomic-operation client-local identifiers, resolved at
    /// request execution rather than at parse time.
    #[error(
        "relationship `{name}` at {location} references {type_}:{id}, but no such resource is included"
    )]
    IncludedRefMissing {
        /// Wire name of the offending relationship.
        name: String,
        /// Wire-side referenced resource type.
        type_: String,
        /// Wire-side referenced resource id.
        id: String,
        /// JSON pointer-style path to the offending relationship, e.g.
        /// `"data.relationships.author"` or
        /// `"included[3].relationships.organization"`.
        location: String,
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

    #[test]
    fn test_missing_attribute_display() {
        let err = Error::MissingAttribute {
            resource_type: "articles",
            attribute: "title",
            location: "data".into(),
        };
        assert_eq!(
            err.to_string(),
            "missing required attribute `title` on `articles` at data"
        );
    }

    #[test]
    fn test_included_ref_missing_display() {
        let err = Error::IncludedRefMissing {
            name: "author".into(),
            type_: "people".into(),
            id: "9".into(),
            location: "data.relationships.author".into(),
        };
        assert_eq!(
            err.to_string(),
            "relationship `author` at data.relationships.author references people:9, but no such resource is included"
        );
    }
}
