//! `SumSub` document API client.
//!
//! Provides [`SumsubDocumentClient`] for listing applicant documents and
//! fetching document image bytes as a streaming [`reqwest::Response`].
//!
//! The real `SumSub` API requires HMAC-signed requests; this client targets the
//! internal mock server and uses a simple `X-App-Token` header for now.
//! Request signing is stubbed in [`sign_request`] for future wiring.

use serde::{Deserialize, Serialize};

/// A single document record returned by the `SumSub` documents endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SumsubDocument {
    pub doc_id: String,
    pub kind: String,
    pub country: Option<String>,
    pub side: Option<String>,
    pub content_type: String,
    pub captured_at: String,
}

/// The streaming image result returned by [`SumsubDocumentClient::fetch_image`].
///
/// Callers may stream the image body via `response.bytes_stream()`.
pub struct SumsubImage {
    pub content_type: String,
    pub response: reqwest::Response,
}

/// HTTP client for the `SumSub` document API (mock endpoints).
#[derive(Clone)]
pub struct SumsubDocumentClient {
    base_url: String,
    app_token: String,
    secret_key: String,
    http: reqwest::Client,
}

#[derive(Deserialize)]
struct DocumentsPayload {
    documents: Vec<SumsubDocument>,
}

impl SumsubDocumentClient {
    /// Create a new client targeting `base_url`.
    #[must_use]
    pub fn new(base_url: String, app_token: String, secret_key: String) -> Self {
        Self {
            base_url,
            app_token,
            secret_key,
            http: reqwest::Client::new(),
        }
    }

    /// List all documents for `applicant_id`.
    ///
    /// # Errors
    ///
    /// Returns [`SumsubDocumentError::NotFound`] on HTTP 404,
    /// [`SumsubDocumentError::ProviderAuth`] on HTTP 401/403, and
    /// [`SumsubDocumentError::Upstream`] on other failures.
    pub async fn list_documents(
        &self,
        applicant_id: &str,
    ) -> Result<Vec<SumsubDocument>, SumsubDocumentError> {
        let url = format!(
            "{}/mock/sumsub/applicants/{applicant_id}/documents",
            self.base_url
        );
        let request = self.http.get(&url).header("X-App-Token", &self.app_token);
        let request = sign_request(request, &self.secret_key);
        let response = request
            .send()
            .await
            .map_err(|e| SumsubDocumentError::Upstream(e.to_string()))?;

        match response.status().as_u16() {
            200 => {}
            404 => return Err(SumsubDocumentError::NotFound),
            401 | 403 => return Err(SumsubDocumentError::ProviderAuth),
            other => {
                return Err(SumsubDocumentError::Upstream(format!(
                    "unexpected status {other}"
                )));
            }
        }

        let payload: DocumentsPayload = response
            .json()
            .await
            .map_err(|e| SumsubDocumentError::Upstream(e.to_string()))?;
        Ok(payload.documents)
    }

    /// Fetch the image for `doc_id` of `applicant_id` as a streaming response.
    ///
    /// The response body is **not** buffered — callers stream it via
    /// `image.response.bytes_stream()`.
    ///
    /// # Errors
    ///
    /// Returns [`SumsubDocumentError::NotFound`] on HTTP 404,
    /// [`SumsubDocumentError::ProviderAuth`] on HTTP 401/403, and
    /// [`SumsubDocumentError::Upstream`] on other failures.
    pub async fn fetch_image(
        &self,
        applicant_id: &str,
        doc_id: &str,
    ) -> Result<SumsubImage, SumsubDocumentError> {
        let url = format!(
            "{}/mock/sumsub/applicants/{applicant_id}/documents/{doc_id}/image",
            self.base_url
        );
        let request = self.http.get(&url).header("X-App-Token", &self.app_token);
        let request = sign_request(request, &self.secret_key);
        let response = request
            .send()
            .await
            .map_err(|e| SumsubDocumentError::Upstream(e.to_string()))?;

        match response.status().as_u16() {
            200 => {}
            404 => return Err(SumsubDocumentError::NotFound),
            401 | 403 => return Err(SumsubDocumentError::ProviderAuth),
            other => {
                return Err(SumsubDocumentError::Upstream(format!(
                    "unexpected status {other}"
                )));
            }
        }

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_owned();

        Ok(SumsubImage {
            content_type,
            response,
        })
    }
}

/// Stub for per-request HMAC signing required by the real `SumSub` API.
///
/// For the mock server this is a no-op; wire real signing logic here when
/// targeting the live `SumSub` API.
fn sign_request(
    builder: reqwest::RequestBuilder,
    _secret_key: &str,
) -> reqwest::RequestBuilder {
    // TODO: compute `X-App-Access-Sig` and `X-App-Access-Ts` headers when
    // targeting the production SumSub API.
    builder
}

/// Errors returned by [`SumsubDocumentClient`].
#[derive(Debug)]
pub enum SumsubDocumentError {
    /// The requested applicant or document was not found.
    NotFound,
    /// The provider rejected the request due to authentication failure.
    ProviderAuth,
    /// An upstream or transport error occurred.
    Upstream(String),
}

impl std::fmt::Display for SumsubDocumentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => f.write_str("document not found"),
            Self::ProviderAuth => f.write_str("provider authentication failed"),
            Self::Upstream(msg) => write!(f, "upstream error: {msg}"),
        }
    }
}

impl std::error::Error for SumsubDocumentError {}
