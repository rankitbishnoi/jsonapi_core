//! Domain-level error mapped to JSON:API via [`IntoJsonApiError`].
use jsonapi_axum::{ApiErrorExt, IntoJsonApiError, JsonApiError, with_status};

#[derive(Debug)]
pub enum AppError {
    BadQuery(String),
}

impl IntoJsonApiError for AppError {
    fn into_json_api_error(self) -> JsonApiError {
        match self {
            AppError::BadQuery(detail) => {
                JsonApiError::from_api_error(with_status(400).detail(detail).title("Invalid query"))
            }
        }
    }
}
