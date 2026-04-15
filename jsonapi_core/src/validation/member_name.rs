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

/// Returns true if `c` is a globally allowed character per JSON:API 1.1.
/// Allowed: `[a-zA-Z0-9\u{0080}-\u{FFFF}]`.
fn is_globally_allowed(c: char) -> bool {
    c.is_ascii_alphanumeric() || ('\u{0080}'..='\u{FFFF}').contains(&c)
}

/// Returns true if `c` is allowed in the interior of a member name.
/// Interior allows globally allowed chars plus `-`, `_`, and space.
fn is_interior_allowed(c: char) -> bool {
    is_globally_allowed(c) || c == '-' || c == '_' || c == ' '
}

/// Check that `name` follows standard member-name character rules.
/// Returns `Ok(())` or `Err(reason)`.
fn check_standard_name(name: &str) -> Result<(), String> {
    let mut chars = name.chars();
    let first = match chars.next() {
        None => return Err("must not be empty".into()),
        Some(c) => c,
    };
    if !is_globally_allowed(first) {
        return Err(format!(
            "must start with [a-zA-Z0-9\\u{{0080}}-\\u{{FFFF}}], got '{first}'"
        ));
    }

    let mut last = first;
    for c in chars {
        if !is_interior_allowed(c) {
            return Err(format!("invalid interior character '{c}'"));
        }
        last = c;
    }

    // If name has more than one char, check the last character
    if last != first && !is_globally_allowed(last) {
        return Err(format!(
            "must end with [a-zA-Z0-9\\u{{0080}}-\\u{{FFFF}}], got '{last}'"
        ));
    }

    Ok(())
}

/// Validate a member name per JSON:API 1.1 rules.
#[must_use = "validation result should be used"]
pub fn validate_member_name(name: &str) -> crate::Result<MemberNameKind> {
    if name.is_empty() {
        return Err(crate::Error::InvalidMemberName {
            name: name.to_string(),
            reason: "member name must not be empty".into(),
        });
    }

    if let Some(rest) = name.strip_prefix('@') {
        let Some((namespace, member)) = rest.split_once(':') else {
            return Err(crate::Error::InvalidMemberName {
                name: name.to_string(),
                reason: "@-member must contain ':' separator (format: @namespace:member)".into(),
            });
        };
        if namespace.is_empty() {
            return Err(crate::Error::InvalidMemberName {
                name: name.to_string(),
                reason: "@-member namespace must not be empty".into(),
            });
        }
        if member.is_empty() {
            return Err(crate::Error::InvalidMemberName {
                name: name.to_string(),
                reason: "@-member member must not be empty".into(),
            });
        }
        check_standard_name(namespace).map_err(|reason| crate::Error::InvalidMemberName {
            name: name.to_string(),
            reason: format!("namespace: {reason}"),
        })?;
        check_standard_name(member).map_err(|reason| crate::Error::InvalidMemberName {
            name: name.to_string(),
            reason: format!("member: {reason}"),
        })?;
        return Ok(MemberNameKind::AtMember {
            namespace: namespace.to_string(),
            member: member.to_string(),
        });
    }

    // Extension-namespaced member: exactly one `:` splits namespace from member.
    if let Some((namespace, member)) = name.split_once(':') {
        if name.matches(':').count() != 1 {
            return Err(crate::Error::InvalidMemberName {
                name: name.to_string(),
                reason: "extension member name must contain exactly one ':' separator".into(),
            });
        }
        if namespace.is_empty() {
            return Err(crate::Error::InvalidMemberName {
                name: name.to_string(),
                reason: "extension namespace must not be empty".into(),
            });
        }
        if member.is_empty() {
            return Err(crate::Error::InvalidMemberName {
                name: name.to_string(),
                reason: "extension member must not be empty".into(),
            });
        }
        check_standard_name(namespace).map_err(|reason| crate::Error::InvalidMemberName {
            name: name.to_string(),
            reason: format!("namespace: {reason}"),
        })?;
        check_standard_name(member).map_err(|reason| crate::Error::InvalidMemberName {
            name: name.to_string(),
            reason: format!("member: {reason}"),
        })?;
        return Ok(MemberNameKind::ExtensionMember {
            namespace: namespace.to_string(),
            member: member.to_string(),
        });
    }

    check_standard_name(name).map_err(|reason| crate::Error::InvalidMemberName {
        name: name.to_string(),
        reason,
    })?;
    Ok(MemberNameKind::Standard)
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
