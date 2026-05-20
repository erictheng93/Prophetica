use axum::{
    Json, Router,
    extract::{Path, State},
    response::IntoResponse,
    routing::{get, post},
};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha1::Sha1;

#[derive(Debug, Deserialize)]
pub struct ApplicantReviewedRequest {
    pub applicant_id: String,
    pub external_user_id: String,
    pub review_answer: String,
    #[serde(default)]
    pub kyc_reason: String,
    #[serde(default)]
    pub sanction_matches: Vec<Value>,
}

#[derive(Debug, Serialize)]
pub struct MockWebhookResponse {
    pub headers: MockWebhookHeaders,
    pub body: Value,
}

#[derive(Debug, Serialize)]
pub struct MockWebhookHeaders {
    #[serde(rename = "x-payload-digest")]
    pub payload_digest: String,
}

#[derive(Clone)]
struct MockState {
    webhook_secret: String,
}

pub fn router(secret: String) -> Router {
    Router::new()
        .route("/mock/sumsub/applicant-reviewed", post(applicant_reviewed))
        .route(
            "/mock/sumsub/applicants/:applicant_id/documents",
            get(mock_document_list),
        )
        .route(
            "/mock/sumsub/applicants/:applicant_id/documents/:doc_id/image",
            get(mock_document_image),
        )
        .with_state(MockState {
            webhook_secret: secret,
        })
}

async fn applicant_reviewed(
    State(state): State<MockState>,
    Json(request): Json<ApplicantReviewedRequest>,
) -> Json<MockWebhookResponse> {
    let body = json!({
        "applicantId": request.applicant_id,
        "externalUserId": request.external_user_id,
        "type": "applicantReviewed",
        "reviewResult": {
            "reviewAnswer": request.review_answer
        },
        "kycReason": request.kyc_reason,
        "sanctionMatches": request.sanction_matches
    });
    let payload = body.to_string();

    Json(MockWebhookResponse {
        headers: MockWebhookHeaders {
            payload_digest: sign_payload(&state.webhook_secret, payload.as_bytes()),
        },
        body,
    })
}

async fn mock_document_list(Path(applicant_id): Path<String>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "applicant_id": applicant_id,
        "documents": [
            { "doc_id": "doc_1", "kind": "id_doc", "country": "TWN", "side": "front",
              "content_type": "image/png", "captured_at": "2026-05-10T00:00:00Z" },
            { "doc_id": "doc_2", "kind": "selfie", "country": null, "side": null,
              "content_type": "image/png", "captured_at": "2026-05-10T00:01:00Z" }
        ]
    }))
}

const MOCK_PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE,
    0x42, 0x60, 0x82,
];

async fn mock_document_image(
    Path((_applicant, _doc)): Path<(String, String)>,
) -> impl IntoResponse {
    ([(axum::http::header::CONTENT_TYPE, "image/png")], MOCK_PNG)
}

fn sign_payload(secret: &str, payload: &[u8]) -> String {
    let Ok(mut mac) = Hmac::<Sha1>::new_from_slice(secret.as_bytes()) else {
        return String::new();
    };
    mac.update(payload);
    hex::encode(mac.finalize().into_bytes())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use tower::ServiceExt;

    #[tokio::test]
    async fn applicant_review_endpoint_returns_signed_sumsub_payload() {
        let response = crate::router("dev-sumsub-webhook-secret".to_owned())
            .oneshot(json_post(&json!({
                "applicant_id": "applicant_42",
                "external_user_id": "user_42",
                "review_answer": "GREEN",
                "kyc_reason": "",
                "sanction_matches": []
            })))
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(
            body["headers"]["x-payload-digest"].as_str().map(str::len),
            Some(40)
        );
        assert_eq!(body["body"]["applicantId"], json!("applicant_42"));
        assert_eq!(body["body"]["externalUserId"], json!("user_42"));
        assert_eq!(body["body"]["reviewResult"]["reviewAnswer"], json!("GREEN"));
    }

    #[tokio::test]
    async fn serves_document_list_and_image() {
        let app = crate::router("test-secret".to_owned());
        let list = app
            .clone()
            .oneshot(
                axum::http::Request::get("/mock/sumsub/applicants/applicant_1/documents")
                    .body(axum::body::Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should be handled");
        assert_eq!(list.status(), axum::http::StatusCode::OK);
        let img = app
            .oneshot(
                axum::http::Request::get(
                    "/mock/sumsub/applicants/applicant_1/documents/doc_1/image",
                )
                .body(axum::body::Body::empty())
                .expect("request should build"),
            )
            .await
            .expect("request should be handled");
        assert_eq!(img.status(), axum::http::StatusCode::OK);
        assert_eq!(
            img.headers()
                .get("content-type")
                .expect("content-type header must be present"),
            "image/png"
        );
    }

    fn json_post(body: &Value) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri("/mock/sumsub/applicant-reviewed")
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .expect("request should build")
    }

    async fn response_json(response: axum::response::Response) -> Value {
        let collected = response
            .into_body()
            .collect()
            .await
            .expect("response body should collect");
        serde_json::from_slice(&collected.to_bytes()).expect("response should be json")
    }
}
