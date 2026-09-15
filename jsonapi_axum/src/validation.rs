//! Optional [`validator`] integration (feature `validator`).
//!
//! Turns a [`validator::ValidationErrors`] into a list of JSON:API
//! [`ApiError`]s — one `422 Unprocessable Entity` per field violation, with
//! `source.pointer` set to `/data/attributes/<field>` — so a failed request-body
//! validation renders as a single spec-shaped errors document.

use jsonapi_core::ApiError;
use jsonapi_http::{ApiErrorExt, with_status};
use validator::ValidationErrors;

/// Convert field-level [`validator::ValidationErrors`] into JSON:API
/// [`ApiError`]s.
///
/// Each violation becomes a `422` error whose `source.pointer` is
/// `/data/attributes/<field>`. The error `detail` is the validator's custom
/// message when present, otherwise its `code`; `code` is always carried through.
/// Output is ordered by field name so the document is deterministic.
///
/// Nested (`Struct`/`List`) validation errors are not expanded — only the
/// top-level field errors are mapped, which matches flat JSON:API attribute
/// validation. Feed the result to `JsonApiError::from_api_errors` (or collect it
/// into [`ApiErrors`](jsonapi_http::ApiErrors)) to build the response.
#[must_use]
pub fn from_validation_errors(errors: &ValidationErrors) -> Vec<ApiError> {
    let mut fields: Vec<_> = errors.field_errors().into_iter().collect();
    fields.sort_by(|(a, _), (b, _)| a.cmp(b));

    let mut api_errors = Vec::new();
    for (field, violations) in fields {
        for violation in violations {
            let detail = violation
                .message
                .as_ref()
                .map_or_else(|| violation.code.to_string(), |m| m.to_string());
            api_errors.push(
                with_status(422)
                    .pointer(format!("/data/attributes/{field}"))
                    .code(violation.code.to_string())
                    .detail(detail),
            );
        }
    }
    api_errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Cow;
    use validator::ValidationError;

    fn field_error(code: &'static str, message: Option<&'static str>) -> ValidationError {
        ValidationError {
            code: Cow::Borrowed(code),
            message: message.map(Cow::Borrowed),
            params: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn two_invalid_fields_map_to_two_422_errors_with_pointers() {
        let mut errors = ValidationErrors::new();
        errors.add("title", field_error("length", Some("must not be empty")));
        errors.add("age", field_error("range", None));

        let api = from_validation_errors(&errors);
        assert_eq!(api.len(), 2);

        // Ordered by field name: "age" before "title".
        assert_eq!(api[0].status.as_deref(), Some("422"));
        assert_eq!(
            api[0].source.as_ref().unwrap().pointer.as_deref(),
            Some("/data/attributes/age")
        );
        assert_eq!(api[0].detail.as_deref(), Some("range")); // no message → code
        assert_eq!(api[0].code.as_deref(), Some("range"));

        assert_eq!(
            api[1].source.as_ref().unwrap().pointer.as_deref(),
            Some("/data/attributes/title")
        );
        assert_eq!(api[1].detail.as_deref(), Some("must not be empty"));
    }
}
