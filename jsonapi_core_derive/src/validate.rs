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

/// Validate a member name per JSON:API 1.1 character rules.
/// Returns `Ok(())` or `Err(reason)`.
///
/// This is a standalone copy of `jsonapi_core::validation::validate_member_name`
/// for use at compile time in the proc macro. The core crate version returns
/// `MemberNameKind`; this version just validates and rejects invalid names.
pub fn validate_member_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("member name must not be empty".into());
    }

    // @-members: @namespace:member
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
        return Ok(());
    }

    // Extension-namespaced member: namespace:member (JSON:API 1.1 extension syntax)
    if let Some((namespace, member)) = name.split_once(':') {
        if name.matches(':').count() != 1 {
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
        return Ok(());
    }

    check_standard_name(name)
}

// Keep in sync with jsonapi_core::validation::member_name::check_standard_name
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_names() {
        assert!(validate_member_name("title").is_ok());
        assert!(validate_member_name("first-name").is_ok());
        assert!(validate_member_name("first_name").is_ok());
        assert!(validate_member_name("a").is_ok());
        assert!(validate_member_name("articles").is_ok());
        assert!(validate_member_name("blog-posts").is_ok());
    }

    #[test]
    fn invalid_names() {
        assert!(validate_member_name("").is_err());
        assert!(validate_member_name("-foo").is_err());
        assert!(validate_member_name("foo-").is_err());
        assert!(validate_member_name("_foo").is_err());
        assert!(validate_member_name("foo!bar").is_err());
    }

    #[test]
    fn valid_at_members() {
        assert!(validate_member_name("@ext:comments").is_ok());
    }

    #[test]
    fn invalid_at_members() {
        assert!(validate_member_name("@extcomments").is_err());
        assert!(validate_member_name("@:comments").is_err());
        assert!(validate_member_name("@ext:").is_err());
    }

    #[test]
    fn valid_extension_members() {
        assert!(validate_member_name("atomic:operations").is_ok());
        assert!(validate_member_name("atomic:results").is_ok());
    }

    #[test]
    fn invalid_extension_members() {
        assert!(validate_member_name(":member").is_err());
        assert!(validate_member_name("namespace:").is_err());
        assert!(validate_member_name("a:b:c").is_err());
    }
}
