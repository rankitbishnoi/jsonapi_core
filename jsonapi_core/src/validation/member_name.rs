/// The kind of member name per JSON:API 1.1.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemberNameKind {
    /// Standard member name (e.g. "title", "first-name").
    Standard,
    /// @-member: `@namespace:member` (e.g. `"@ext:comments"`).
    AtMember {
        /// The extension namespace (e.g. `"ext"` in `@ext:comments`).
        namespace: String,
        /// The member name after the colon (e.g. `"comments"` in `@ext:comments`).
        member: String,
    },
    /// Extension-namespaced member: `namespace:member` (e.g. `"atomic:operations"`).
    ///
    /// Used by registered JSON:API extensions. The namespace is declared by
    /// the extension's URI in the media-type `ext` parameter; validation
    /// checks syntax only, not whether the extension is active.
    ExtensionMember {
        /// The extension namespace (e.g. `"atomic"` in `"atomic:operations"`).
        namespace: String,
        /// The member name after the colon (e.g. `"operations"` in `"atomic:operations"`).
        member: String,
    },
}

impl From<jsonapi_core_validation::MemberNameKind<'_>> for MemberNameKind {
    fn from(kind: jsonapi_core_validation::MemberNameKind<'_>) -> Self {
        use jsonapi_core_validation::MemberNameKind as Shared;
        match kind {
            Shared::Standard => MemberNameKind::Standard,
            Shared::AtMember { namespace, member } => MemberNameKind::AtMember {
                namespace: namespace.to_string(),
                member: member.to_string(),
            },
            Shared::ExtensionMember { namespace, member } => MemberNameKind::ExtensionMember {
                namespace: namespace.to_string(),
                member: member.to_string(),
            },
        }
    }
}

/// Validate a member name per JSON:API 1.1 rules.
///
/// The character-level rules live in `jsonapi_core_validation` (shared with the
/// derive macro, so compile-time and runtime validation cannot drift); this
/// wrapper maps the shared classification to the owned [`MemberNameKind`] and
/// failures to [`Error::InvalidMemberName`](crate::Error::InvalidMemberName).
#[must_use = "validation result should be used"]
pub fn validate_member_name(name: &str) -> crate::Result<MemberNameKind> {
    jsonapi_core_validation::classify_member_name(name)
        .map(MemberNameKind::from)
        .map_err(|reason| crate::Error::InvalidMemberName {
            name: name.to_string(),
            reason,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Standard names: valid ---

    #[test]
    fn valid_simple_name() {
        assert_eq!(
            validate_member_name("title").unwrap(),
            MemberNameKind::Standard
        );
    }

    #[test]
    fn valid_single_char() {
        assert_eq!(validate_member_name("a").unwrap(), MemberNameKind::Standard);
    }

    #[test]
    fn valid_single_digit() {
        assert_eq!(validate_member_name("1").unwrap(), MemberNameKind::Standard);
    }

    #[test]
    fn valid_with_hyphen() {
        assert_eq!(
            validate_member_name("first-name").unwrap(),
            MemberNameKind::Standard
        );
    }

    #[test]
    fn valid_with_underscore() {
        assert_eq!(
            validate_member_name("first_name").unwrap(),
            MemberNameKind::Standard
        );
    }

    #[test]
    fn valid_with_space() {
        assert_eq!(
            validate_member_name("first name").unwrap(),
            MemberNameKind::Standard
        );
    }

    #[test]
    fn valid_with_digits() {
        assert_eq!(
            validate_member_name("field1").unwrap(),
            MemberNameKind::Standard
        );
    }

    #[test]
    fn valid_unicode_start() {
        // U+00E9 = é (above U+0080 threshold)
        assert_eq!(
            validate_member_name("\u{00E9}tag").unwrap(),
            MemberNameKind::Standard
        );
    }

    #[test]
    fn valid_unicode_only() {
        assert_eq!(
            validate_member_name("\u{00E9}").unwrap(),
            MemberNameKind::Standard
        );
    }

    #[test]
    fn valid_unicode_ffff_boundary() {
        // U+FFFF is the upper boundary of globally allowed
        assert_eq!(
            validate_member_name("\u{FFFF}").unwrap(),
            MemberNameKind::Standard
        );
    }

    // --- Standard names: invalid ---

    #[test]
    fn invalid_empty() {
        assert!(validate_member_name("").is_err());
    }

    #[test]
    fn invalid_starts_with_hyphen() {
        assert!(validate_member_name("-foo").is_err());
    }

    #[test]
    fn invalid_ends_with_hyphen() {
        assert!(validate_member_name("foo-").is_err());
    }

    #[test]
    fn invalid_starts_with_underscore() {
        assert!(validate_member_name("_foo").is_err());
    }

    #[test]
    fn invalid_ends_with_underscore() {
        assert!(validate_member_name("foo_").is_err());
    }

    #[test]
    fn invalid_starts_with_space() {
        assert!(validate_member_name(" foo").is_err());
    }

    #[test]
    fn invalid_ends_with_space() {
        assert!(validate_member_name("foo ").is_err());
    }

    #[test]
    fn invalid_interior_bang() {
        assert!(validate_member_name("foo!bar").is_err());
    }

    #[test]
    fn invalid_interior_dot() {
        assert!(validate_member_name("foo.bar").is_err());
    }

    // --- @-members: valid ---

    #[test]
    fn valid_at_member() {
        let result = validate_member_name("@ext:comments").unwrap();
        assert_eq!(
            result,
            MemberNameKind::AtMember {
                namespace: "ext".into(),
                member: "comments".into(),
            }
        );
    }

    #[test]
    fn valid_at_member_unicode() {
        let result = validate_member_name("@\u{00E9}xt:field").unwrap();
        assert_eq!(
            result,
            MemberNameKind::AtMember {
                namespace: "\u{00E9}xt".into(),
                member: "field".into(),
            }
        );
    }

    // --- @-members: invalid ---

    #[test]
    fn invalid_at_member_no_colon() {
        assert!(validate_member_name("@extcomments").is_err());
    }

    #[test]
    fn invalid_at_member_empty_namespace() {
        assert!(validate_member_name("@:comments").is_err());
    }

    #[test]
    fn invalid_at_member_empty_member() {
        assert!(validate_member_name("@ext:").is_err());
    }

    #[test]
    fn invalid_at_member_bad_namespace_char() {
        assert!(validate_member_name("@-ext:comments").is_err());
    }

    #[test]
    fn invalid_at_only() {
        assert!(validate_member_name("@").is_err());
    }

    // --- Extension-namespaced members (`namespace:member`) ---

    #[test]
    fn valid_extension_member_atomic_operations() {
        let result = validate_member_name("atomic:operations").unwrap();
        assert_eq!(
            result,
            MemberNameKind::ExtensionMember {
                namespace: "atomic".into(),
                member: "operations".into(),
            }
        );
    }

    #[test]
    fn valid_extension_member_atomic_results() {
        let result = validate_member_name("atomic:results").unwrap();
        assert_eq!(
            result,
            MemberNameKind::ExtensionMember {
                namespace: "atomic".into(),
                member: "results".into(),
            }
        );
    }

    #[test]
    fn invalid_extension_member_empty_namespace() {
        assert!(validate_member_name(":member").is_err());
    }

    #[test]
    fn invalid_extension_member_empty_member() {
        assert!(validate_member_name("namespace:").is_err());
    }

    #[test]
    fn invalid_extension_member_bad_namespace_char() {
        assert!(validate_member_name("-atomic:results").is_err());
    }

    #[test]
    fn invalid_extension_member_bad_member_char() {
        assert!(validate_member_name("atomic:-results").is_err());
    }

    #[test]
    fn extension_member_multiple_colons_is_error() {
        // Only a single `:` splits namespace from member.
        assert!(validate_member_name("atomic:results:extra").is_err());
    }

    #[test]
    fn at_member_still_takes_precedence() {
        // Regression: @-prefixed names must not be routed to ExtensionMember.
        let result = validate_member_name("@ext:foo").unwrap();
        assert!(matches!(result, MemberNameKind::AtMember { .. }));
    }
}
