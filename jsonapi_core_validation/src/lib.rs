#![deny(missing_docs)]
//! Shared JSON:API member-name validation for `jsonapi_core`.
//!
//! **Implementation detail.** This crate exists so that `jsonapi_core` (at
//! runtime) and `jsonapi_core_derive` (at macro-expansion time) validate member
//! names against a single source of truth. It carries no API-stability guarantee
//! for direct use — depend on `jsonapi_core` instead.
//!
//! The entry point is [`classify_member_name`], which both validates a name per
//! the JSON:API 1.1 character rules and reports which kind of member it is.

/// The classification of a valid member name per JSON:API 1.1.
///
/// The namespace and member segments borrow from the validated input; callers
/// that need owned data convert as required.
///
/// This enum is intentionally exhaustive: consumers within the `jsonapi_core`
/// workspace match on it directly, and the crate is versioned in lockstep.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemberNameKind<'a> {
    /// Standard member name (e.g. `"title"`, `"first-name"`).
    Standard,
    /// `@`-member: `@namespace:member` (e.g. `"@ext:comments"`).
    AtMember {
        /// The extension namespace (e.g. `"ext"` in `@ext:comments`).
        namespace: &'a str,
        /// The member name after the colon (e.g. `"comments"`).
        member: &'a str,
    },
    /// Extension-namespaced member: `namespace:member` (e.g. `"atomic:operations"`).
    ///
    /// Used by registered JSON:API extensions. The namespace is declared by the
    /// extension's URI in the media-type `ext` parameter; validation checks
    /// syntax only, not whether the extension is active.
    ExtensionMember {
        /// The extension namespace (e.g. `"atomic"` in `"atomic:operations"`).
        namespace: &'a str,
        /// The member name after the colon (e.g. `"operations"`).
        member: &'a str,
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

    // If name has more than one char, check the last character.
    if last != first && !is_globally_allowed(last) {
        return Err(format!(
            "must end with [a-zA-Z0-9\\u{{0080}}-\\u{{FFFF}}], got '{last}'"
        ));
    }

    Ok(())
}

/// Validate and classify a member name per JSON:API 1.1 rules.
///
/// On success returns the [`MemberNameKind`]; on failure returns a
/// human-readable reason. The reason strings are stable: `jsonapi_core_derive`
/// embeds them in its compile-error output, so changing them is a breaking
/// change to that crate's diagnostics.
pub fn classify_member_name(name: &str) -> Result<MemberNameKind<'_>, String> {
    if name.is_empty() {
        return Err("member name must not be empty".into());
    }

    // @-member: @namespace:member
    if let Some(rest) = name.strip_prefix('@') {
        let Some((namespace, member)) = rest.split_once(':') else {
            return Err("@-member must contain ':' separator (format: @namespace:member)".into());
        };
        if namespace.is_empty() {
            return Err("@-member namespace must not be empty".into());
        }
        if member.is_empty() {
            return Err("@-member member must not be empty".into());
        }
        check_standard_name(namespace).map_err(|reason| format!("namespace: {reason}"))?;
        check_standard_name(member).map_err(|reason| format!("member: {reason}"))?;
        return Ok(MemberNameKind::AtMember { namespace, member });
    }

    // Extension-namespaced member: exactly one `:` splits namespace from member.
    if let Some((namespace, member)) = name.split_once(':') {
        if member.contains(':') {
            return Err("extension member name must contain exactly one ':' separator".into());
        }
        if namespace.is_empty() {
            return Err("extension namespace must not be empty".into());
        }
        if member.is_empty() {
            return Err("extension member must not be empty".into());
        }
        check_standard_name(namespace).map_err(|reason| format!("namespace: {reason}"))?;
        check_standard_name(member).map_err(|reason| format!("member: {reason}"))?;
        return Ok(MemberNameKind::ExtensionMember { namespace, member });
    }

    check_standard_name(name)?;
    Ok(MemberNameKind::Standard)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Standard names: valid ---

    #[test]
    fn valid_standard_names() {
        for name in [
            "title",
            "a",
            "1",
            "first-name",
            "first_name",
            "first name",
            "field1",
        ] {
            assert_eq!(
                classify_member_name(name).unwrap(),
                MemberNameKind::Standard,
                "{name}"
            );
        }
    }

    #[test]
    fn valid_unicode_names() {
        // U+00E9 = é (above the U+0080 threshold); U+FFFF is the upper boundary.
        assert_eq!(
            classify_member_name("\u{00E9}tag").unwrap(),
            MemberNameKind::Standard
        );
        assert_eq!(
            classify_member_name("\u{00E9}").unwrap(),
            MemberNameKind::Standard
        );
        assert_eq!(
            classify_member_name("\u{FFFF}").unwrap(),
            MemberNameKind::Standard
        );
    }

    // --- Standard names: invalid ---

    #[test]
    fn invalid_standard_names() {
        for name in [
            "", "-foo", "foo-", "_foo", "foo_", " foo", "foo ", "foo!bar", "foo.bar",
        ] {
            assert!(classify_member_name(name).is_err(), "{name}");
        }
    }

    // --- @-members ---

    #[test]
    fn valid_at_members() {
        assert_eq!(
            classify_member_name("@ext:comments").unwrap(),
            MemberNameKind::AtMember {
                namespace: "ext",
                member: "comments",
            }
        );
        assert_eq!(
            classify_member_name("@\u{00E9}xt:field").unwrap(),
            MemberNameKind::AtMember {
                namespace: "\u{00E9}xt",
                member: "field",
            }
        );
    }

    #[test]
    fn invalid_at_members() {
        for name in ["@extcomments", "@:comments", "@ext:", "@-ext:comments", "@"] {
            assert!(classify_member_name(name).is_err(), "{name}");
        }
    }

    // --- Extension-namespaced members ---

    #[test]
    fn valid_extension_members() {
        assert_eq!(
            classify_member_name("atomic:operations").unwrap(),
            MemberNameKind::ExtensionMember {
                namespace: "atomic",
                member: "operations",
            }
        );
        assert_eq!(
            classify_member_name("atomic:results").unwrap(),
            MemberNameKind::ExtensionMember {
                namespace: "atomic",
                member: "results",
            }
        );
    }

    #[test]
    fn invalid_extension_members() {
        for name in [
            ":member",
            "namespace:",
            "-atomic:results",
            "atomic:-results",
            "atomic:results:extra",
        ] {
            assert!(classify_member_name(name).is_err(), "{name}");
        }
    }

    #[test]
    fn at_member_takes_precedence_over_extension() {
        // Regression: @-prefixed names must not be routed to ExtensionMember.
        assert!(matches!(
            classify_member_name("@ext:foo").unwrap(),
            MemberNameKind::AtMember { .. }
        ));
    }

    // --- Reason-string stability ---

    #[test]
    fn bad_start_reason_is_stable() {
        // This exact string is embedded in jsonapi_core_derive compile-fail
        // output (tests/compile_fail/invalid_type_name.stderr). Do not change.
        assert_eq!(
            classify_member_name("-invalid").unwrap_err(),
            "must start with [a-zA-Z0-9\\u{0080}-\\u{FFFF}], got '-'"
        );
    }
}
