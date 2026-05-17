use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use argon2::{Algorithm, Argon2, Params, PasswordHasher, Version, password_hash::SaltString};
use async_trait::async_trait;
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get, patch, post},
};
use sdk::masking::mask_email;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{PgPool, Row};
use ulid::Ulid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberContactSummary {
    pub email_masked: String,
    pub phone_masked: String,
    pub line_id_masked: Option<String>,
    pub country: String,
    pub timezone: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberIdentitySummary {
    pub user_id: String,
    pub display_name: String,
    pub status: String,
    pub tier: String,
    pub registered_at: String,
    pub contact: MemberContactSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberAccountSummary {
    pub available_minor_units: i64,
    pub locked_minor_units: i64,
    pub frozen_minor_units: i64,
    pub currency: String,
    pub last_login_at: Option<String>,
    pub device_count: i32,
    pub two_factor_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberKycRiskSummary {
    pub kyc_status: String,
    pub risk_level: String,
    pub risk_score: i32,
    pub last_reviewed_at: Option<String>,
    pub restrictions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberOpenCaseSummary {
    pub case_id: String,
    pub case_type: String,
    pub status: String,
    pub owner_role: String,
    pub age: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberAuditSummary {
    pub correlation_id: String,
    pub source: String,
    pub audit_required: bool,
    pub latest_admin_action_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registration_audit: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberSourceSummary {
    pub channel: String,
    pub affiliate_id: Option<String>,
    pub affiliate_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberManagementSummary {
    pub member: MemberIdentitySummary,
    pub account: MemberAccountSummary,
    pub kyc_risk: MemberKycRiskSummary,
    pub open_cases: Vec<MemberOpenCaseSummary>,
    pub audit: MemberAuditSummary,
    pub source: MemberSourceSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ListMembersQuery {
    pub q: Option<String>,
    pub affiliate_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListMembersResponse {
    pub members: Vec<MemberManagementSummary>,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CreateMemberRequest {
    pub user_id: String,
    pub display_name: String,
    pub email: String,
    pub phone: String,
    pub line_id: Option<String>,
    pub country: String,
    pub timezone: String,
    pub tier: String,
    pub affiliate_id: Option<String>,
    pub registration_audit: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PublicRegistrationRequest {
    pub email: String,
    pub phone: String,
    pub password: String,
    pub display_name: String,
    pub referral_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicRegistrationResponse {
    pub user_id: String,
    pub member: MemberManagementSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct UpdateMemberRequest {
    pub status: Option<String>,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub line_id: Option<Option<String>>,
    pub country: Option<String>,
    pub timezone: Option<String>,
    pub tier: Option<String>,
    pub affiliate_id: Option<Option<String>>,
    pub kyc_status: Option<String>,
    pub risk_level: Option<String>,
    pub risk_score: Option<i32>,
    pub restrictions: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeleteMemberResponse {
    pub user_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LookupMaskedContactsRequest {
    pub ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LookupMaskedContactsResponse {
    pub contacts: Vec<MaskedContact>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaskedContact {
    pub id: String,
    pub email_masked: String,
    pub phone_masked: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberRecord {
    pub user_id: String,
    pub display_name: String,
    pub status: String,
    pub tier: String,
    pub email: String,
    pub phone: String,
    pub line_id: Option<String>,
    pub country: String,
    pub timezone: String,
    pub affiliate_id: Option<String>,
    pub kyc_status: String,
    pub risk_level: String,
    pub risk_score: i32,
    pub restrictions: Vec<String>,
    pub last_reviewed_at: Option<String>,
    pub last_login_at: Option<String>,
    pub device_count: i32,
    pub two_factor_enabled: bool,
    pub created_at: String,
    pub registration_audit: Option<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum MemberStoreError {
    #[error("member store is unavailable")]
    Unavailable,
    #[error("member already exists")]
    Duplicate,
    #[error("member not found")]
    NotFound,
    #[error("invalid member input")]
    Invalid,
}

#[async_trait]
pub trait MemberStore: Clone + Send + Sync + 'static {
    async fn list_members(
        &self,
        query: ListMembersQuery,
    ) -> Result<ListMembersResponse, MemberStoreError>;

    async fn member_summary(
        &self,
        user_id: &str,
    ) -> Result<MemberManagementSummary, MemberStoreError>;

    async fn create_member(
        &self,
        request: CreateMemberRequest,
    ) -> Result<MemberManagementSummary, MemberStoreError>;

    async fn update_member(
        &self,
        user_id: &str,
        request: UpdateMemberRequest,
    ) -> Result<MemberManagementSummary, MemberStoreError>;

    async fn delete_member(&self, user_id: &str) -> Result<DeleteMemberResponse, MemberStoreError>;

    async fn lookup_masked_contacts(
        &self,
        ids: Vec<String>,
    ) -> Result<Vec<MaskedContact>, MemberStoreError>;

    async fn provision_login_credential(
        &self,
        _user_id: &str,
        _email: &str,
        _phone: &str,
        _password_hash: &str,
    ) -> Result<(), MemberStoreError> {
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryMemberStore {
    records: Arc<Mutex<HashMap<String, MemberRecord>>>,
}

impl InMemoryMemberStore {
    #[must_use]
    pub fn new(records: Vec<MemberRecord>) -> Self {
        Self {
            records: Arc::new(Mutex::new(
                records
                    .into_iter()
                    .map(|record| (record.user_id.clone(), record))
                    .collect(),
            )),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PostgresMemberStore {
    pool: PgPool,
}

impl PostgresMemberStore {
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

pub fn internal_members_router<S>(store: S) -> Router
where
    S: MemberStore,
{
    Router::new()
        .route("/v1/internal/admin/members", get(list_members::<S>))
        .route("/v1/internal/admin/members", post(create_member::<S>))
        .route(
            "/v1/public/registrations",
            post(register_public_member::<S>),
        )
        .route(
            "/v1/internal/admin/members/:user_id/summary",
            get(member_summary::<S>),
        )
        .route(
            "/v1/internal/admin/members/:user_id",
            patch(update_member::<S>),
        )
        .route(
            "/v1/internal/admin/members/:user_id",
            delete(delete_member::<S>),
        )
        .route(
            "/v1/internal/users/contacts:lookup-masked",
            post(lookup_masked_contacts::<S>),
        )
        .with_state(store)
}

async fn list_members<S>(
    State(store): State<S>,
    Query(query): Query<ListMembersQuery>,
) -> Result<Json<ListMembersResponse>, (StatusCode, Json<ErrorResponse>)>
where
    S: MemberStore,
{
    store
        .list_members(query)
        .await
        .map(Json)
        .map_err(store_error)
}

async fn member_summary<S>(
    State(store): State<S>,
    Path(user_id): Path<String>,
) -> Result<Json<MemberManagementSummary>, (StatusCode, Json<ErrorResponse>)>
where
    S: MemberStore,
{
    store
        .member_summary(&user_id)
        .await
        .map(Json)
        .map_err(store_error)
}

async fn create_member<S>(
    State(store): State<S>,
    Json(request): Json<CreateMemberRequest>,
) -> Result<(StatusCode, Json<MemberManagementSummary>), (StatusCode, Json<ErrorResponse>)>
where
    S: MemberStore,
{
    store
        .create_member(request)
        .await
        .map(|summary| (StatusCode::CREATED, Json(summary)))
        .map_err(store_error)
}

async fn register_public_member<S>(
    State(store): State<S>,
    Json(request): Json<PublicRegistrationRequest>,
) -> Result<(StatusCode, Json<PublicRegistrationResponse>), (StatusCode, Json<ErrorResponse>)>
where
    S: MemberStore,
{
    let password_hash = hash_public_registration_password(&request.password)?;
    let create = public_registration_to_create_member(request)?;
    let summary = store
        .create_member(create.clone())
        .await
        .map_err(store_error)?;
    store
        .provision_login_credential(
            &create.user_id,
            &create.email,
            &create.phone,
            &password_hash,
        )
        .await
        .map_err(store_error)?;

    Ok((
        StatusCode::CREATED,
        Json(PublicRegistrationResponse {
            user_id: summary.member.user_id.clone(),
            member: summary,
        }),
    ))
}

async fn update_member<S>(
    State(store): State<S>,
    Path(user_id): Path<String>,
    Json(request): Json<UpdateMemberRequest>,
) -> Result<Json<MemberManagementSummary>, (StatusCode, Json<ErrorResponse>)>
where
    S: MemberStore,
{
    store
        .update_member(&user_id, request)
        .await
        .map(Json)
        .map_err(store_error)
}

async fn delete_member<S>(
    State(store): State<S>,
    Path(user_id): Path<String>,
) -> Result<Json<DeleteMemberResponse>, (StatusCode, Json<ErrorResponse>)>
where
    S: MemberStore,
{
    store
        .delete_member(&user_id)
        .await
        .map(Json)
        .map_err(store_error)
}

async fn lookup_masked_contacts<S>(
    State(store): State<S>,
    Json(request): Json<LookupMaskedContactsRequest>,
) -> Result<Json<LookupMaskedContactsResponse>, (StatusCode, Json<ErrorResponse>)>
where
    S: MemberStore,
{
    if request.ids.iter().any(|id| id.trim().is_empty()) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "ERROR_CODE_EMPTY_CONTACT_ID",
            }),
        ));
    }

    store
        .lookup_masked_contacts(request.ids)
        .await
        .map(|contacts| Json(LookupMaskedContactsResponse { contacts }))
        .map_err(store_error)
}

#[async_trait]
impl MemberStore for InMemoryMemberStore {
    async fn list_members(
        &self,
        query: ListMembersQuery,
    ) -> Result<ListMembersResponse, MemberStoreError> {
        let records = self
            .records
            .lock()
            .map_err(|_| MemberStoreError::Unavailable)?;
        Ok(list_response(records.values().cloned(), &query))
    }

    async fn member_summary(
        &self,
        user_id: &str,
    ) -> Result<MemberManagementSummary, MemberStoreError> {
        let records = self
            .records
            .lock()
            .map_err(|_| MemberStoreError::Unavailable)?;
        records
            .get(user_id)
            .cloned()
            .map(summary_from_record)
            .ok_or(MemberStoreError::NotFound)
    }

    async fn create_member(
        &self,
        request: CreateMemberRequest,
    ) -> Result<MemberManagementSummary, MemberStoreError> {
        validate_create_member(&request)?;
        let mut records = self
            .records
            .lock()
            .map_err(|_| MemberStoreError::Unavailable)?;
        if records.contains_key(&request.user_id) {
            return Err(MemberStoreError::Duplicate);
        }
        let record = record_from_create(&request);
        let summary = summary_from_record(record.clone());
        records.insert(record.user_id.clone(), record);
        Ok(summary)
    }

    async fn update_member(
        &self,
        user_id: &str,
        request: UpdateMemberRequest,
    ) -> Result<MemberManagementSummary, MemberStoreError> {
        let mut records = self
            .records
            .lock()
            .map_err(|_| MemberStoreError::Unavailable)?;
        let record = records.get_mut(user_id).ok_or(MemberStoreError::NotFound)?;
        apply_update(record, request)?;
        Ok(summary_from_record(record.clone()))
    }

    async fn delete_member(&self, user_id: &str) -> Result<DeleteMemberResponse, MemberStoreError> {
        let mut records = self
            .records
            .lock()
            .map_err(|_| MemberStoreError::Unavailable)?;
        records.remove(user_id).ok_or(MemberStoreError::NotFound)?;
        Ok(DeleteMemberResponse {
            user_id: user_id.to_owned(),
        })
    }

    async fn lookup_masked_contacts(
        &self,
        ids: Vec<String>,
    ) -> Result<Vec<MaskedContact>, MemberStoreError> {
        let records = self
            .records
            .lock()
            .map_err(|_| MemberStoreError::Unavailable)?;
        Ok(ids
            .into_iter()
            .filter_map(|id| records.get(&id).map(masked_contact_from_record))
            .collect())
    }
}

#[async_trait]
impl MemberStore for PostgresMemberStore {
    async fn list_members(
        &self,
        query: ListMembersQuery,
    ) -> Result<ListMembersResponse, MemberStoreError> {
        let rows = sqlx::query(
            r"
            SELECT user_id, display_name, status, tier, email, phone, line_id, country, timezone,
                   affiliate_id, registration_audit, kyc_status, risk_level, risk_score, restrictions,
                   last_reviewed_at::text AS last_reviewed_at,
                   last_login_at::text AS last_login_at,
                   device_count, two_factor_enabled, created_at::text AS created_at
            FROM user_members
            WHERE deleted_at IS NULL
            ORDER BY created_at DESC, user_id ASC
            ",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(sqlx_store_error)?;
        let records = rows
            .iter()
            .map(row_to_member_record)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(list_response(records, &query))
    }

    async fn member_summary(
        &self,
        user_id: &str,
    ) -> Result<MemberManagementSummary, MemberStoreError> {
        let record = fetch_member_record(&self.pool, user_id).await?;
        Ok(summary_from_record(record))
    }

    async fn create_member(
        &self,
        request: CreateMemberRequest,
    ) -> Result<MemberManagementSummary, MemberStoreError> {
        validate_create_member(&request)?;
        let record = record_from_create(&request);
        let row = sqlx::query(
            r"
            INSERT INTO user_members (
                user_id, display_name, status, tier, email, phone, line_id, country, timezone,
                affiliate_id, registration_audit, kyc_status, risk_level, risk_score, restrictions
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
            RETURNING user_id, display_name, status, tier, email, phone, line_id, country, timezone,
                      affiliate_id, registration_audit, kyc_status, risk_level, risk_score, restrictions,
                      last_reviewed_at::text AS last_reviewed_at,
                      last_login_at::text AS last_login_at,
                      device_count, two_factor_enabled, created_at::text AS created_at
            ",
        )
        .bind(&record.user_id)
        .bind(&record.display_name)
        .bind(&record.status)
        .bind(&record.tier)
        .bind(&record.email)
        .bind(&record.phone)
        .bind(&record.line_id)
        .bind(&record.country)
        .bind(&record.timezone)
        .bind(&record.affiliate_id)
        .bind(&record.registration_audit)
        .bind(&record.kyc_status)
        .bind(&record.risk_level)
        .bind(record.risk_score)
        .bind(serde_json::json!(record.restrictions))
        .fetch_one(&self.pool)
        .await
        .map_err(sqlx_store_error)?;

        Ok(summary_from_record(row_to_member_record(&row)?))
    }

    async fn update_member(
        &self,
        user_id: &str,
        request: UpdateMemberRequest,
    ) -> Result<MemberManagementSummary, MemberStoreError> {
        let mut record = fetch_member_record(&self.pool, user_id).await?;
        apply_update(&mut record, request)?;
        let row = sqlx::query(
            r"
            UPDATE user_members
            SET display_name = $2,
                status = $3,
                tier = $4,
                email = $5,
                phone = $6,
                line_id = $7,
                country = $8,
                timezone = $9,
                affiliate_id = $10,
                kyc_status = $11,
                risk_level = $12,
                risk_score = $13,
                restrictions = $14,
                last_reviewed_at = CASE WHEN $15::BOOL THEN now() ELSE last_reviewed_at END,
                updated_at = now()
            WHERE user_id = $1 AND deleted_at IS NULL
            RETURNING user_id, display_name, status, tier, email, phone, line_id, country, timezone,
                      affiliate_id, registration_audit, kyc_status, risk_level, risk_score, restrictions,
                      last_reviewed_at::text AS last_reviewed_at,
                      last_login_at::text AS last_login_at,
                      device_count, two_factor_enabled, created_at::text AS created_at
            ",
        )
        .bind(user_id)
        .bind(&record.display_name)
        .bind(&record.status)
        .bind(&record.tier)
        .bind(&record.email)
        .bind(&record.phone)
        .bind(&record.line_id)
        .bind(&record.country)
        .bind(&record.timezone)
        .bind(&record.affiliate_id)
        .bind(&record.kyc_status)
        .bind(&record.risk_level)
        .bind(record.risk_score)
        .bind(serde_json::json!(record.restrictions))
        .bind(record.kyc_status == "verified")
        .fetch_one(&self.pool)
        .await
        .map_err(sqlx_store_error)?;

        Ok(summary_from_record(row_to_member_record(&row)?))
    }

    async fn delete_member(&self, user_id: &str) -> Result<DeleteMemberResponse, MemberStoreError> {
        let result = sqlx::query(
            "UPDATE user_members SET deleted_at = now(), updated_at = now() WHERE user_id = $1 AND deleted_at IS NULL",
        )
        .bind(user_id)
        .execute(&self.pool)
        .await
        .map_err(sqlx_store_error)?;
        if result.rows_affected() == 0 {
            return Err(MemberStoreError::NotFound);
        }
        Ok(DeleteMemberResponse {
            user_id: user_id.to_owned(),
        })
    }

    async fn lookup_masked_contacts(
        &self,
        ids: Vec<String>,
    ) -> Result<Vec<MaskedContact>, MemberStoreError> {
        let rows = sqlx::query(
            r"
            SELECT user_id, email, phone
            FROM user_members
            WHERE deleted_at IS NULL AND user_id = ANY($1)
            ",
        )
        .bind(&ids)
        .fetch_all(&self.pool)
        .await
        .map_err(sqlx_store_error)?;
        Ok(rows
            .iter()
            .map(|row| {
                let id: String = row.try_get("user_id").map_err(sqlx_store_error)?;
                let email: String = row.try_get("email").map_err(sqlx_store_error)?;
                let phone: String = row.try_get("phone").map_err(sqlx_store_error)?;
                Ok(MaskedContact {
                    id,
                    email_masked: mask_email(&email),
                    phone_masked: mask_phone(&phone),
                })
            })
            .collect::<Result<Vec<_>, MemberStoreError>>()?)
    }

    async fn provision_login_credential(
        &self,
        user_id: &str,
        email: &str,
        phone: &str,
        password_hash: &str,
    ) -> Result<(), MemberStoreError> {
        sqlx::query(
            r"
            INSERT INTO user_login_credentials (
                user_id,
                email,
                phone,
                password_hash,
                role,
                affiliate_id
            )
            VALUES ($1, $2, $3, $4, 'player', NULL)
            ON CONFLICT (user_id) DO UPDATE SET
                email = EXCLUDED.email,
                phone = EXCLUDED.phone,
                password_hash = EXCLUDED.password_hash,
                disabled_at = NULL
            ",
        )
        .bind(user_id)
        .bind(email)
        .bind(phone)
        .bind(password_hash)
        .execute(&self.pool)
        .await
        .map_err(sqlx_store_error)?;
        Ok(())
    }
}

fn validate_create_member(request: &CreateMemberRequest) -> Result<(), MemberStoreError> {
    if request.user_id.trim().is_empty()
        || request.display_name.trim().is_empty()
        || request.email.trim().is_empty()
        || request.phone.trim().is_empty()
        || request.country.trim().is_empty()
        || request.timezone.trim().is_empty()
        || request.tier.trim().is_empty()
    {
        return Err(MemberStoreError::Invalid);
    }
    Ok(())
}

fn public_registration_to_create_member(
    request: PublicRegistrationRequest,
) -> Result<CreateMemberRequest, (StatusCode, Json<ErrorResponse>)> {
    let email = request.email.trim().to_ascii_lowercase();
    let phone = normalize_taiwan_mobile(&request.phone).ok_or_else(invalid_registration)?;
    let display_name = request.display_name.trim().to_owned();
    let password = request.password.trim();
    if !is_valid_email(&email) || display_name.is_empty() || password.len() < 8 {
        return Err(invalid_registration());
    }
    let referral_code = request
        .referral_code
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let user_id = format!("MEM-{}", Ulid::new());

    Ok(CreateMemberRequest {
        user_id: user_id.clone(),
        display_name,
        email,
        phone,
        line_id: None,
        country: "TW".to_owned(),
        timezone: "Asia/Taipei".to_owned(),
        tier: "Tier 0".to_owned(),
        affiliate_id: None,
        registration_audit: Some(serde_json::json!({
            "source": "public-web-signup",
            "referral_capture_required": referral_code.is_some(),
            "referral_code": referral_code,
            "referred_user_id": user_id
        })),
    })
}

fn hash_public_registration_password(
    password: &str,
) -> Result<String, (StatusCode, Json<ErrorResponse>)> {
    let password = password.trim();
    if password.len() < 8 {
        return Err(invalid_registration());
    }
    let params = Params::new(19_456, 2, 1, None).map_err(|_| invalid_registration())?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let salt = SaltString::encode_b64(Ulid::new().to_string().as_bytes())
        .map_err(|_| invalid_registration())?;
    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| invalid_registration())
}

fn invalid_registration() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: "ERROR_CODE_INVALID_REGISTRATION",
        }),
    )
}

fn is_valid_email(value: &str) -> bool {
    let Some((local, domain)) = value.split_once('@') else {
        return false;
    };
    !local.is_empty() && domain.contains('.') && !domain.ends_with('.')
}

fn normalize_taiwan_mobile(value: &str) -> Option<String> {
    let compact = value
        .trim()
        .chars()
        .filter(|ch| !matches!(ch, ' ' | '-' | '(' | ')' | '.'))
        .collect::<String>();
    if !compact
        .strip_prefix('+')
        .unwrap_or(&compact)
        .chars()
        .all(|ch| ch.is_ascii_digit())
    {
        return None;
    }
    let digits = compact.strip_prefix('+').unwrap_or(&compact);
    let subscriber = digits
        .strip_prefix("886")
        .or_else(|| digits.strip_prefix('0'))
        .unwrap_or(digits);

    (subscriber.len() == 9 && subscriber.starts_with('9')).then(|| format!("+886{subscriber}"))
}

fn record_from_create(request: &CreateMemberRequest) -> MemberRecord {
    MemberRecord {
        user_id: request.user_id.trim().to_owned(),
        display_name: request.display_name.trim().to_owned(),
        status: "inactive".to_owned(),
        tier: request.tier.trim().to_owned(),
        email: request.email.trim().to_owned(),
        phone: request.phone.trim().to_owned(),
        line_id: request.line_id.as_deref().and_then(trim_optional),
        country: request.country.trim().to_ascii_uppercase(),
        timezone: request.timezone.trim().to_owned(),
        affiliate_id: request.affiliate_id.as_deref().and_then(trim_optional),
        kyc_status: "pending".to_owned(),
        risk_level: "medium".to_owned(),
        risk_score: 50,
        restrictions: vec!["kyc_pending".to_owned()],
        last_reviewed_at: None,
        last_login_at: None,
        device_count: 0,
        two_factor_enabled: false,
        created_at: "now".to_owned(),
        registration_audit: request.registration_audit.clone(),
    }
}

fn apply_update(
    record: &mut MemberRecord,
    request: UpdateMemberRequest,
) -> Result<(), MemberStoreError> {
    if let Some(value) = request.status {
        if !matches!(
            value.as_str(),
            "inactive" | "active" | "restricted" | "suspended"
        ) {
            return Err(MemberStoreError::Invalid);
        }
        record.status = value;
    }
    if let Some(value) = request.display_name.as_deref().and_then(trim_optional) {
        record.display_name = value;
    }
    if let Some(value) = request.email.as_deref().and_then(trim_optional) {
        record.email = value;
    }
    if let Some(value) = request.phone.as_deref().and_then(trim_optional) {
        record.phone = value;
    }
    if let Some(value) = request.line_id {
        record.line_id = value.as_deref().and_then(trim_optional);
    }
    if let Some(value) = request.country.as_deref().and_then(trim_optional) {
        record.country = value.to_ascii_uppercase();
    }
    if let Some(value) = request.timezone.as_deref().and_then(trim_optional) {
        record.timezone = value;
    }
    if let Some(value) = request.tier.as_deref().and_then(trim_optional) {
        record.tier = value;
    }
    if let Some(value) = request.affiliate_id {
        record.affiliate_id = value.as_deref().and_then(trim_optional);
    }
    if let Some(value) = request.kyc_status {
        if !matches!(value.as_str(), "pending" | "verified" | "review_required") {
            return Err(MemberStoreError::Invalid);
        }
        record.kyc_status = value;
        if record.kyc_status == "verified" {
            "active".clone_into(&mut record.status);
            record.restrictions.clear();
        } else if record.kyc_status == "pending" {
            "inactive".clone_into(&mut record.status);
            record.restrictions = vec!["kyc_pending".to_owned()];
        } else {
            "restricted".clone_into(&mut record.status);
            record.restrictions = vec!["manual_review_required".to_owned()];
        }
    }
    if let Some(value) = request.risk_level {
        if !matches!(value.as_str(), "low" | "medium" | "high") {
            return Err(MemberStoreError::Invalid);
        }
        record.risk_level = value;
    }
    if let Some(value) = request.risk_score {
        if !(0..=100).contains(&value) {
            return Err(MemberStoreError::Invalid);
        }
        record.risk_score = value;
    }
    if let Some(restrictions) = request.restrictions {
        let normalized = restrictions
            .into_iter()
            .map(|restriction| restriction.trim().to_owned())
            .filter(|restriction| !restriction.is_empty())
            .collect::<Vec<_>>();
        if normalized.iter().any(|restriction| {
            !restriction
                .chars()
                .all(|character| character.is_ascii_lowercase() || character == '_')
        }) {
            return Err(MemberStoreError::Invalid);
        }
        record.restrictions = normalized;
    }
    Ok(())
}

fn list_response(
    records: impl IntoIterator<Item = MemberRecord>,
    query: &ListMembersQuery,
) -> ListMembersResponse {
    let q = query.q.as_deref().map_or("", str::trim).to_lowercase();
    let affiliate_id = query.affiliate_id.as_deref().map(str::trim);
    let mut members = records
        .into_iter()
        .filter(|record| {
            affiliate_id.is_none_or(|scope| record.affiliate_id.as_deref() == Some(scope))
                && (q.is_empty()
                    || record.user_id.to_lowercase().contains(&q)
                    || record.display_name.to_lowercase().contains(&q)
                    || record.email.to_lowercase().contains(&q)
                    || record.phone.to_lowercase().contains(&q))
        })
        .map(summary_from_record)
        .collect::<Vec<_>>();
    members.sort_by(|left, right| left.member.user_id.cmp(&right.member.user_id));
    ListMembersResponse {
        members,
        source: "user-service:member-store".to_owned(),
    }
}

fn summary_from_record(record: MemberRecord) -> MemberManagementSummary {
    let affiliate_name = record
        .affiliate_id
        .as_deref()
        .map_or("Direct / 無代理".to_owned(), |affiliate_id| {
            format!("{affiliate_id} / 代理")
        });
    MemberManagementSummary {
        member: MemberIdentitySummary {
            user_id: record.user_id.clone(),
            display_name: record.display_name.clone(),
            status: record.status,
            tier: record.tier,
            registered_at: record.created_at,
            contact: MemberContactSummary {
                email_masked: mask_email(&record.email),
                phone_masked: mask_phone(&record.phone),
                line_id_masked: record.line_id.as_deref().map(mask_line_id),
                country: record.country,
                timezone: record.timezone,
            },
        },
        account: MemberAccountSummary {
            available_minor_units: 0,
            locked_minor_units: 0,
            frozen_minor_units: 0,
            currency: "POINTS".to_owned(),
            last_login_at: record.last_login_at,
            device_count: record.device_count,
            two_factor_enabled: record.two_factor_enabled,
        },
        kyc_risk: MemberKycRiskSummary {
            kyc_status: record.kyc_status,
            risk_level: record.risk_level,
            risk_score: record.risk_score,
            last_reviewed_at: record.last_reviewed_at,
            restrictions: record.restrictions,
        },
        open_cases: Vec::new(),
        audit: MemberAuditSummary {
            correlation_id: format!("user-service-member-{}", record.user_id),
            source: "user-service:member-store".to_owned(),
            audit_required: true,
            latest_admin_action_at: None,
            registration_audit: record.registration_audit,
        },
        source: MemberSourceSummary {
            channel: if record.affiliate_id.is_some() {
                "affiliate".to_owned()
            } else {
                "direct".to_owned()
            },
            affiliate_id: record.affiliate_id,
            affiliate_name,
        },
    }
}

fn row_to_member_record(row: &sqlx::postgres::PgRow) -> Result<MemberRecord, MemberStoreError> {
    let restrictions_value: Value = row.try_get("restrictions").map_err(sqlx_store_error)?;
    let restrictions = serde_json::from_value::<Vec<String>>(restrictions_value)
        .map_err(|_| MemberStoreError::Unavailable)?;
    Ok(MemberRecord {
        user_id: row.try_get("user_id").map_err(sqlx_store_error)?,
        display_name: row.try_get("display_name").map_err(sqlx_store_error)?,
        status: row.try_get("status").map_err(sqlx_store_error)?,
        tier: row.try_get("tier").map_err(sqlx_store_error)?,
        email: row.try_get("email").map_err(sqlx_store_error)?,
        phone: row.try_get("phone").map_err(sqlx_store_error)?,
        line_id: row.try_get("line_id").map_err(sqlx_store_error)?,
        country: row.try_get("country").map_err(sqlx_store_error)?,
        timezone: row.try_get("timezone").map_err(sqlx_store_error)?,
        affiliate_id: row.try_get("affiliate_id").map_err(sqlx_store_error)?,
        registration_audit: row
            .try_get("registration_audit")
            .map_err(sqlx_store_error)?,
        kyc_status: row.try_get("kyc_status").map_err(sqlx_store_error)?,
        risk_level: row.try_get("risk_level").map_err(sqlx_store_error)?,
        risk_score: row.try_get("risk_score").map_err(sqlx_store_error)?,
        restrictions,
        last_reviewed_at: row.try_get("last_reviewed_at").map_err(sqlx_store_error)?,
        last_login_at: row.try_get("last_login_at").map_err(sqlx_store_error)?,
        device_count: row.try_get("device_count").map_err(sqlx_store_error)?,
        two_factor_enabled: row
            .try_get("two_factor_enabled")
            .map_err(sqlx_store_error)?,
        created_at: row.try_get("created_at").map_err(sqlx_store_error)?,
    })
}

async fn fetch_member_record(
    pool: &PgPool,
    user_id: &str,
) -> Result<MemberRecord, MemberStoreError> {
    let row = sqlx::query(
        r"
        SELECT user_id, display_name, status, tier, email, phone, line_id, country, timezone,
               affiliate_id, registration_audit, kyc_status, risk_level, risk_score, restrictions,
               last_reviewed_at::text AS last_reviewed_at,
               last_login_at::text AS last_login_at,
               device_count, two_factor_enabled, created_at::text AS created_at
        FROM user_members
        WHERE user_id = $1 AND deleted_at IS NULL
        ",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(sqlx_store_error)?
    .ok_or(MemberStoreError::NotFound)?;
    row_to_member_record(&row)
}

fn masked_contact_from_record(record: &MemberRecord) -> MaskedContact {
    MaskedContact {
        id: record.user_id.clone(),
        email_masked: mask_email(&record.email),
        phone_masked: mask_phone(&record.phone),
    }
}

fn trim_optional(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

fn mask_phone(value: &str) -> String {
    let digits = value
        .chars()
        .filter(char::is_ascii_digit)
        .collect::<String>();
    if digits.len() <= 6 {
        return "***".to_owned();
    }
    format!("{}***{}", &digits[..4], &digits[digits.len() - 3..])
}

fn mask_line_id(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() <= 4 {
        "***".to_owned()
    } else {
        format!("{}***", &trimmed[..3])
    }
}

fn store_error(error: MemberStoreError) -> (StatusCode, Json<ErrorResponse>) {
    let (status, code) = match error {
        MemberStoreError::Duplicate => (StatusCode::CONFLICT, "ERROR_CODE_MEMBER_EXISTS"),
        MemberStoreError::NotFound => (StatusCode::NOT_FOUND, "ERROR_CODE_MEMBER_NOT_FOUND"),
        MemberStoreError::Invalid => (StatusCode::BAD_REQUEST, "ERROR_CODE_INVALID_MEMBER"),
        MemberStoreError::Unavailable => (StatusCode::INTERNAL_SERVER_ERROR, "ERROR_CODE_INTERNAL"),
    };
    (status, Json(ErrorResponse { error: code }))
}

fn sqlx_store_error(error: sqlx::Error) -> MemberStoreError {
    if let sqlx::Error::Database(database_error) = &error {
        if database_error
            .constraint()
            .is_some_and(|constraint| constraint == "user_members_pkey")
        {
            return MemberStoreError::Duplicate;
        }
    }
    drop(error);
    MemberStoreError::Unavailable
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: &'static str,
}
