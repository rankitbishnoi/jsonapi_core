//! Parse an incoming HTTP request into typed JSON:API values.
//!
//! Every function here operates on borrowed [`http`] types so any adapter can
//! reuse it. The adapter is responsible for draining the request body into
//! [`Bytes`] before calling [`deserialize_body`] (and for enforcing a body-size
//! limit — this layer never buffers the body itself).

use bytes::Bytes;
use http::{HeaderMap, Uri, header};
use serde::de::DeserializeOwned;

use jsonapi_core::{
    ApiError, Document, Error, JsonApiMediaType, Query, ResourceObject, negotiate_accept,
    validate_content_type,
};

use crate::JSON_API_MEDIA_TYPE;
use crate::error::to_api_error;

/// Validate the request `Content-Type` is `application/vnd.api+json`
/// (reusing [`jsonapi_core::validate_content_type`]).
///
/// A missing header, a non-UTF-8 header, or a non-conforming media type all
/// yield an [`Error`] that [`status_for`](crate::status_for) maps to
/// `415 Unsupported Media Type` (or `400` for an unparseable header).
pub fn check_content_type(headers: &HeaderMap) -> Result<JsonApiMediaType, Error> {
    let value = headers
        .get(header::CONTENT_TYPE)
        .ok_or_else(|| Error::MediaTypeMismatch {
            expected: JSON_API_MEDIA_TYPE.to_string(),
            got: "(missing)".to_string(),
        })?;

    let text = value
        .to_str()
        .map_err(|_| Error::MediaTypeParse("Content-Type header is not valid UTF-8".to_string()))?;

    validate_content_type(text)
}

/// Negotiate a response media type from the request `Accept` header
/// (reusing [`jsonapi_core::negotiate_accept`]), intersecting the client's
/// request with the server's advertised `server_ext` / `server_profile`.
///
/// A missing `Accept` header means the client accepts anything, so a plain
/// `application/vnd.api+json` media type is returned. A present-but-unacceptable
/// header yields an [`Error`] that maps to `406 Not Acceptable`.
pub fn negotiate(
    headers: &HeaderMap,
    server_ext: &[&str],
    server_profile: &[&str],
) -> Result<JsonApiMediaType, Error> {
    match headers.get(header::ACCEPT) {
        None => Ok(JsonApiMediaType::plain()),
        Some(value) => {
            let text = value.to_str().map_err(|_| {
                Error::MediaTypeParse("Accept header is not valid UTF-8".to_string())
            })?;
            negotiate_accept(text, server_ext, server_profile)
        }
    }
}

/// Parse the query string of `uri` into a typed [`Query`]
/// (reusing [`jsonapi_core::Query::from_query_string`]).
///
/// `filter[...]` is surfaced verbatim as a generic map — this layer does no
/// interpretation. A malformed parameter yields an [`Error::QueryParse`] whose
/// `ApiError` carries the offending `source.parameter`.
pub fn parse_query(uri: &Uri) -> Result<Query, Error> {
    Query::from_query_string(uri.query().unwrap_or(""))
}

/// Deserialize an already-collected request body into a typed [`Document<T>`],
/// mapping parse failures to an [`ApiError`] carrying the appropriate
/// `source.pointer` (via [`to_api_error`]).
///
/// The error is boxed because [`ApiError`] is large and would otherwise bloat
/// every `Ok` value on the stack (`clippy::result_large_err`).
pub fn deserialize_body<T>(body: &Bytes) -> Result<Document<T>, Box<ApiError>>
where
    T: ResourceObject + DeserializeOwned,
{
    Document::<T>::from_slice(body).map_err(|err| Box::new(to_api_error(&err)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::HeaderValue;
    use jsonapi_core::Resource;

    fn headers_with(name: header::HeaderName, value: &str) -> HeaderMap {
        let mut map = HeaderMap::new();
        map.insert(name, HeaderValue::from_str(value).unwrap());
        map
    }

    #[test]
    fn check_content_type_accepts_json_api() {
        let headers = headers_with(header::CONTENT_TYPE, JSON_API_MEDIA_TYPE);
        assert!(check_content_type(&headers).is_ok());
    }

    #[test]
    fn check_content_type_rejects_wrong_type() {
        let headers = headers_with(header::CONTENT_TYPE, "application/json");
        let err = check_content_type(&headers).unwrap_err();
        assert!(matches!(err, Error::MediaTypeMismatch { .. }));
    }

    #[test]
    fn check_content_type_rejects_missing_header() {
        let err = check_content_type(&HeaderMap::new()).unwrap_err();
        assert!(matches!(err, Error::MediaTypeMismatch { got, .. } if got == "(missing)"));
    }

    #[test]
    fn negotiate_absent_accept_yields_plain() {
        let media = negotiate(&HeaderMap::new(), &[], &[]).unwrap();
        assert_eq!(media, JsonApiMediaType::plain());
    }

    #[test]
    fn negotiate_accepts_json_api() {
        let headers = headers_with(header::ACCEPT, JSON_API_MEDIA_TYPE);
        assert!(negotiate(&headers, &[], &[]).is_ok());
    }

    #[test]
    fn negotiate_rejects_unacceptable() {
        let headers = headers_with(header::ACCEPT, "text/html");
        let err = negotiate(&headers, &[], &[]).unwrap_err();
        assert!(matches!(err, Error::NoAcceptableMediaType));
    }

    #[test]
    fn negotiate_intersects_requested_ext_with_server_capabilities() {
        // Client requests two extensions; the server advertises only one, so the
        // negotiated media type carries just the supported one (the other is
        // dropped, not an error).
        let headers = headers_with(
            header::ACCEPT,
            "application/vnd.api+json; ext=\"https://srv/ext https://other/ext\"",
        );
        let media = negotiate(&headers, &["https://srv/ext"], &[]).unwrap();
        assert_eq!(media.ext, vec!["https://srv/ext"]);
        assert!(media.profile.is_empty());
    }

    #[test]
    fn negotiate_drops_profile_the_server_does_not_advertise() {
        // Requested profile the server never advertises is dropped, leaving a
        // plain media type rather than echoing an unsupported profile back.
        let headers = headers_with(
            header::ACCEPT,
            "application/vnd.api+json; profile=\"https://unknown/p\"",
        );
        let media = negotiate(&headers, &[], &[]).unwrap();
        assert!(media.profile.is_empty());
    }

    #[test]
    fn check_content_type_rejects_non_utf8_header_with_media_type_parse() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_bytes(&[0xff, 0xfe]).unwrap(),
        );
        let err = check_content_type(&headers).unwrap_err();
        assert!(matches!(err, Error::MediaTypeParse(_)));
    }

    #[test]
    fn negotiate_rejects_non_utf8_accept_with_media_type_parse() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::ACCEPT,
            HeaderValue::from_bytes(&[0xff, 0xfe]).unwrap(),
        );
        let err = negotiate(&headers, &[], &[]).unwrap_err();
        assert!(matches!(err, Error::MediaTypeParse(_)));
    }

    #[test]
    fn parse_query_reads_sort_and_page() {
        let uri: Uri = "/articles?sort=-created&page[size]=2".parse().unwrap();
        let query = parse_query(&uri).unwrap();
        assert_eq!(query.sort.len(), 1);
    }

    #[test]
    fn parse_query_empty_uri_is_ok() {
        let uri: Uri = "/articles".parse().unwrap();
        assert!(parse_query(&uri).is_ok());
    }

    #[test]
    fn deserialize_body_parses_valid_document() {
        let body = Bytes::from_static(br#"{"data":{"type":"articles","id":"1"}}"#);
        let document = deserialize_body::<Resource>(&body).unwrap();
        assert!(matches!(document, Document::Data { .. }));
    }

    #[test]
    fn deserialize_body_maps_malformed_json_to_api_error() {
        let body = Bytes::from_static(b"not json");
        let err = deserialize_body::<Resource>(&body).unwrap_err();
        assert_eq!(err.status.as_deref(), Some("400"));
    }
}
