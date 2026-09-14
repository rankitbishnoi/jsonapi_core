//! Compile-time member-name validation for the derive macro.
//!
//! Thin wrapper over [`jsonapi_core_validation::classify_member_name`] so the
//! proc macro rejects invalid member names during expansion. The runtime crate
//! (`jsonapi_core::validation`) uses the same shared logic, keeping compile-time
//! and runtime validation in lockstep.

/// Validate a member name per JSON:API 1.1 character rules.
/// Returns `Ok(())` or `Err(reason)`.
///
/// The proc macro only needs to accept or reject; the classification returned by
/// the shared validator is discarded.
pub fn validate_member_name(name: &str) -> Result<(), String> {
    jsonapi_core_validation::classify_member_name(name).map(|_| ())
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
