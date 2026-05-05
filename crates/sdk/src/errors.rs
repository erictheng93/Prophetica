use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BootstrapError {
    #[error("configuration error: {0}")]
    Config(String),
}

#[derive(Debug, Serialize)]
pub struct ErrorEnvelope {
    pub code: &'static str,
    pub message: String,
}

impl From<BootstrapError> for ErrorEnvelope {
    fn from(value: BootstrapError) -> Self {
        Self {
            code: "BOOTSTRAP_ERROR",
            message: value.to_string(),
        }
    }
}
