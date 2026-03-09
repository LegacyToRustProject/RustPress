use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use rustpress_core::error::RustPressError;

#[allow(dead_code)]
pub struct AppError(pub RustPressError);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self.0 {
            RustPressError::NotFound(_) => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        // H4: log full error internally but never expose DB paths/schema to clients.
        tracing::error!(error = %self.0, status = %status, "request error");
        let body = match status {
            StatusCode::NOT_FOUND => "Not found.",
            _ => "An internal server error occurred.",
        };
        (status, body).into_response()
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
