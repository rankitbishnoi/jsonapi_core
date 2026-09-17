//! Pure helpers for turning HTTP metadata into inspector records. No network,
//! no wasm — unit-testable on the host.

use crate::inspector::model::Header;

pub const MEDIA_TYPE: &str = "application/vnd.api+json";

/// Pretty-print a JSON:API body if it parses as JSON; otherwise return it as-is.
/// Empty input yields `None`.
pub fn pretty_body(raw: &str) -> Option<String> {
    if raw.trim().is_empty() {
        return None;
    }
    match serde_json::from_str::<serde_json::Value>(raw) {
        Ok(v) => Some(serde_json::to_string_pretty(&v).unwrap_or_else(|_| raw.to_string())),
        Err(_) => Some(raw.to_string()),
    }
}

/// Case-insensitively find the `x-request-id` value in a header list.
pub fn request_id(headers: &[Header]) -> Option<String> {
    headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case("x-request-id"))
        .map(|h| h.value.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pretty_body_formats_json() {
        let out = pretty_body(r#"{"data":{"id":"1"}}"#).unwrap();
        assert!(out.contains("\"data\""));
        assert!(out.contains('\n'), "pretty output is multi-line");
    }

    #[test]
    fn pretty_body_none_on_empty() {
        assert_eq!(pretty_body("   "), None);
    }

    #[test]
    fn pretty_body_passthrough_on_non_json() {
        assert_eq!(pretty_body("not json").as_deref(), Some("not json"));
    }

    #[test]
    fn request_id_is_case_insensitive() {
        let headers = vec![Header {
            name: "X-Request-Id".into(),
            value: "abc-123".into(),
        }];
        assert_eq!(request_id(&headers).as_deref(), Some("abc-123"));
    }

    #[test]
    fn request_id_absent() {
        assert_eq!(request_id(&[]), None);
    }
}
