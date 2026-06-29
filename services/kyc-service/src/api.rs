use axum::{
    Json, Router,
    body::{Body, Bytes},
    extract::{OriginalUri, Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::Response,
    routing::{get, post},
};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha1::Sha1;
use sha2::{Digest, Sha256};

use crate::SecretConfig;
use crate::internal_auth::{InternalCallRejection, verify_internal_call};
use crate::kyc::{
    AmlEventDraft, DocumentAccessEntry, KycRecord, KycStatus, KycStore, KycStoreError, KycTier,
    PendingMemberCasesQuery, PendingMemberCasesResponse, ProviderWebhookDraft, ReviewAnswer,
    ScreeningMatchDraft, ScreeningMatchType, StartUpgradeDraft, TierOneVerificationDraft,
    is_supported_tier_one_document,
};
use crate::provider_documents::{ProviderDocumentClient, ProviderDocumentError};

#[derive(Debug, Deserialize)]
pub struct TierOneVerificationRequest {
    pub idempotency_key: String,
    pub legal_name: String,
    pub birthday: String,
    pub country: String,
    pub document_id: String,
    pub consent: bool,
}

#[derive(Debug, Deserialize)]
pub struct StartUpgradeRequest {
    pub target_tier: KycTier,
    pub idempotency_key: String,
    pub declared_monthly_income: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct ProviderWebhookPayload {
    #[serde(rename = "applicantId")]
    pub applicant_id: String,
    #[serde(rename = "externalUserId")]
    pub external_user_id: String,
    #[serde(rename = "type")]
    pub event_type: String,
    #[serde(rename = "reviewResult")]
    pub review_result: ProviderReviewResult,
    #[serde(rename = "kycReason", default)]
    pub kyc_reason: String,
    #[serde(rename = "sanctionMatches", default)]
    pub sanction_matches: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct ProviderReviewResult {
    #[serde(rename = "reviewAnswer")]
    pub review_answer: String,
}

#[derive(Debug, Deserialize)]
pub struct AmlEventRequest {
    pub user_id: String,
    pub event_type: String,
    pub amount_minor_units: i64,
    pub ml_score: i32,
    #[serde(default)]
    pub rule_flags: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct MonthlyReportQuery {
    pub month: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ScreeningRescanRequest {
    #[serde(default)]
    pub matches: Vec<ScreeningMatchRequest>,
}

#[derive(Debug, Deserialize)]
pub struct ScreeningMatchRequest {
    pub user_id: String,
    pub match_type: String,
    pub list_name: String,
}

#[derive(Debug, Serialize)]
pub struct KycStatusResponse {
    pub user_id: String,
    pub current_tier: KycTier,
    pub status: KycStatus,
    pub provider: Option<String>,
    pub provider_applicant_id: Option<String>,
    pub country: Option<String>,
    pub tier1_record_id: Option<String>,
    pub declared_monthly_income: Option<i64>,
    pub submitted_at: Option<String>,
    pub reviewed_at: Option<String>,
    pub expires_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct KycTierResponse {
    pub tier: KycTier,
}

#[derive(Debug, Serialize)]
pub struct TierOneVerificationResponse {
    pub user_id: String,
    pub target_tier: KycTier,
    pub status: KycStatus,
    pub provider: String,
    pub document_id_hash: Option<String>,
    pub replayed: bool,
}

#[derive(Debug, Serialize)]
pub struct StartUpgradeResponse {
    pub user_id: String,
    pub target_tier: KycTier,
    pub status: KycStatus,
    pub provider: String,
    pub provider_applicant_id: Option<String>,
    pub sdk_token: String,
    pub expires_at: Option<String>,
    pub replayed: bool,
}

#[derive(Debug, Serialize)]
pub struct ProviderWebhookResponse {
    pub user_id: String,
    pub current_tier: KycTier,
    pub status: KycStatus,
    pub provider_applicant_id: Option<String>,
    pub replayed: bool,
}

#[derive(Debug, Serialize)]
pub struct AmlEventResponse {
    pub user_id: String,
    pub requires_compliance_review: bool,
    pub sar_candidate_id: Option<String>,
    pub reason: Option<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct MonthlyKycReportResponse {
    pub month: String,
    pub total_records: usize,
    pub by_tier: std::collections::HashMap<String, usize>,
    pub by_status: std::collections::HashMap<String, usize>,
    pub open_sar_candidates: i64,
}

#[derive(Debug, Serialize)]
pub struct ScreeningRescanResponse {
    pub scanned_matches: usize,
    pub frozen_users: Vec<String>,
    pub edd_users: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ApiErrorResponse {
    pub error: &'static str,
    pub message: String,
}

#[derive(Clone)]
struct ApiState<S> {
    store: S,
    webhook_secret: String,
    internal_call_secret: String,
    provider_documents: ProviderDocumentClient,
}

pub fn router<S>(store: S, secrets: SecretConfig) -> Router
where
    S: KycStore,
{
    let provider_base_url = std::env::var("KYC_PROVIDER_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:8081".to_owned());
    let provider_app_token = std::env::var("KYC_PROVIDER_APP_TOKEN")
        .unwrap_or_else(|_| "dev-provider-app-token".to_owned());
    let provider_secret_key = std::env::var("KYC_PROVIDER_SECRET_KEY")
        .unwrap_or_else(|_| "dev-provider-secret-key".to_owned());
    let provider_documents =
        ProviderDocumentClient::new(provider_base_url, provider_app_token, provider_secret_key);

    let state = ApiState {
        store,
        webhook_secret: secrets.webhook_secret,
        internal_call_secret: secrets.internal_call_secret,
        provider_documents,
    };

    build_router(state)
}

/// Like [`router`] but accepts an explicit [`ProviderDocumentClient`] instead of
/// reading env vars. Intended for integration tests that spin up a mock server.
pub fn router_with_provider_client<S>(
    store: S,
    secrets: SecretConfig,
    provider_documents: ProviderDocumentClient,
) -> Router
where
    S: KycStore,
{
    let state = ApiState {
        store,
        webhook_secret: secrets.webhook_secret,
        internal_call_secret: secrets.internal_call_secret,
        provider_documents,
    };
    build_router(state)
}

fn build_router<S>(state: ApiState<S>) -> Router
where
    S: KycStore,
{
    Router::new()
        .route("/api/v1/kyc/users/:user_id/status", get(get_status::<S>))
        .route(
            "/internal/kyc/users/:user_id/tier",
            get(get_internal_tier::<S>),
        )
        .route(
            "/api/v1/kyc/users/:user_id/tier-1",
            post(verify_tier_one::<S>),
        )
        .route(
            "/api/v1/kyc/users/:user_id/upgrades",
            post(start_upgrade::<S>),
        )
        .route(
            "/api/internal/webhooks/provider",
            post(provider_webhook::<S>),
        )
        .route("/internal/aml/events", post(record_aml_event::<S>))
        .route("/internal/screening/rescans", post(screening_rescan::<S>))
        .route("/internal/reports/kyc/monthly", get(monthly_report::<S>))
        .route(
            "/v1/internal/admin/members/pending-cases",
            get(pending_member_cases::<S>),
        )
        .route(
            "/v1/internal/admin/kyc/members/:user_id/documents",
            get(list_member_documents::<S>),
        )
        .route(
            "/v1/internal/admin/kyc/members/:user_id/documents/:doc_id",
            get(get_member_document::<S>),
        )
        .with_state(state)
}

#[tracing::instrument(fields(service.name = crate::SERVICE_NAME, proph.user_id = %user_id), skip(state))]
async fn get_status<S>(
    State(state): State<ApiState<S>>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
) -> Result<Json<KycStatusResponse>, (StatusCode, Json<ApiErrorResponse>)>
where
    S: KycStore,
{
    authorize_user_subject(&headers, &user_id)?;
    let record = state
        .store
        .current_record(&user_id)
        .await
        .map_err(store_error)?;
    let tier_one_record = state
        .store
        .tier_one_record(&user_id)
        .await
        .map_err(store_error)?;
    Ok(Json(status_response(
        &user_id,
        record.as_ref(),
        tier_one_record.as_ref(),
    )))
}

#[tracing::instrument(fields(service.name = crate::SERVICE_NAME, proph.user_id = %user_id), skip(state, headers))]
async fn get_internal_tier<S>(
    State(state): State<ApiState<S>>,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
    Path(user_id): Path<String>,
) -> Result<Json<KycTierResponse>, (StatusCode, Json<ApiErrorResponse>)>
where
    S: KycStore,
{
    verify_internal_request(
        &headers,
        "GET",
        uri.path(),
        b"",
        &state.internal_call_secret,
    )?;
    let record = state
        .store
        .current_record(&user_id)
        .await
        .map_err(store_error)?;
    Ok(Json(KycTierResponse {
        tier: record.as_ref().map_or(KycTier::Tier0, |record| record.tier),
    }))
}

async fn pending_member_cases<S>(
    State(state): State<ApiState<S>>,
    Query(query): Query<PendingMemberCasesQuery>,
) -> Result<Json<PendingMemberCasesResponse>, (StatusCode, Json<ApiErrorResponse>)>
where
    S: KycStore,
{
    state
        .store
        .pending_member_cases(query)
        .await
        .map(Json)
        .map_err(store_error)
}

#[tracing::instrument(fields(service.name = crate::SERVICE_NAME, proph.user_id = %user_id), skip(state, request))]
async fn verify_tier_one<S>(
    State(state): State<ApiState<S>>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
    Json(request): Json<TierOneVerificationRequest>,
) -> Result<(StatusCode, Json<TierOneVerificationResponse>), (StatusCode, Json<ApiErrorResponse>)>
where
    S: KycStore,
{
    authorize_user_subject(&headers, &user_id)?;
    validate_tier_one(&request)?;
    let outcome = state
        .store
        .verify_tier_one(TierOneVerificationDraft {
            user_id,
            idempotency_key: request.idempotency_key,
            legal_name: request.legal_name,
            birthday: request.birthday,
            country: request.country,
            document_id: request.document_id,
        })
        .await
        .map_err(store_error)?;

    Ok((
        if outcome.replayed {
            StatusCode::OK
        } else {
            StatusCode::CREATED
        },
        Json(TierOneVerificationResponse {
            user_id: outcome.record.user_id,
            target_tier: outcome.record.tier,
            status: outcome.record.status,
            provider: outcome.record.provider,
            document_id_hash: outcome.record.document_id_hash,
            replayed: outcome.replayed,
        }),
    ))
}

#[tracing::instrument(fields(service.name = crate::SERVICE_NAME, proph.user_id = %user_id), skip(state, request))]
async fn start_upgrade<S>(
    State(state): State<ApiState<S>>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
    Json(request): Json<StartUpgradeRequest>,
) -> Result<(StatusCode, Json<StartUpgradeResponse>), (StatusCode, Json<ApiErrorResponse>)>
where
    S: KycStore,
{
    authorize_user_subject(&headers, &user_id)?;
    validate_start_upgrade(&request)?;
    let outcome = state
        .store
        .start_upgrade(StartUpgradeDraft {
            user_id,
            target_tier: request.target_tier,
            idempotency_key: request.idempotency_key,
            declared_monthly_income: request.declared_monthly_income,
        })
        .await
        .map_err(store_error)?;

    Ok((
        if outcome.replayed {
            StatusCode::OK
        } else {
            StatusCode::CREATED
        },
        Json(StartUpgradeResponse {
            user_id: outcome.record.user_id,
            target_tier: outcome.record.tier,
            status: outcome.record.status,
            provider: outcome.record.provider,
            provider_applicant_id: outcome.record.provider_applicant_id,
            sdk_token: outcome.sdk_token,
            expires_at: outcome.record.expires_at,
            replayed: outcome.replayed,
        }),
    ))
}

#[tracing::instrument(fields(service.name = crate::SERVICE_NAME), skip(state, headers, body))]
async fn provider_webhook<S>(
    State(state): State<ApiState<S>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<ProviderWebhookResponse>, (StatusCode, Json<ApiErrorResponse>)>
where
    S: KycStore,
{
    verify_provider_hmac(&state.webhook_secret, &headers, &body)?;
    let payload = serde_json::from_slice::<ProviderWebhookPayload>(&body).map_err(|error| {
        api_error(
            StatusCode::BAD_REQUEST,
            "invalid_webhook_payload",
            error.to_string(),
        )
    })?;
    let review_answer = parse_review_answer(&payload.review_result.review_answer)?;
    let raw_payload_sha256 = hex::encode(Sha256::digest(&body));
    let outcome = state
        .store
        .apply_provider_webhook(ProviderWebhookDraft {
            applicant_id: payload.applicant_id,
            external_user_id: payload.external_user_id,
            review_answer,
            kyc_reason: payload.kyc_reason,
            sanction_matches: payload
                .sanction_matches
                .into_iter()
                .map(|value| value.to_string())
                .collect(),
            raw_payload_sha256,
        })
        .await
        .map_err(store_error)?
        .ok_or_else(|| {
            api_error(
                StatusCode::NOT_FOUND,
                "kyc_record_not_found",
                "webhook applicant/user pair does not match a pending KYC record",
            )
        })?;

    Ok(Json(ProviderWebhookResponse {
        user_id: outcome.record.user_id,
        current_tier: outcome.record.tier,
        status: outcome.record.status,
        provider_applicant_id: outcome.record.provider_applicant_id,
        replayed: outcome.replayed,
    }))
}

#[tracing::instrument(fields(service.name = crate::SERVICE_NAME), skip(state, request))]
async fn record_aml_event<S>(
    State(state): State<ApiState<S>>,
    Json(request): Json<AmlEventRequest>,
) -> Result<(StatusCode, Json<AmlEventResponse>), (StatusCode, Json<ApiErrorResponse>)>
where
    S: KycStore,
{
    validate_aml_event(&request)?;
    let outcome = state
        .store
        .record_aml_event(AmlEventDraft {
            user_id: request.user_id,
            event_type: request.event_type,
            amount_minor_units: request.amount_minor_units,
            ml_score: request.ml_score,
            rule_flags: request.rule_flags,
        })
        .await
        .map_err(store_error)?;

    Ok((
        if outcome.requires_compliance_review {
            StatusCode::CREATED
        } else {
            StatusCode::OK
        },
        Json(AmlEventResponse {
            user_id: outcome.user_id,
            requires_compliance_review: outcome.requires_compliance_review,
            sar_candidate_id: outcome.sar_candidate_id,
            reason: outcome.reason,
        }),
    ))
}

async fn monthly_report<S>(
    State(state): State<ApiState<S>>,
    Query(query): Query<MonthlyReportQuery>,
) -> Result<Json<MonthlyKycReportResponse>, (StatusCode, Json<ApiErrorResponse>)>
where
    S: KycStore,
{
    let report = state
        .store
        .monthly_report(query.month.unwrap_or_else(|| "current".to_owned()))
        .await
        .map_err(store_error)?;

    Ok(Json(MonthlyKycReportResponse {
        month: report.month,
        total_records: report.total_records,
        by_tier: report.by_tier,
        by_status: report.by_status,
        open_sar_candidates: report.open_sar_candidates,
    }))
}

async fn screening_rescan<S>(
    State(state): State<ApiState<S>>,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
    body: Bytes,
) -> Result<Json<ScreeningRescanResponse>, (StatusCode, Json<ApiErrorResponse>)>
where
    S: KycStore,
{
    verify_internal_request(
        &headers,
        "POST",
        uri.path(),
        &body,
        &state.internal_call_secret,
    )?;
    let request = serde_json::from_slice::<ScreeningRescanRequest>(&body).map_err(|error| {
        validation_error(format!("invalid screening rescan request body: {error}"))
    })?;
    let matches = request
        .matches
        .into_iter()
        .map(screening_match_draft)
        .collect::<Result<Vec<_>, _>>()?;
    let outcome = state
        .store
        .apply_screening_rescan(matches)
        .await
        .map_err(store_error)?;

    Ok(Json(ScreeningRescanResponse {
        scanned_matches: outcome.scanned_matches,
        frozen_users: outcome.frozen_users,
        edd_users: outcome.edd_users,
    }))
}

fn validate_tier_one(
    request: &TierOneVerificationRequest,
) -> Result<(), (StatusCode, Json<ApiErrorResponse>)> {
    if request.idempotency_key.trim().is_empty() {
        return Err(validation_error("idempotency_key is required"));
    }
    if request.legal_name.trim().is_empty() {
        return Err(validation_error("legal_name is required"));
    }
    if request.birthday.trim().is_empty() {
        return Err(validation_error("birthday is required"));
    }
    if !request.consent {
        return Err(validation_error(
            "explicit KYC privacy consent is required before collecting document data",
        ));
    }
    if !is_supported_tier_one_document(&request.country, &request.document_id) {
        return Err(validation_error(
            "country/document_id is not supported for Tier 1 verification",
        ));
    }
    Ok(())
}

fn screening_match_draft(
    request: ScreeningMatchRequest,
) -> Result<ScreeningMatchDraft, (StatusCode, Json<ApiErrorResponse>)> {
    if request.user_id.trim().is_empty() {
        return Err(validation_error("user_id is required"));
    }
    let match_type = match request.match_type.as_str() {
        "sanction" => ScreeningMatchType::Sanction,
        "pep" => ScreeningMatchType::Pep,
        _ => return Err(validation_error("match_type must be sanction or pep")),
    };
    Ok(ScreeningMatchDraft {
        user_id: request.user_id,
        match_type,
        list_name: request.list_name,
    })
}

fn validate_aml_event(
    request: &AmlEventRequest,
) -> Result<(), (StatusCode, Json<ApiErrorResponse>)> {
    if request.user_id.trim().is_empty() {
        return Err(validation_error("user_id is required"));
    }
    if request.event_type.trim().is_empty() {
        return Err(validation_error("event_type is required"));
    }
    if request.amount_minor_units < 0 {
        return Err(validation_error("amount_minor_units must be non-negative"));
    }
    if !(0..=100).contains(&request.ml_score) {
        return Err(validation_error("ml_score must be between 0 and 100"));
    }
    Ok(())
}

fn verify_provider_hmac(
    secret: &str,
    headers: &HeaderMap,
    body: &[u8],
) -> Result<(), (StatusCode, Json<ApiErrorResponse>)> {
    let Some(raw_digest) = headers
        .get("x-payload-digest")
        .and_then(|value| value.to_str().ok())
    else {
        return Err(unauthorized_webhook());
    };
    let digest = raw_digest.strip_prefix("sha1=").unwrap_or(raw_digest);
    let Ok(expected_bytes) = hex::decode(digest) else {
        return Err(unauthorized_webhook());
    };

    let Ok(mut mac) = Hmac::<Sha1>::new_from_slice(secret.as_bytes()) else {
        return Err(unauthorized_webhook());
    };
    mac.update(body);
    mac.verify_slice(&expected_bytes)
        .map_err(|_| unauthorized_webhook())
}

fn authorize_user_subject(
    headers: &HeaderMap,
    path_user_id: &str,
) -> Result<(), (StatusCode, Json<ApiErrorResponse>)> {
    let Some(subject) = headers
        .get("x-user-id")
        .and_then(|value| value.to_str().ok())
    else {
        return Err(api_error(
            StatusCode::UNAUTHORIZED,
            "subject_missing",
            "x-user-id auth context is required",
        ));
    };
    if subject != path_user_id {
        return Err(api_error(
            StatusCode::FORBIDDEN,
            "subject_mismatch",
            "x-user-id auth context must match path user_id",
        ));
    }
    Ok(())
}

fn parse_review_answer(raw: &str) -> Result<ReviewAnswer, (StatusCode, Json<ApiErrorResponse>)> {
    match raw {
        "GREEN" => Ok(ReviewAnswer::Green),
        "RED" => Ok(ReviewAnswer::Red),
        "YELLOW" => Ok(ReviewAnswer::Yellow),
        _ => Err(api_error(
            StatusCode::BAD_REQUEST,
            "invalid_review_answer",
            "reviewAnswer must be GREEN, RED, or YELLOW",
        )),
    }
}

fn unauthorized_webhook() -> (StatusCode, Json<ApiErrorResponse>) {
    api_error(
        StatusCode::UNAUTHORIZED,
        "webhook_hmac_invalid",
        "KYC provider webhook HMAC is missing or invalid",
    )
}

fn verify_internal_request(
    headers: &HeaderMap,
    method: &str,
    path: &str,
    body: &[u8],
    secret: &str,
) -> Result<(), (StatusCode, Json<ApiErrorResponse>)> {
    verify_internal_call(
        headers,
        method,
        path,
        body,
        secret,
        current_unix_timestamp(),
    )
    .map_err(|InternalCallRejection::Unauthorized| {
        (
            StatusCode::UNAUTHORIZED,
            Json(ApiErrorResponse {
                error: "unauthorized",
                message: "Internal call signature missing or invalid".to_owned(),
            }),
        )
    })
}

fn validate_start_upgrade(
    request: &StartUpgradeRequest,
) -> Result<(), (StatusCode, Json<ApiErrorResponse>)> {
    if request.idempotency_key.trim().is_empty() {
        return Err(validation_error("idempotency_key is required"));
    }
    if request.target_tier == KycTier::Tier0 {
        return Err(validation_error(
            "target_tier must be TIER_1, TIER_2, or TIER_3",
        ));
    }
    if request
        .declared_monthly_income
        .is_some_and(|income| income < 0)
    {
        return Err(validation_error(
            "declared_monthly_income must be null or non-negative",
        ));
    }
    Ok(())
}

fn status_response(
    user_id: &str,
    record: Option<&KycRecord>,
    tier_one_record: Option<&KycRecord>,
) -> KycStatusResponse {
    let tier_one_context = tier_one_record.or_else(|| {
        record.and_then(|record| {
            if record.tier == KycTier::Tier1 {
                Some(record)
            } else {
                None
            }
        })
    });

    match record {
        Some(record) => KycStatusResponse {
            user_id: user_id.to_owned(),
            current_tier: record.tier,
            status: record.status,
            provider: Some(record.provider.clone()),
            provider_applicant_id: record.provider_applicant_id.clone(),
            country: tier_one_context
                .and_then(|record| record.country.clone())
                .or_else(|| record.country.clone()),
            tier1_record_id: tier_one_context.map(|record| record.id.to_string()),
            declared_monthly_income: record.declared_monthly_income,
            submitted_at: Some(record.submitted_at.clone()),
            reviewed_at: record.reviewed_at.clone(),
            expires_at: record.expires_at.clone(),
        },
        None => KycStatusResponse {
            user_id: user_id.to_owned(),
            current_tier: KycTier::Tier0,
            status: KycStatus::Pending,
            provider: None,
            provider_applicant_id: None,
            country: None,
            tier1_record_id: None,
            declared_monthly_income: None,
            submitted_at: None,
            reviewed_at: None,
            expires_at: None,
        },
    }
}

fn store_error(error: KycStoreError) -> (StatusCode, Json<ApiErrorResponse>) {
    match error {
        KycStoreError::Unavailable => api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "kyc_store_unavailable",
            error.to_string(),
        ),
        KycStoreError::DuplicateDocument => api_error(
            StatusCode::CONFLICT,
            "duplicate_document",
            error.to_string(),
        ),
    }
}

fn validation_error(message: impl Into<String>) -> (StatusCode, Json<ApiErrorResponse>) {
    api_error(
        StatusCode::UNPROCESSABLE_ENTITY,
        "validation_failed",
        message,
    )
}

fn current_unix_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_secs()).unwrap_or(i64::MAX)
        })
}

fn api_error(
    status: StatusCode,
    error: &'static str,
    message: impl Into<String>,
) -> (StatusCode, Json<ApiErrorResponse>) {
    (
        status,
        Json(ApiErrorResponse {
            error,
            message: message.into(),
        }),
    )
}

/// Fixed 404 body used for both "no applicant" and "doc not bound" to prevent
/// leaking whether a document exists in the KYC provider.
static DOC_NOT_FOUND_BODY: &[u8] =
    br#"{"error":"document_not_found","message":"document not found"}"#;

fn extract_actor_headers(headers: &HeaderMap) -> (String, String, Option<String>) {
    let reviewer_id = headers
        .get("x-actor-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_owned();
    let reviewer_role = headers
        .get("x-operator-role")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_owned();
    let request_id = headers
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    (reviewer_id, reviewer_role, request_id)
}

/// KYC document access requires the KYC review permission. SPEC-005 §5.3.2 / §8
/// maps that permission to the `admin` and `manager` roles in the MVP.
fn is_kyc_reviewer_role(role: &str) -> bool {
    role == "admin" || role == "manager"
}

async fn list_member_documents<S>(
    State(state): State<ApiState<S>>,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
    Path(user_id): Path<String>,
) -> Result<Response, (StatusCode, Json<ApiErrorResponse>)>
where
    S: KycStore,
{
    verify_internal_request(
        &headers,
        "GET",
        uri.path(),
        b"",
        &state.internal_call_secret,
    )?;
    let (reviewer_id, reviewer_role, request_id) = extract_actor_headers(&headers);

    if !is_kyc_reviewer_role(&reviewer_role) {
        let entry = DocumentAccessEntry {
            reviewer_id,
            reviewer_role,
            user_id,
            doc_id: None,
            action: "list".to_owned(),
            result: "denied".to_owned(),
            denial_reason: Some("role_not_permitted".to_owned()),
            request_id,
            source_ip_hash: None,
        };
        if let Err(err) = state.store.record_document_access(entry).await {
            tracing::error!(?err, "failed to write kyc_document_access_log entry");
        }
        return Err(api_error(
            StatusCode::FORBIDDEN,
            "forbidden",
            "kyc review permission required",
        ));
    }

    let applicant_id = state
        .store
        .applicant_id_for_user(&user_id)
        .await
        .map_err(store_error)?;

    let documents = match applicant_id {
        None => {
            let entry = DocumentAccessEntry {
                reviewer_id,
                reviewer_role,
                user_id,
                doc_id: None,
                action: "list".to_owned(),
                result: "success".to_owned(),
                denial_reason: None,
                request_id,
                source_ip_hash: None,
            };
            if let Err(err) = state.store.record_document_access(entry).await {
                tracing::error!(?err, "failed to write kyc_document_access_log entry");
            }
            return Ok(axum::response::IntoResponse::into_response(Json(Vec::<
                crate::provider_documents::ProviderDocument,
            >::new(
            ))));
        }
        Some(applicant) => state
            .provider_documents
            .list_documents(&applicant)
            .await
            .map_err(|e| match e {
                ProviderDocumentError::ProviderAuth => {
                    tracing::error!("KYC provider auth failure listing documents");
                    api_error(
                        StatusCode::BAD_GATEWAY,
                        "provider_error",
                        "upstream provider error",
                    )
                }
                ProviderDocumentError::NotFound => api_error(
                    StatusCode::NOT_FOUND,
                    "document_not_found",
                    "applicant not found",
                ),
                ProviderDocumentError::Upstream(msg) => {
                    tracing::error!(%msg, "provider upstream error listing documents");
                    api_error(
                        StatusCode::BAD_GATEWAY,
                        "provider_error",
                        "upstream provider error",
                    )
                }
            })?,
    };

    let entry = DocumentAccessEntry {
        reviewer_id,
        reviewer_role,
        user_id,
        doc_id: None,
        action: "list".to_owned(),
        result: "success".to_owned(),
        denial_reason: None,
        request_id,
        source_ip_hash: None,
    };
    if let Err(err) = state.store.record_document_access(entry).await {
        tracing::error!(?err, "failed to write kyc_document_access_log entry");
    }

    Ok(axum::response::IntoResponse::into_response(Json(documents)))
}

async fn get_member_document<S>(
    State(state): State<ApiState<S>>,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
    Path((user_id, doc_id)): Path<(String, String)>,
) -> Result<Response, (StatusCode, Json<ApiErrorResponse>)>
where
    S: KycStore,
{
    verify_internal_request(
        &headers,
        "GET",
        uri.path(),
        b"",
        &state.internal_call_secret,
    )?;
    let (reviewer_id, reviewer_role, request_id) = extract_actor_headers(&headers);

    if !is_kyc_reviewer_role(&reviewer_role) {
        let entry = denied_entry(
            &reviewer_id,
            &reviewer_role,
            &user_id,
            Some(&doc_id),
            "role_not_permitted",
            request_id.as_deref(),
        );
        if let Err(err) = state.store.record_document_access(entry).await {
            tracing::error!(?err, "failed to write kyc_document_access_log entry");
        }
        return Err(api_error(
            StatusCode::FORBIDDEN,
            "forbidden",
            "kyc review permission required",
        ));
    }

    // Step 1: resolve applicant_id — None → 404
    let applicant_id = state
        .store
        .applicant_id_for_user(&user_id)
        .await
        .map_err(store_error)?;

    let Some(applicant) = applicant_id else {
        let entry = denied_entry(
            &reviewer_id,
            &reviewer_role,
            &user_id,
            Some(&doc_id),
            "no_applicant",
            request_id.as_deref(),
        );
        if let Err(err) = state.store.record_document_access(entry).await {
            tracing::error!(?err, "failed to write kyc_document_access_log entry");
        }
        return Ok(doc_not_found_response());
    };

    // Step 2: verify doc_id belongs to this applicant
    let documents = state
        .provider_documents
        .list_documents(&applicant)
        .await
        .map_err(provider_list_error)?;

    if !documents.iter().any(|doc| doc.doc_id == doc_id) {
        let entry = denied_entry(
            &reviewer_id,
            &reviewer_role,
            &user_id,
            Some(&doc_id),
            "doc_not_bound",
            request_id.as_deref(),
        );
        if let Err(err) = state.store.record_document_access(entry).await {
            tracing::error!(?err, "failed to write kyc_document_access_log entry");
        }
        return Ok(doc_not_found_response());
    }

    // Step 3: stream the image
    let image = state
        .provider_documents
        .fetch_image(&applicant, &doc_id)
        .await
        .map_err(provider_image_error)?;

    let safe_content_type = safe_image_content_type(&image.content_type);

    let entry = DocumentAccessEntry {
        reviewer_id,
        reviewer_role,
        user_id,
        doc_id: Some(doc_id),
        action: "view".to_owned(),
        result: "success".to_owned(),
        denial_reason: None,
        request_id,
        source_ip_hash: None,
    };
    if let Err(err) = state.store.record_document_access(entry).await {
        tracing::error!(?err, "failed to write kyc_document_access_log entry");
    }

    let body = Body::from_stream(image.response.bytes_stream());
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, safe_content_type)
        .header(header::CACHE_CONTROL, "private, no-store")
        .body(body)
        .unwrap_or_else(|_| Response::new(Body::empty()));
    Ok(response)
}

fn denied_entry(
    reviewer_id: &str,
    reviewer_role: &str,
    user_id: &str,
    doc_id: Option<&str>,
    denial_reason: &str,
    request_id: Option<&str>,
) -> DocumentAccessEntry {
    DocumentAccessEntry {
        reviewer_id: reviewer_id.to_owned(),
        reviewer_role: reviewer_role.to_owned(),
        user_id: user_id.to_owned(),
        doc_id: doc_id.map(str::to_owned),
        action: "view".to_owned(),
        result: "denied".to_owned(),
        denial_reason: Some(denial_reason.to_owned()),
        request_id: request_id.map(str::to_owned),
        source_ip_hash: None,
    }
}

fn provider_list_error(e: ProviderDocumentError) -> (StatusCode, Json<ApiErrorResponse>) {
    match e {
        ProviderDocumentError::ProviderAuth => {
            tracing::error!("KYC provider auth failure fetching document list for binding check");
            api_error(
                StatusCode::BAD_GATEWAY,
                "provider_error",
                "upstream provider error",
            )
        }
        ProviderDocumentError::NotFound => api_error(
            StatusCode::NOT_FOUND,
            "document_not_found",
            "applicant not found",
        ),
        ProviderDocumentError::Upstream(msg) => {
            tracing::error!(%msg, "provider upstream error fetching document list for binding check");
            api_error(
                StatusCode::BAD_GATEWAY,
                "provider_error",
                "upstream provider error",
            )
        }
    }
}

fn provider_image_error(e: ProviderDocumentError) -> (StatusCode, Json<ApiErrorResponse>) {
    match e {
        ProviderDocumentError::ProviderAuth => {
            tracing::error!("KYC provider auth failure fetching image");
            api_error(
                StatusCode::BAD_GATEWAY,
                "provider_error",
                "upstream provider error",
            )
        }
        ProviderDocumentError::NotFound => api_error(
            StatusCode::NOT_FOUND,
            "document_not_found",
            "image not found",
        ),
        ProviderDocumentError::Upstream(msg) => {
            tracing::error!(%msg, "provider upstream error fetching image");
            api_error(
                StatusCode::BAD_GATEWAY,
                "provider_error",
                "upstream provider error",
            )
        }
    }
}

fn doc_not_found_response() -> Response {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(DOC_NOT_FOUND_BODY))
        .unwrap_or_else(|_| Response::new(Body::empty()))
}

fn safe_image_content_type(content_type: &str) -> &'static str {
    match content_type {
        ct if ct.starts_with("image/jpeg") => "image/jpeg",
        ct if ct.starts_with("image/png") => "image/png",
        ct if ct.starts_with("image/webp") => "image/webp",
        _ => "application/octet-stream",
    }
}
