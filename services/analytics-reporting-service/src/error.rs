use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ReportingError {
    #[error("forbidden")]
    Forbidden,
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("not found")]
    NotFound,
    #[error("reveal not available")]
    RevealNotAvailable,
    #[error("upstream unavailable: {0}")]
    Upstream(String),
    #[error("store error: {0}")]
    Store(String),
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    error: &'static str,
    message: String,
}

impl IntoResponse for ReportingError {
    fn into_response(self) -> Response {
        let (status, code) = match self {
            Self::Forbidden => (StatusCode::FORBIDDEN, "ERROR_CODE_REPORT_FORBIDDEN"),
            Self::InvalidRequest(_) => {
                (StatusCode::BAD_REQUEST, "ERROR_CODE_REPORT_INVALID_REQUEST")
            }
            Self::NotFound => (StatusCode::NOT_FOUND, "ERROR_CODE_REPORT_NOT_FOUND"),
            Self::RevealNotAvailable => (
                StatusCode::NOT_IMPLEMENTED,
                "ERROR_CODE_REVEAL_NOT_AVAILABLE",
            ),
            Self::Upstream(_) => (
                StatusCode::BAD_GATEWAY,
                "ERROR_CODE_REPORT_SOURCE_UNAVAILABLE",
            ),
            Self::Store(_) => (StatusCode::INTERNAL_SERVER_ERROR, "ERROR_CODE_REPORT_STORE"),
        };
        let message = self.to_string();
        (
            status,
            Json(ErrorBody {
                error: code,
                message,
            }),
        )
            .into_response()
    }
}
