use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use rustpress_core::error::RustPressError;

/// HTTP error wrapper around `RustPressError` with `IntoResponse` for Axum.
pub struct AppError(pub RustPressError);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self.0 {
            RustPressError::NotFound(_) => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, self.0.to_string()).into_response()
    }
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        AppError(RustPressError::Internal(err.to_string()))
    }
}

impl From<RustPressError> for AppError {
    fn from(err: RustPressError) -> Self {
        AppError(err)
    }
}
