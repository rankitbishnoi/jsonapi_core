use super::parser::parse_media_type_params;

const JSONAPI_MEDIA_TYPE: &str = "application/vnd.api+json";

/// Parsed JSON:API media type with extension and profile parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonApiMediaType {
    /// Extension URIs.
    pub ext: Vec<String>,
    /// Profile URIs.
    pub profile: Vec<String>,
}

/// Shared parser. When `reject_unknown_params` is true, returns an error
/// for any parameter other than `ext` or `profile`.
fn parse_jsonapi(header: &str, reject_unknown_params: bool) -> crate::Result<JsonApiMediaType> {
    let (base, params) = parse_media_type_params(header)?;
    if !base.eq_ignore_ascii_case(JSONAPI_MEDIA_TYPE) {
        return Err(crate::Error::MediaTypeMismatch {
            expected: JSONAPI_MEDIA_TYPE.to_string(),
            got: base.to_string(),
        });
    }

    let mut ext = Vec::new();
    let mut profile = Vec::new();

    for (key, value) in params {
        match key {
            "ext" => ext.extend(value.split_whitespace().map(String::from)),
            "profile" => profile.extend(value.split_whitespace().map(String::from)),
            other if reject_unknown_params => {
                return Err(crate::Error::UnsupportedMediaTypeParam {
                    param: other.to_string(),
                });
            }
            _ => {}
        }
    }

    Ok(JsonApiMediaType { ext, profile })
}

impl JsonApiMediaType {
    /// Parse from a header value. Validates base type is `application/vnd.api+json`.
    /// Unknown parameters are silently ignored.
    #[must_use = "parsing result should be used"]
    pub fn parse(header: &str) -> crate::Result<Self> {
        parse_jsonapi(header, false)
    }

    /// Check if this media type is compatible with another.
    /// Compatible when all of self's ext URIs appear in other's ext,
    /// and all of self's profile URIs appear in other's profile.
    #[must_use]
    pub fn is_compatible_with(&self, other: &JsonApiMediaType) -> bool {
        self.ext.iter().all(|e| other.ext.contains(e))
            && self.profile.iter().all(|p| other.profile.contains(p))
    }

    /// Construct a plain `application/vnd.api+json` media type with no
    /// extensions or profiles.
    ///
    /// ```
    /// # use jsonapi_core::JsonApiMediaType;
    /// let mt = JsonApiMediaType::plain();
    /// assert_eq!(mt.to_header_value(), "application/vnd.api+json");
    /// ```
    #[must_use]
    pub fn plain() -> Self {
        Self {
            ext: Vec::new(),
            profile: Vec::new(),
        }
    }

    /// Construct an `application/vnd.api+json` media type declaring one or
    /// more extension URIs.
    ///
    /// ```
    /// # use jsonapi_core::JsonApiMediaType;
    /// let mt = JsonApiMediaType::with_ext(["https://jsonapi.org/ext/atomic"]);
    /// assert_eq!(mt.ext, vec!["https://jsonapi.org/ext/atomic".to_string()]);
    /// ```
    #[must_use]
    pub fn with_ext<S, I>(ext: I) -> Self
    where
        S: Into<String>,
        I: IntoIterator<Item = S>,
    {
        Self {
            ext: ext.into_iter().map(Into::into).collect(),
            profile: Vec::new(),
        }
    }

    /// Format as a header value string (usable for both Content-Type and Accept).
    #[must_use]
    pub fn to_header_value(&self) -> String {
        let mut result = JSONAPI_MEDIA_TYPE.to_string();
        if !self.ext.is_empty() {
            let escaped = self.ext.join(" ").replace('"', "\\\"");
            result.push_str(&format!("; ext=\"{escaped}\""));
        }
        if !self.profile.is_empty() {
            let escaped = self.profile.join(" ").replace('"', "\\\"");
            result.push_str(&format!("; profile=\"{escaped}\""));
        }
        result
    }
}

/// Validate a Content-Type header per JSON:API 1.1 rules.
/// Rejects if any parameters other than `ext` and `profile` are present.
#[must_use = "validation result should be used"]
pub fn validate_content_type(header: &str) -> crate::Result<JsonApiMediaType> {
    parse_jsonapi(header, true)
}

/// Choose a response media type from an Accept header.
///
/// Parses comma-separated entries. Returns the server's capabilities when a
/// valid JSON:API entry is found. Returns an error if:
/// - All JSON:API entries have unsupported parameters (`Error::AllMediaTypesUnsupportedParams`, 406 semantics)
/// - No JSON:API media type is present at all (`Error::NoAcceptableMediaType`)
///
/// Note: this function always returns the server's full ext/profile capabilities.
/// Per JSON:API 1.1, the 406 rule applies to unknown parameter *names* (e.g.
/// `charset`), not to ext/profile value mismatches. Use `is_compatible_with` if
/// you need to check whether specific ext/profile URIs are supported.
#[must_use = "negotiation result should be used"]
pub fn negotiate_accept(
    accept_header: &str,
    server_ext: &[&str],
    server_profile: &[&str],
) -> crate::Result<JsonApiMediaType> {
    let server = JsonApiMediaType {
        ext: server_ext.iter().map(|s| s.to_string()).collect(),
        profile: server_profile.iter().map(|s| s.to_string()).collect(),
    };

    let mut found_jsonapi = false;

    for entry in accept_header.split(',') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }

        // Check for wildcards
        let base = entry.split(';').next().unwrap_or("").trim();
        if base == "*/*" || base.eq_ignore_ascii_case("application/*") {
            return Ok(server);
        }

        let Ok((parsed_base, params)) = parse_media_type_params(entry) else {
            continue;
        };

        if !parsed_base.eq_ignore_ascii_case(JSONAPI_MEDIA_TYPE) {
            continue;
        }

        found_jsonapi = true;

        // Skip entries with unknown params
        if params.iter().any(|(k, _)| *k != "ext" && *k != "profile") {
            continue;
        }

        // Valid JSON:API entry — return server capabilities
        return Ok(server);
    }

    if found_jsonapi {
        Err(crate::Error::AllMediaTypesUnsupportedParams)
    } else {
        Err(crate::Error::NoAcceptableMediaType)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- parse ---

    #[test]
    fn parse_bare() {
        let mt = JsonApiMediaType::parse("application/vnd.api+json").unwrap();
        assert!(mt.ext.is_empty());
        assert!(mt.profile.is_empty());
    }

    #[test]
    fn parse_with_ext() {
        let mt =
            JsonApiMediaType::parse("application/vnd.api+json; ext=\"https://example.com/ext1\"")
                .unwrap();
        assert_eq!(mt.ext, vec!["https://example.com/ext1"]);
        assert!(mt.profile.is_empty());
    }

    #[test]
    fn parse_with_profile() {
        let mt =
            JsonApiMediaType::parse("application/vnd.api+json; profile=\"https://example.com/p1\"")
                .unwrap();
        assert!(mt.ext.is_empty());
        assert_eq!(mt.profile, vec!["https://example.com/p1"]);
    }

    #[test]
    fn parse_with_both() {
        let mt = JsonApiMediaType::parse(
            "application/vnd.api+json; ext=\"https://e1 https://e2\"; profile=\"https://p1\"",
        )
        .unwrap();
        assert_eq!(mt.ext, vec!["https://e1", "https://e2"]);
        assert_eq!(mt.profile, vec!["https://p1"]);
    }

    #[test]
    fn parse_ignores_unknown_params() {
        let mt =
            JsonApiMediaType::parse("application/vnd.api+json; charset=utf-8; ext=\"https://e1\"")
                .unwrap();
        assert_eq!(mt.ext, vec!["https://e1"]);
    }

    #[test]
    fn parse_case_insensitive_base() {
        let mt = JsonApiMediaType::parse("Application/Vnd.Api+JSON").unwrap();
        assert!(mt.ext.is_empty());
    }

    #[test]
    fn parse_wrong_base_type() {
        let err = JsonApiMediaType::parse("text/html").unwrap_err();
        assert!(matches!(err, crate::Error::MediaTypeMismatch { .. }));
    }

    // --- to_header_value ---

    #[test]
    fn to_header_value_bare() {
        let mt = JsonApiMediaType {
            ext: vec![],
            profile: vec![],
        };
        assert_eq!(mt.to_header_value(), "application/vnd.api+json");
    }

    #[test]
    fn to_header_value_with_ext() {
        let mt = JsonApiMediaType {
            ext: vec!["https://e1".into(), "https://e2".into()],
            profile: vec![],
        };
        assert_eq!(
            mt.to_header_value(),
            "application/vnd.api+json; ext=\"https://e1 https://e2\""
        );
    }

    #[test]
    fn to_header_value_with_both() {
        let mt = JsonApiMediaType {
            ext: vec!["https://e1".into()],
            profile: vec!["https://p1".into()],
        };
        assert_eq!(
            mt.to_header_value(),
            "application/vnd.api+json; ext=\"https://e1\"; profile=\"https://p1\""
        );
    }

    // --- validate_content_type ---

    #[test]
    fn validate_ct_bare() {
        let mt = validate_content_type("application/vnd.api+json").unwrap();
        assert!(mt.ext.is_empty());
        assert!(mt.profile.is_empty());
    }

    #[test]
    fn validate_ct_with_ext_and_profile() {
        let mt = validate_content_type(
            "application/vnd.api+json; ext=\"https://e1\"; profile=\"https://p1\"",
        )
        .unwrap();
        assert_eq!(mt.ext, vec!["https://e1"]);
        assert_eq!(mt.profile, vec!["https://p1"]);
    }

    #[test]
    fn validate_ct_rejects_unknown_param() {
        let err = validate_content_type("application/vnd.api+json; charset=utf-8").unwrap_err();
        assert!(matches!(
            err,
            crate::Error::UnsupportedMediaTypeParam { .. }
        ));
    }

    #[test]
    fn validate_ct_rejects_wrong_base() {
        let err = validate_content_type("text/html").unwrap_err();
        assert!(matches!(err, crate::Error::MediaTypeMismatch { .. }));
    }

    #[test]
    fn validate_ct_rejects_unknown_mixed_with_known() {
        let err =
            validate_content_type("application/vnd.api+json; ext=\"https://e1\"; charset=utf-8")
                .unwrap_err();
        assert!(matches!(
            err,
            crate::Error::UnsupportedMediaTypeParam { .. }
        ));
    }

    // --- is_compatible_with ---

    #[test]
    fn compatible_both_empty() {
        let a = JsonApiMediaType {
            ext: vec![],
            profile: vec![],
        };
        let b = JsonApiMediaType {
            ext: vec![],
            profile: vec![],
        };
        assert!(a.is_compatible_with(&b));
    }

    #[test]
    fn compatible_subset_ext() {
        let requested = JsonApiMediaType {
            ext: vec!["https://e1".into()],
            profile: vec![],
        };
        let server = JsonApiMediaType {
            ext: vec!["https://e1".into(), "https://e2".into()],
            profile: vec![],
        };
        assert!(requested.is_compatible_with(&server));
    }

    #[test]
    fn incompatible_ext_not_in_server() {
        let requested = JsonApiMediaType {
            ext: vec!["https://e3".into()],
            profile: vec![],
        };
        let server = JsonApiMediaType {
            ext: vec!["https://e1".into()],
            profile: vec![],
        };
        assert!(!requested.is_compatible_with(&server));
    }

    #[test]
    fn compatible_subset_profile() {
        let requested = JsonApiMediaType {
            ext: vec![],
            profile: vec!["https://p1".into()],
        };
        let server = JsonApiMediaType {
            ext: vec![],
            profile: vec!["https://p1".into(), "https://p2".into()],
        };
        assert!(requested.is_compatible_with(&server));
    }

    #[test]
    fn compatible_empty_requested_ext() {
        let requested = JsonApiMediaType {
            ext: vec![],
            profile: vec![],
        };
        let server = JsonApiMediaType {
            ext: vec!["https://e1".into()],
            profile: vec![],
        };
        assert!(requested.is_compatible_with(&server));
    }

    // --- negotiate_accept ---

    #[test]
    fn negotiate_bare_accept() {
        let mt = negotiate_accept("application/vnd.api+json", &[], &[]).unwrap();
        assert!(mt.ext.is_empty());
        assert!(mt.profile.is_empty());
    }

    #[test]
    fn negotiate_with_server_capabilities() {
        let mt =
            negotiate_accept("application/vnd.api+json", &["https://e1"], &["https://p1"]).unwrap();
        assert_eq!(mt.ext, vec!["https://e1"]);
        assert_eq!(mt.profile, vec!["https://p1"]);
    }

    #[test]
    fn negotiate_wildcard_accepts() {
        let mt = negotiate_accept("*/*", &["https://e1"], &[]).unwrap();
        assert_eq!(mt.ext, vec!["https://e1"]);
    }

    #[test]
    fn negotiate_application_wildcard() {
        let mt = negotiate_accept("application/*", &[], &["https://p1"]).unwrap();
        assert_eq!(mt.profile, vec!["https://p1"]);
    }

    #[test]
    fn negotiate_multiple_entries_first_valid_wins() {
        let mt = negotiate_accept(
            "text/html, application/vnd.api+json; ext=\"https://e1\"",
            &["https://e1"],
            &[],
        )
        .unwrap();
        assert_eq!(mt.ext, vec!["https://e1"]);
    }

    #[test]
    fn negotiate_skips_entries_with_unknown_params() {
        let mt = negotiate_accept(
            "application/vnd.api+json; charset=utf-8, application/vnd.api+json",
            &[],
            &[],
        )
        .unwrap();
        assert!(mt.ext.is_empty());
    }

    #[test]
    fn negotiate_all_jsonapi_have_unknown_params_is_406() {
        let err =
            negotiate_accept("application/vnd.api+json; charset=utf-8", &[], &[]).unwrap_err();
        assert!(matches!(err, crate::Error::AllMediaTypesUnsupportedParams));
    }

    #[test]
    fn negotiate_no_jsonapi_at_all_is_error() {
        let err = negotiate_accept("text/html, application/xml", &[], &[]).unwrap_err();
        assert!(matches!(err, crate::Error::NoAcceptableMediaType));
    }

    #[test]
    fn negotiate_empty_accept_is_error() {
        let err = negotiate_accept("", &[], &[]).unwrap_err();
        assert!(matches!(err, crate::Error::NoAcceptableMediaType));
    }

    // --- round-trip ---

    #[test]
    fn round_trip() {
        let original =
            "application/vnd.api+json; ext=\"https://e1 https://e2\"; profile=\"https://p1\"";
        let mt = JsonApiMediaType::parse(original).unwrap();
        let reparsed = JsonApiMediaType::parse(&mt.to_header_value()).unwrap();
        assert_eq!(mt, reparsed);
    }

    // --- plain / with_ext constructors ---

    #[test]
    fn plain_media_type_has_no_params() {
        let mt = JsonApiMediaType::plain();
        assert!(mt.ext.is_empty());
        assert!(mt.profile.is_empty());
        assert_eq!(mt.to_header_value(), "application/vnd.api+json");
    }

    #[test]
    fn with_ext_single_uri() {
        let mt = JsonApiMediaType::with_ext(["https://jsonapi.org/ext/atomic"]);
        assert_eq!(mt.ext, vec!["https://jsonapi.org/ext/atomic".to_string()]);
        assert!(mt.profile.is_empty());
    }

    #[test]
    fn with_ext_multiple_uris() {
        let mt = JsonApiMediaType::with_ext(["uri1", "uri2"]);
        assert_eq!(mt.ext, vec!["uri1".to_string(), "uri2".to_string()]);
    }

    #[test]
    fn with_ext_from_owned_strings() {
        let uris: Vec<String> = vec!["a".into(), "b".into()];
        let mt = JsonApiMediaType::with_ext(uris);
        assert_eq!(mt.ext, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn with_ext_round_trips_through_header_value() {
        let original = JsonApiMediaType::with_ext(["https://jsonapi.org/ext/atomic"]);
        let header = original.to_header_value();
        let parsed = JsonApiMediaType::parse(&header).unwrap();
        assert_eq!(parsed, original);
    }
}
