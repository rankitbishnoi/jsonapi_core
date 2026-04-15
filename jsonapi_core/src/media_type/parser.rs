/// Parse a media type header into base type and parameter list.
///
/// Example: `application/vnd.api+json; ext="uri1 uri2"`
/// → `("application/vnd.api+json", [("ext", "uri1 uri2")])`
///
/// Handles quoted strings with `\"` escaping and bare token values.
pub(crate) fn parse_media_type_params(header: &str) -> crate::Result<(&str, Vec<(&str, String)>)> {
    let header = header.trim();
    if header.is_empty() {
        return Err(crate::Error::MediaTypeParse("empty media type".into()));
    }

    // Split base type from parameters at first ';'
    let (base, rest) = match header.find(';') {
        Some(pos) => (header[..pos].trim(), &header[pos + 1..]),
        None => return Ok((header, vec![])),
    };

    let mut params = Vec::new();
    let mut remaining = rest;

    while !remaining.trim().is_empty() {
        remaining = remaining.trim_start();

        // Find '=' separating key from value
        let eq_pos = remaining.find('=').ok_or_else(|| {
            crate::Error::MediaTypeParse(format!("parameter missing '=': {remaining}"))
        })?;
        let key = remaining[..eq_pos].trim();
        if key.is_empty() {
            return Err(crate::Error::MediaTypeParse("empty parameter key".into()));
        }
        remaining = remaining[eq_pos + 1..].trim_start();

        // Parse value: quoted string or bare token
        let (value, after) = if let Some(after_quote) = remaining.strip_prefix('"') {
            parse_quoted_string(after_quote)?
        } else {
            parse_bare_token(remaining)
        };

        params.push((key, value));
        remaining = after.trim_start();
        if remaining.starts_with(';') {
            remaining = &remaining[1..];
        }
    }

    Ok((base, params))
}

/// Parse a quoted string. Input starts after the opening `"`.
/// Returns `(unescaped_value, remaining_input_after_closing_quote)`.
fn parse_quoted_string(input: &str) -> crate::Result<(String, &str)> {
    let mut value = String::new();
    let mut chars = input.char_indices();

    while let Some((i, c)) = chars.next() {
        match c {
            '"' => return Ok((value, &input[i + 1..])),
            '\\' => {
                if let Some((_, escaped)) = chars.next() {
                    value.push(escaped);
                } else {
                    return Err(crate::Error::MediaTypeParse(
                        "unterminated escape in quoted string".into(),
                    ));
                }
            }
            _ => value.push(c),
        }
    }

    Err(crate::Error::MediaTypeParse(
        "unterminated quoted string".into(),
    ))
}

/// Parse a bare (unquoted) token value. Ends at `;` or end of input.
fn parse_bare_token(input: &str) -> (String, &str) {
    match input.find(';') {
        Some(pos) => (input[..pos].trim().to_string(), &input[pos..]),
        None => (input.trim().to_string(), ""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_type_only() {
        let (base, params) = parse_media_type_params("application/json").unwrap();
        assert_eq!(base, "application/json");
        assert!(params.is_empty());
    }

    #[test]
    fn base_type_trimmed() {
        let (base, params) = parse_media_type_params("  application/json  ").unwrap();
        assert_eq!(base, "application/json");
        assert!(params.is_empty());
    }

    #[test]
    fn single_quoted_param() {
        let (base, params) =
            parse_media_type_params("application/vnd.api+json; ext=\"uri1\"").unwrap();
        assert_eq!(base, "application/vnd.api+json");
        assert_eq!(params, vec![("ext", "uri1".to_string())]);
    }

    #[test]
    fn multiple_params() {
        let (base, params) = parse_media_type_params(
            "application/vnd.api+json; ext=\"uri1 uri2\"; profile=\"uri3\"",
        )
        .unwrap();
        assert_eq!(base, "application/vnd.api+json");
        assert_eq!(
            params,
            vec![
                ("ext", "uri1 uri2".to_string()),
                ("profile", "uri3".to_string()),
            ]
        );
    }

    #[test]
    fn bare_token_value() {
        let (base, params) = parse_media_type_params("text/html; charset=utf-8").unwrap();
        assert_eq!(base, "text/html");
        assert_eq!(params, vec![("charset", "utf-8".to_string())]);
    }

    #[test]
    fn escaped_quote_in_value() {
        let (_, params) =
            parse_media_type_params("application/vnd.api+json; ext=\"has\\\"quote\"").unwrap();
        assert_eq!(params[0].1, "has\"quote");
    }

    #[test]
    fn whitespace_around_equals() {
        let (_, params) = parse_media_type_params("application/json; charset = utf-8").unwrap();
        assert_eq!(params, vec![("charset", "utf-8".to_string())]);
    }

    #[test]
    fn whitespace_around_semicolons() {
        let (base, params) =
            parse_media_type_params("application/json ;  charset=utf-8  ;  boundary=something")
                .unwrap();
        assert_eq!(base, "application/json");
        assert_eq!(params.len(), 2);
        assert_eq!(params[0], ("charset", "utf-8".to_string()));
        assert_eq!(params[1], ("boundary", "something".to_string()));
    }

    #[test]
    fn empty_input_is_error() {
        assert!(parse_media_type_params("").is_err());
    }

    #[test]
    fn unterminated_quoted_string_is_error() {
        assert!(parse_media_type_params("application/json; ext=\"unterminated").is_err());
    }

    #[test]
    fn empty_parameter_key_is_error() {
        let result = parse_media_type_params("application/json; =value");
        assert!(result.is_err());
    }
}
