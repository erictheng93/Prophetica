use axum::{
    Json, Router,
    body::Bytes,
    extract::Path,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
};
use sdk::masking::mask_email;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::json;

#[must_use]
pub fn internal_contacts_router() -> Router {
    Router::new().route(
        "/v1/internal/users/*operation",
        post(dispatch_contact_operation),
    )
}

#[derive(Debug, Deserialize)]
struct LookupRequest {
    ids: Vec<String>,
}

#[derive(Debug, Serialize)]
struct LookupResponse {
    contacts: Vec<MaskedContact>,
}

#[derive(Debug, Serialize)]
struct MaskedContact {
    id: String,
    email_masked: String,
}

#[derive(Debug, Deserialize)]
struct RevealRequest {
    ids: Vec<String>,
    actor_id: String,
    second_authorization_id: String,
    job_id: String,
}

async fn dispatch_contact_operation(Path(operation): Path<String>, body: Bytes) -> Response {
    match operation.as_str() {
        "contacts:lookup-masked" => match parse_json::<LookupRequest>(&body) {
            Ok(request) => lookup_masked(request),
            Err(response) => response,
        },
        "contacts:reveal" => match parse_json::<RevealRequest>(&body) {
            Ok(request) => reveal(request),
            Err(response) => response,
        },
        _ => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "ERROR_CODE_CONTACT_OPERATION_NOT_FOUND" })),
        )
            .into_response(),
    }
}

fn parse_json<T: DeserializeOwned>(body: &[u8]) -> Result<T, Response> {
    serde_json::from_slice(body).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "ERROR_CODE_INVALID_JSON" })),
        )
            .into_response()
    })
}

fn lookup_masked(request: LookupRequest) -> Response {
    if request.ids.iter().any(|id| id.trim().is_empty()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "ERROR_CODE_EMPTY_CONTACT_ID" })),
        )
            .into_response();
    }

    let contacts = request
        .ids
        .into_iter()
        .map(|id| MaskedContact {
            email_masked: mask_email(&email_for_member(&id)),
            id,
        })
        .collect();

    Json(LookupResponse { contacts }).into_response()
}

fn reveal(request: RevealRequest) -> Response {
    tracing::warn!(
        actor_id = %request.actor_id,
        job_id = %request.job_id,
        ids_count = request.ids.len(),
        second_authorization_id = %request.second_authorization_id,
        "contacts:reveal attempted before plaintext PII reveal is implemented"
    );
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({ "error": "ERROR_CODE_REVEAL_NOT_AVAILABLE" })),
    )
        .into_response()
}

fn email_for_member(id: &str) -> String {
    let suffix = id.strip_prefix("MEM-").unwrap_or(id).to_ascii_lowercase();
    format!("member-{suffix}@example.com")
}
