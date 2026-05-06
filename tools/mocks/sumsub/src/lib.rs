use axum::{Json, Router, routing::post};
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

pub fn router() -> Router {
    Router::new().route("/mock/sumsub/applicant-reviewed", post(applicant_reviewed))
}

async fn applicant_reviewed(
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
            payload_digest: sign_payload(payload.as_bytes()),
        },
        body,
    })
}

fn sign_payload(payload: &[u8]) -> String {
    let secret = std::env::var("SUMSUB_WEBHOOK_SECRET")
        .unwrap_or_else(|_| "dev-sumsub-webhook-secret".to_owned());
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
        let response = crate::router()
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
