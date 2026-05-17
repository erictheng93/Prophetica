use std::{
    collections::HashSet,
    env,
    fs::{self, OpenOptions},
    io::Write,
    net::SocketAddr,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use argon2::{Argon2, PasswordHash, PasswordVerifier};
use async_trait::async_trait;
use axum::{
    Json, Router,
    extract::{ConnectInfo, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha1::{Digest, Sha1};
use sqlx::{PgPool, Row};

use crate::{
    login_events::{
        AdminLoginEventFilter, InMemoryLoginEventStore, LoginEventInput, LoginEventStore,
        LoginEventType, TrustedProxyConfig, client_ip_from_request, geo_from_headers,
        identifier_hmac, normalize_login_surface, parse_user_agent, principal_type_from_role,
        principal_type_from_surface, to_self_response,
    },
    webauthn_runtime::{
        PasskeyAssertionFinish, WebauthnRuntimeConfig, finish_passkey_assertion,
        finish_passkey_registration, start_and_store_passkey_assertion,
        start_and_store_passkey_registration,
    },
    webauthn_store::{
        InMemoryWebauthnStore, WebauthnCeremony, WebauthnCredentialRecord, WebauthnStore,
    },
};

const LOCAL_ACCESS_TOKEN_TTL_SECONDS: u64 = 60 * 60 * 24;
const TOTP_STEP_SECONDS: u64 = 30;
const TOTP_DIGITS_MODULUS: u32 = 1_000_000;

#[derive(Clone)]
pub struct AuthConfig {
    accounts: Arc<[LocalLoginAccount]>,
    session_signer: Option<SessionSigner>,
    consumed_recovery_codes: Arc<Mutex<HashSet<String>>>,
    consumed_recovery_code_store_path: Option<Arc<PathBuf>>,
    webauthn_config: Option<WebauthnRuntimeConfig>,
    webauthn_store: Arc<dyn WebauthnStore>,
    credential_store: Option<Arc<dyn PasswordCredentialStore>>,
    login_event_store: Option<Arc<dyn LoginEventStore>>,
    login_identifier_hmac_key: Arc<Vec<u8>>,
    login_identifier_hmac_key_version: String,
    trusted_proxy_config: TrustedProxyConfig,
    login_event_metrics: Arc<LoginEventMetrics>,
}

#[derive(Clone)]
struct SessionSigner {
    kid: String,
    signing_key: SigningKey,
    public_x: String,
}

#[derive(Debug, Deserialize)]
struct LocalLoginAccountSpec {
    user_id: String,
    email: String,
    #[serde(default)]
    phone: Option<String>,
    password: String,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    affiliate_id: Option<String>,
    #[serde(default)]
    mfa_methods: Vec<String>,
    #[serde(default)]
    mfa_at_epoch_seconds: Option<u64>,
    #[serde(default)]
    totp_secret_base32: Option<String>,
    #[serde(default)]
    recovery_codes: Vec<String>,
    #[serde(default)]
    webauthn_user_uuid: Option<String>,
}

#[derive(Clone)]
pub struct LocalLoginAccount {
    user_id: String,
    email: String,
    phone: String,
    password: String,
    role: String,
    affiliate_id: Option<String>,
    mfa_methods: Vec<String>,
    mfa_at_epoch_seconds: Option<u64>,
    totp_secret_base32: Option<String>,
    recovery_codes: Vec<String>,
    webauthn_user_uuid: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredLoginCredential {
    pub user_id: String,
    pub email: String,
    pub phone: String,
    pub password_hash: String,
    pub role: String,
    pub affiliate_id: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum PasswordCredentialStoreError {
    #[error("password credential store unavailable")]
    Unavailable,
}

#[async_trait]
pub trait PasswordCredentialStore: Send + Sync {
    async fn credential_by_identifier(
        &self,
        identifier: &LoginIdentifier,
    ) -> Result<Option<StoredLoginCredential>, PasswordCredentialStoreError>;

    async fn credential_by_user_id(
        &self,
        user_id: &str,
    ) -> Result<Option<StoredLoginCredential>, PasswordCredentialStoreError>;
}

#[derive(Clone, Default)]
pub struct InMemoryPasswordCredentialStore {
    credentials: Arc<Vec<StoredLoginCredential>>,
}

#[derive(Clone)]
pub struct PostgresPasswordCredentialStore {
    pool: PgPool,
}

#[derive(Debug, Deserialize)]
struct LoginRequest {
    #[serde(default)]
    identifier: String,
    #[serde(default)]
    identifier_kind: String,
    #[serde(default)]
    password: String,
    #[serde(default)]
    remember: bool,
    #[serde(default)]
    mfa_code: Option<String>,
    #[serde(default)]
    mfa_recovery_code: Option<String>,
    #[serde(default)]
    login_surface: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LoginEventsQuery {
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    principal_type: Option<String>,
    #[serde(default)]
    principal_id: Option<String>,
    #[serde(default)]
    user_id: Option<String>,
    #[serde(default)]
    ip_address: Option<String>,
    #[serde(default)]
    event_type: Option<String>,
    #[serde(default)]
    from_epoch_seconds: Option<u64>,
    #[serde(default)]
    to_epoch_seconds: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct LoginEventsRetentionRequest {
    older_than_epoch_seconds: u64,
    #[serde(default = "default_retention_dry_run")]
    dry_run: bool,
}

#[derive(Debug, Default)]
struct LoginEventMetrics {
    written: AtomicU64,
    failed: AtomicU64,
    spoofed_headers_ignored: AtomicU64,
}

#[derive(Debug, Serialize)]
struct LoginEventMetricsSnapshot {
    #[serde(rename = "login_events_written_total")]
    events_written: u64,
    #[serde(rename = "login_events_write_failed_total")]
    events_write_failed: u64,
    #[serde(rename = "login_ip_spoof_header_ignored_total")]
    ip_spoof_header_ignored: u64,
}

struct SuccessfulLoginInput<'a> {
    headers: &'a HeaderMap,
    remote_addr: Option<SocketAddr>,
    login_surface: &'a str,
    account: &'a LocalLoginAccount,
    remember: bool,
    mfa_at_epoch_seconds: Option<u64>,
    mfa_amr_method: Option<&'static str>,
    auth_method: &'a str,
}

#[derive(Debug, Serialize)]
struct LoginResponse {
    user_id: String,
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    affiliate_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mfa_at_epoch_seconds: Option<u64>,
    session_token: String,
    expires_at_epoch_seconds: u64,
}

#[derive(Debug, Serialize)]
struct WebauthnRegistrationStartResponse {
    challenge_id: String,
    expires_at_epoch_seconds: i64,
    creation_options: Value,
}

#[derive(Debug, Deserialize)]
struct WebauthnRegistrationFinishRequest {
    #[serde(default)]
    challenge_id: String,
    #[serde(default)]
    registration_data: Value,
}

#[derive(Debug, Serialize)]
struct WebauthnAssertionStartResponse {
    challenge_id: String,
    expires_at_epoch_seconds: i64,
    request_options: Value,
}

#[derive(Debug, Deserialize)]
struct WebauthnAssertionFinishRequest {
    #[serde(default)]
    challenge_id: String,
    #[serde(default)]
    assertion_data: Value,
}

#[derive(Debug, Deserialize)]
struct WebauthnCredentialRevokeRequest {
    #[serde(default)]
    credential_id: String,
}

#[derive(Debug, Serialize)]
struct MeResponse {
    user_id: String,
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    affiliate_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mfa_at_epoch_seconds: Option<u64>,
    expires_at_epoch_seconds: u64,
}

#[derive(Debug, Serialize)]
struct JwksResponse {
    keys: Vec<JwkResponse>,
}

#[derive(Debug, Serialize)]
struct JwkResponse {
    kty: &'static str,
    crv: &'static str,
    kid: String,
    alg: &'static str,
    #[serde(rename = "use")]
    key_use: &'static str,
    x: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum LoginIdentifier {
    Email(String),
    Phone(String),
}

impl LoginIdentifier {
    fn as_normalized_str(&self) -> &str {
        match self {
            Self::Email(value) | Self::Phone(value) => value,
        }
    }
}

pub fn auth_router() -> Router {
    auth_router_with_config(AuthConfig::from_env())
}

pub fn auth_router_with_config(config: AuthConfig) -> Router {
    Router::new()
        .route("/api/auth/login", post(login))
        .route("/api/auth/me", get(me))
        .route("/api/auth/logout", post(logout))
        .route("/api/auth/jwks", get(jwks))
        .route("/v1/auth/login", post(login))
        .route("/v1/auth/me", get(me))
        .route("/v1/auth/logout", post(logout))
        .route("/v1/auth/jwks", get(jwks))
        .route("/v1/users/me/login-events", get(self_login_events))
        .route("/v1/internal/admin/login-events", get(admin_login_events))
        .route(
            "/v1/internal/admin/login-events/retention",
            post(admin_login_events_retention),
        )
        .route(
            "/v1/internal/admin/login-events/metrics",
            get(admin_login_events_metrics),
        )
        .route(
            "/v1/auth/webauthn/register/start",
            post(webauthn_register_start),
        )
        .route(
            "/v1/auth/webauthn/register/finish",
            post(webauthn_register_finish),
        )
        .route(
            "/v1/auth/webauthn/assert/start",
            post(webauthn_assert_start),
        )
        .route(
            "/v1/auth/webauthn/assert/finish",
            post(webauthn_assert_finish),
        )
        .route(
            "/v1/auth/webauthn/credentials/revoke",
            post(webauthn_credential_revoke),
        )
        .route("/.well-known/jwks.json", get(jwks))
        .with_state(config)
}

impl AuthConfig {
    #[must_use]
    pub fn new(accounts: Vec<LocalLoginAccount>) -> Self {
        Self {
            accounts: Arc::from(accounts.into_boxed_slice()),
            session_signer: None,
            consumed_recovery_codes: Arc::new(Mutex::new(HashSet::new())),
            consumed_recovery_code_store_path: None,
            webauthn_config: None,
            webauthn_store: Arc::new(InMemoryWebauthnStore::default()),
            credential_store: None,
            login_event_store: Some(Arc::new(InMemoryLoginEventStore::default())),
            login_identifier_hmac_key: Arc::new(b"prophetica-login-events-dev-key".to_vec()),
            login_identifier_hmac_key_version: "dev-v1".to_owned(),
            trusted_proxy_config: TrustedProxyConfig::default(),
            login_event_metrics: Arc::new(LoginEventMetrics::default()),
        }
    }

    #[must_use]
    pub fn with_session_signer(mut self, private_jwk: &str) -> Self {
        self.session_signer = SessionSigner::from_private_jwk(private_jwk);
        self
    }

    #[must_use]
    pub fn with_recovery_code_store_path(mut self, path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        self.consumed_recovery_codes = Arc::new(Mutex::new(read_consumed_recovery_codes(&path)));
        self.consumed_recovery_code_store_path = Some(Arc::new(path));
        self
    }

    #[must_use]
    pub fn with_webauthn_config(mut self, config: WebauthnRuntimeConfig) -> Self {
        self.webauthn_config = Some(config);
        self
    }

    #[must_use]
    pub fn with_webauthn_store(mut self, store: Arc<dyn WebauthnStore>) -> Self {
        self.webauthn_store = store;
        self
    }

    #[must_use]
    pub fn with_credential_store(mut self, store: Arc<dyn PasswordCredentialStore>) -> Self {
        self.credential_store = Some(store);
        self
    }

    #[must_use]
    pub fn with_login_event_store(mut self, store: Arc<dyn LoginEventStore>) -> Self {
        self.login_event_store = Some(store);
        self
    }

    #[must_use]
    pub fn with_login_identifier_hmac_key(
        mut self,
        key: impl Into<Vec<u8>>,
        key_version: impl Into<String>,
    ) -> Self {
        self.login_identifier_hmac_key = Arc::new(key.into());
        self.login_identifier_hmac_key_version = key_version.into();
        self
    }

    #[must_use]
    pub fn with_trusted_proxy_cidrs(mut self, cidrs_csv: &str) -> Self {
        self.trusted_proxy_config = TrustedProxyConfig::from_csv(cidrs_csv);
        self
    }

    #[must_use]
    pub fn from_env() -> Self {
        let prophetica_env = env::var("PROPHETICA_ENV").unwrap_or_else(|_| "dev".to_owned());

        if prophetica_env.eq_ignore_ascii_case("production") {
            return Self::new(Vec::new());
        }

        let accounts = if let Some(accounts) = Self::from_local_login_accounts_json_env() {
            accounts
        } else {
            let user_id = env::var("USER_SERVICE_LOCAL_LOGIN_USER_ID")
                .unwrap_or_else(|_| "user_42".to_owned());
            let email = env::var("USER_SERVICE_LOCAL_LOGIN_EMAIL")
                .unwrap_or_else(|_| "member@example.com".to_owned());
            let phone = env::var("USER_SERVICE_LOCAL_LOGIN_PHONE")
                .unwrap_or_else(|_| "0912345698".to_owned());
            let password = env::var("USER_SERVICE_LOCAL_LOGIN_PASSWORD")
                .unwrap_or_else(|_| "member".to_owned());
            let role =
                env::var("USER_SERVICE_LOCAL_LOGIN_ROLE").unwrap_or_else(|_| "player".to_owned());
            let affiliate_id = env::var("USER_SERVICE_LOCAL_LOGIN_AFFILIATE_ID").ok();

            LocalLoginAccount::new(user_id, email, phone, password)
                .map(|account| account.with_profile(role, affiliate_id))
                .map_or_else(Vec::new, |account| vec![account])
        };

        let mut config = Self::new(accounts);
        if let Ok(private_jwk) = env::var("USER_SERVICE_ED25519_SESSION_PRIVATE_JWK") {
            config = config.with_session_signer(&private_jwk);
        }
        if let Ok(path) = env::var("USER_SERVICE_RECOVERY_CODE_CONSUMED_PATH") {
            config = config.with_recovery_code_store_path(path);
        }
        if let Some(webauthn_config) = WebauthnRuntimeConfig::from_env() {
            config = config.with_webauthn_config(webauthn_config);
        }
        if let Ok(key) = env::var("USER_SERVICE_LOGIN_EVENT_IDENTIFIER_HMAC_KEY") {
            let version = env::var("USER_SERVICE_LOGIN_EVENT_IDENTIFIER_HMAC_KEY_VERSION")
                .unwrap_or_else(|_| "env-v1".to_owned());
            config = config.with_login_identifier_hmac_key(key.into_bytes(), version);
        }
        if let Ok(cidrs) = env::var("USER_SERVICE_TRUSTED_PROXY_CIDRS") {
            config = config.with_trusted_proxy_cidrs(&cidrs);
        }
        config
    }

    fn from_local_login_accounts_json_env() -> Option<Vec<LocalLoginAccount>> {
        let require_argon2id = env_flag("USER_SERVICE_REQUIRE_ARGON2ID_CREDENTIALS");
        let require_hardware_mfa = env_flag("USER_SERVICE_REQUIRE_HARDWARE_MFA");
        Self::from_local_login_accounts_json_with_security_policy(
            &env::var("USER_SERVICE_LOCAL_LOGIN_ACCOUNTS").ok()?,
            require_argon2id,
            require_hardware_mfa,
        )
    }

    #[must_use]
    pub fn from_local_login_accounts_json(raw_accounts: &str) -> Option<Vec<LocalLoginAccount>> {
        Self::from_local_login_accounts_json_with_policy(raw_accounts, false)
    }

    #[must_use]
    pub fn from_local_login_accounts_json_with_policy(
        raw_accounts: &str,
        require_argon2id: bool,
    ) -> Option<Vec<LocalLoginAccount>> {
        Self::from_local_login_accounts_json_with_security_policy(
            raw_accounts,
            require_argon2id,
            false,
        )
    }

    #[must_use]
    pub fn from_local_login_accounts_json_with_security_policy(
        raw_accounts: &str,
        require_argon2id: bool,
        require_hardware_mfa: bool,
    ) -> Option<Vec<LocalLoginAccount>> {
        let specs: Vec<LocalLoginAccountSpec> = serde_json::from_str(raw_accounts).ok()?;
        let accounts = specs
            .into_iter()
            .filter_map(|spec| {
                let phone = spec.phone.unwrap_or_else(|| "0912345698".to_owned());
                let role = spec.role.unwrap_or_else(|| "player".to_owned());
                if require_argon2id
                    && is_privileged_or_affiliate_role(&role)
                    && !spec.password.starts_with("$argon2id$")
                {
                    return None;
                }
                if require_hardware_mfa
                    && is_privileged_or_affiliate_role(&role)
                    && !has_hardware_mfa(&spec.mfa_methods, spec.mfa_at_epoch_seconds)
                {
                    return None;
                }
                LocalLoginAccount::new(spec.user_id, spec.email, phone, spec.password).map(
                    |account| {
                        account
                            .with_profile(role, spec.affiliate_id)
                            .with_mfa(spec.mfa_methods, spec.mfa_at_epoch_seconds)
                            .with_totp(spec.totp_secret_base32, spec.recovery_codes)
                            .with_webauthn_user_uuid(spec.webauthn_user_uuid)
                    },
                )
            })
            .collect::<Vec<_>>();

        if accounts.is_empty() {
            None
        } else {
            Some(accounts)
        }
    }

    fn account_by_user_id(&self, user_id: &str) -> Option<&LocalLoginAccount> {
        self.accounts
            .iter()
            .find(|account| account.user_id == user_id)
    }

    async fn stored_account_for_login(
        &self,
        identifier: &LoginIdentifier,
        password: &str,
    ) -> Option<LocalLoginAccount> {
        let credential = self
            .credential_store
            .as_ref()?
            .credential_by_identifier(identifier)
            .await
            .ok()??;
        if !verify_argon2id_password(&credential.password_hash, password) {
            return None;
        }
        LocalLoginAccount::new(
            credential.user_id,
            credential.email,
            credential.phone,
            credential.password_hash,
        )
        .map(|account| account.with_profile(credential.role, credential.affiliate_id))
    }

    async fn stored_account_by_user_id(&self, user_id: &str) -> Option<LocalLoginAccount> {
        let credential = self
            .credential_store
            .as_ref()?
            .credential_by_user_id(user_id)
            .await
            .ok()??;
        LocalLoginAccount::new(
            credential.user_id,
            credential.email,
            credential.phone,
            credential.password_hash,
        )
        .map(|account| account.with_profile(credential.role, credential.affiliate_id))
    }

    async fn role_for_user_id(&self, user_id: &str) -> Option<String> {
        if let Some(account) = self.account_by_user_id(user_id) {
            return Some(account.role.clone());
        }
        self.stored_account_by_user_id(user_id)
            .await
            .map(|account| account.role)
    }

    fn consume_recovery_code(&self, account: &LocalLoginAccount, candidate: &str) -> bool {
        if !account
            .recovery_codes
            .iter()
            .any(|code| constant_time_equals_str(code, candidate.trim()))
        {
            return false;
        }
        let key = recovery_code_consumed_key(&account.user_id, candidate.trim());
        let Ok(mut consumed) = self.consumed_recovery_codes.lock() else {
            return false;
        };
        if consumed.contains(&key) {
            return false;
        }
        consumed.insert(key);
        if let Some(path) = self.consumed_recovery_code_store_path.as_deref() {
            if persist_consumed_recovery_codes(path, &consumed).is_err() {
                return false;
            }
        }
        true
    }
}

impl InMemoryPasswordCredentialStore {
    #[must_use]
    pub fn new(credentials: Vec<StoredLoginCredential>) -> Self {
        Self {
            credentials: Arc::new(credentials),
        }
    }
}

impl PostgresPasswordCredentialStore {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PasswordCredentialStore for InMemoryPasswordCredentialStore {
    async fn credential_by_identifier(
        &self,
        identifier: &LoginIdentifier,
    ) -> Result<Option<StoredLoginCredential>, PasswordCredentialStoreError> {
        Ok(self
            .credentials
            .iter()
            .find(|credential| match identifier {
                LoginIdentifier::Email(email) => credential.email == *email,
                LoginIdentifier::Phone(phone) => credential.phone == *phone,
            })
            .cloned())
    }

    async fn credential_by_user_id(
        &self,
        user_id: &str,
    ) -> Result<Option<StoredLoginCredential>, PasswordCredentialStoreError> {
        Ok(self
            .credentials
            .iter()
            .find(|credential| credential.user_id == user_id)
            .cloned())
    }
}

#[async_trait]
impl PasswordCredentialStore for PostgresPasswordCredentialStore {
    async fn credential_by_identifier(
        &self,
        identifier: &LoginIdentifier,
    ) -> Result<Option<StoredLoginCredential>, PasswordCredentialStoreError> {
        let (column, value) = match identifier {
            LoginIdentifier::Email(email) => ("email", email.as_str()),
            LoginIdentifier::Phone(phone) => ("phone", phone.as_str()),
        };
        let query = format!(
            "SELECT user_id, email, phone, password_hash, role, affiliate_id FROM user_login_credentials WHERE {column} = $1 AND disabled_at IS NULL"
        );
        let row = sqlx::query(&query)
            .bind(value)
            .fetch_optional(&self.pool)
            .await
            .map_err(|_| PasswordCredentialStoreError::Unavailable)?;

        row.as_ref().map(row_to_stored_login_credential).transpose()
    }

    async fn credential_by_user_id(
        &self,
        user_id: &str,
    ) -> Result<Option<StoredLoginCredential>, PasswordCredentialStoreError> {
        let row = sqlx::query(
            "SELECT user_id, email, phone, password_hash, role, affiliate_id FROM user_login_credentials WHERE user_id = $1 AND disabled_at IS NULL",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| PasswordCredentialStoreError::Unavailable)?;

        row.as_ref().map(row_to_stored_login_credential).transpose()
    }
}

fn row_to_stored_login_credential(
    row: &sqlx::postgres::PgRow,
) -> Result<StoredLoginCredential, PasswordCredentialStoreError> {
    Ok(StoredLoginCredential {
        user_id: row
            .try_get("user_id")
            .map_err(|_| PasswordCredentialStoreError::Unavailable)?,
        email: row
            .try_get("email")
            .map_err(|_| PasswordCredentialStoreError::Unavailable)?,
        phone: row
            .try_get("phone")
            .map_err(|_| PasswordCredentialStoreError::Unavailable)?,
        password_hash: row
            .try_get("password_hash")
            .map_err(|_| PasswordCredentialStoreError::Unavailable)?,
        role: row
            .try_get("role")
            .map_err(|_| PasswordCredentialStoreError::Unavailable)?,
        affiliate_id: row
            .try_get("affiliate_id")
            .map_err(|_| PasswordCredentialStoreError::Unavailable)?,
    })
}

fn env_flag(name: &str) -> bool {
    env::var(name)
        .ok()
        .is_some_and(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
}

fn is_privileged_or_affiliate_role(role: &str) -> bool {
    matches!(role.trim(), "admin" | "manager" | "affiliate")
}

fn has_hardware_mfa(methods: &[String], mfa_at_epoch_seconds: Option<u64>) -> bool {
    mfa_at_epoch_seconds.is_some_and(|mfa_at| mfa_at > 0)
        && methods.iter().any(|method| {
            matches!(
                method.trim().to_ascii_lowercase().as_str(),
                "webauthn" | "passkey" | "fido2" | "security_key" | "hardware_key"
            )
        })
}

impl LocalLoginAccount {
    #[must_use]
    pub fn new(
        user_id: impl Into<String>,
        email: impl AsRef<str>,
        phone: impl AsRef<str>,
        password: impl Into<String>,
    ) -> Option<Self> {
        let user_id = user_id.into();
        let password = password.into();
        let email = normalize_email(email.as_ref())?;
        let phone = normalize_phone(phone.as_ref())?;

        if user_id.trim().is_empty() || password.is_empty() {
            return None;
        }

        Some(Self {
            user_id,
            email,
            phone,
            password,
            role: "player".to_owned(),
            affiliate_id: None,
            mfa_methods: Vec::new(),
            mfa_at_epoch_seconds: None,
            totp_secret_base32: None,
            recovery_codes: Vec::new(),
            webauthn_user_uuid: None,
        })
    }

    #[must_use]
    pub fn with_profile(mut self, role: impl Into<String>, affiliate_id: Option<String>) -> Self {
        let role = role.into();
        if !role.trim().is_empty() {
            self.role = role;
        }
        self.affiliate_id = affiliate_id.and_then(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_owned())
            }
        });
        self
    }

    #[must_use]
    pub fn with_mfa(mut self, methods: Vec<String>, mfa_at_epoch_seconds: Option<u64>) -> Self {
        self.mfa_methods = methods
            .into_iter()
            .map(|method| method.trim().to_ascii_lowercase())
            .filter(|method| !method.is_empty())
            .collect();
        self.mfa_at_epoch_seconds = mfa_at_epoch_seconds.filter(|value| *value > 0);
        self
    }

    #[must_use]
    pub fn with_totp(mut self, secret_base32: Option<String>, recovery_codes: Vec<String>) -> Self {
        self.totp_secret_base32 = secret_base32
            .map(|secret| secret.trim().replace(' ', ""))
            .filter(|secret| !secret.is_empty());
        self.recovery_codes = recovery_codes
            .into_iter()
            .map(|code| code.trim().to_owned())
            .filter(|code| !code.is_empty())
            .collect();
        self
    }

    #[must_use]
    pub fn with_webauthn_user_uuid(mut self, user_uuid: Option<String>) -> Self {
        self.webauthn_user_uuid = user_uuid
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        self
    }
}

async fn login(
    State(config): State<AuthConfig>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    Json(request): Json<LoginRequest>,
) -> Response {
    let remote_addr = connect_info.map(|ConnectInfo(addr)| addr);
    let login_surface = normalize_login_surface(request.login_surface.as_deref());
    if config.accounts.is_empty() && config.credential_store.is_none() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "error": "ERROR_CODE_AUTH_PROVIDER_UNAVAILABLE" })),
        )
            .into_response();
    }

    let Some(identifier) = normalize_identifier(&request.identifier) else {
        record_login_failure(
            &config,
            &headers,
            remote_addr,
            &login_surface,
            None,
            "invalid_identifier",
            "password",
        )
        .await;
        return invalid_credentials();
    };

    if !identifier_kind_matches(&request.identifier_kind, &identifier) {
        record_login_failure(
            &config,
            &headers,
            remote_addr,
            &login_surface,
            Some(&identifier),
            "invalid_identifier",
            "password",
        )
        .await;
        return invalid_credentials();
    }

    if request.password.is_empty() {
        record_login_failure(
            &config,
            &headers,
            remote_addr,
            &login_surface,
            Some(&identifier),
            "invalid_credentials",
            "password",
        )
        .await;
        return invalid_credentials();
    }

    let account = config.accounts.iter().find(|account| {
        account.matches_identifier(&identifier) && account.verify_password(&request.password)
    });

    let stored_account;
    let account = if let Some(account) = account {
        Some(account)
    } else {
        stored_account = config
            .stored_account_for_login(&identifier, &request.password)
            .await;
        stored_account.as_ref()
    };

    if let Some(account) = account {
        return authenticated_password_login(
            &config,
            &headers,
            remote_addr,
            &login_surface,
            account,
            &request,
        )
        .await;
    }

    record_login_failure(
        &config,
        &headers,
        remote_addr,
        &login_surface,
        Some(&identifier),
        "invalid_credentials",
        "password",
    )
    .await;
    invalid_credentials()
}

async fn authenticated_password_login(
    config: &AuthConfig,
    headers: &HeaderMap,
    remote_addr: Option<SocketAddr>,
    login_surface: &str,
    account: &LocalLoginAccount,
    request: &LoginRequest,
) -> Response {
    let (mfa_at_epoch_seconds, mfa_amr_method) = if account.requires_mfa_challenge() {
        let Some(method) = account.verify_mfa_challenge(
            request.mfa_code.as_deref(),
            request.mfa_recovery_code.as_deref(),
            unix_now_seconds(),
            config,
        ) else {
            record_login_failure_for_account(
                config,
                headers,
                remote_addr,
                login_surface,
                account,
                "mfa_required",
                "password",
            )
            .await;
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": "ERROR_CODE_MFA_REQUIRED" })),
            )
                .into_response();
        };
        (Some(unix_now_seconds()), Some(method))
    } else {
        (
            account.mfa_at_epoch_seconds,
            has_hardware_mfa(&account.mfa_methods, account.mfa_at_epoch_seconds).then_some("hwk"),
        )
    };
    successful_login(
        config,
        SuccessfulLoginInput {
            headers,
            remote_addr,
            login_surface,
            account,
            remember: request.remember,
            mfa_at_epoch_seconds,
            mfa_amr_method,
            auth_method: "password",
        },
    )
    .await
}

async fn self_login_events(
    State(config): State<AuthConfig>,
    headers: HeaderMap,
    Query(query): Query<LoginEventsQuery>,
) -> Response {
    let Some(token) = bearer_token(&headers) else {
        return invalid_credentials();
    };
    let Some(session) = parse_session_token(&config, token) else {
        return invalid_credentials();
    };
    if session.expires_at_epoch_seconds <= unix_now_seconds() {
        return invalid_credentials();
    }
    let limit = query.limit.unwrap_or(20).clamp(1, 50);
    let Some(store) = config.login_event_store.as_ref() else {
        return Json(json!({ "events": [] })).into_response();
    };
    match store.list_for_principal(&session.user_id, limit).await {
        Ok(events) => {
            let events = events.iter().map(to_self_response).collect::<Vec<_>>();
            Json(json!({ "events": events })).into_response()
        }
        Err(_) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "error": "ERROR_CODE_LOGIN_EVENTS_UNAVAILABLE" })),
        )
            .into_response(),
    }
}

async fn admin_login_events(
    State(config): State<AuthConfig>,
    headers: HeaderMap,
    Query(query): Query<LoginEventsQuery>,
) -> Response {
    let Some(token) = bearer_token(&headers) else {
        return invalid_credentials();
    };
    let Some(session) = parse_session_token(&config, token) else {
        return invalid_credentials();
    };
    if session.expires_at_epoch_seconds <= unix_now_seconds() {
        return invalid_credentials();
    }
    let Some(role) = config.role_for_user_id(&session.user_id).await else {
        return invalid_credentials();
    };
    if !matches!(role.as_str(), "admin" | "manager") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "ERROR_CODE_LOGIN_EVENT_ACCESS_DENIED" })),
        )
            .into_response();
    }
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let Some(store) = config.login_event_store.as_ref() else {
        return Json(json!({ "events": [] })).into_response();
    };
    match store
        .list_admin(AdminLoginEventFilter {
            limit,
            principal_type: query.principal_type,
            principal_id: query.principal_id,
            user_id: query.user_id,
            ip_address: query.ip_address,
            event_type: query.event_type,
            from_epoch_seconds: query.from_epoch_seconds,
            to_epoch_seconds: query.to_epoch_seconds,
        })
        .await
    {
        Ok(events) => Json(json!({ "events": events })).into_response(),
        Err(_) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "error": "ERROR_CODE_LOGIN_EVENTS_UNAVAILABLE" })),
        )
            .into_response(),
    }
}

async fn admin_login_events_retention(
    State(config): State<AuthConfig>,
    headers: HeaderMap,
    Json(request): Json<LoginEventsRetentionRequest>,
) -> Response {
    if !admin_can_query_login_events(&config, &headers).await {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "ERROR_CODE_LOGIN_EVENT_ACCESS_DENIED" })),
        )
            .into_response();
    }
    let Some(store) = config.login_event_store.as_ref() else {
        return (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "error": "ERROR_CODE_LOGIN_EVENTS_UNAVAILABLE" })),
        )
            .into_response();
    };
    match store
        .delete_older_than(request.older_than_epoch_seconds, request.dry_run)
        .await
    {
        Ok(result) => Json(result).into_response(),
        Err(_) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "error": "ERROR_CODE_LOGIN_EVENTS_UNAVAILABLE" })),
        )
            .into_response(),
    }
}

async fn admin_login_events_metrics(
    State(config): State<AuthConfig>,
    headers: HeaderMap,
) -> Response {
    if !admin_can_query_login_events(&config, &headers).await {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "ERROR_CODE_LOGIN_EVENT_ACCESS_DENIED" })),
        )
            .into_response();
    }
    Json(config.login_event_metrics.snapshot()).into_response()
}

async fn me(State(config): State<AuthConfig>, headers: HeaderMap) -> Response {
    let Some(token) = bearer_token(&headers) else {
        return invalid_credentials();
    };
    let Some(session) = parse_session_token(&config, token) else {
        return invalid_credentials();
    };
    if session.expires_at_epoch_seconds <= unix_now_seconds() {
        return invalid_credentials();
    }
    if let Some(account) = config.account_by_user_id(&session.user_id) {
        return Json(MeResponse {
            user_id: account.user_id.clone(),
            role: account.role.clone(),
            affiliate_id: account.affiliate_id.clone(),
            mfa_at_epoch_seconds: session
                .mfa_at_epoch_seconds
                .or(account.mfa_at_epoch_seconds),
            expires_at_epoch_seconds: session.expires_at_epoch_seconds,
        })
        .into_response();
    }
    let Some(account) = config.stored_account_by_user_id(&session.user_id).await else {
        return invalid_credentials();
    };
    Json(MeResponse {
        user_id: account.user_id,
        role: account.role,
        affiliate_id: account.affiliate_id,
        mfa_at_epoch_seconds: session
            .mfa_at_epoch_seconds
            .or(account.mfa_at_epoch_seconds),
        expires_at_epoch_seconds: session.expires_at_epoch_seconds,
    })
    .into_response()
}

async fn admin_can_query_login_events(config: &AuthConfig, headers: &HeaderMap) -> bool {
    let Some(token) = bearer_token(headers) else {
        return false;
    };
    let Some(session) = parse_session_token(config, token) else {
        return false;
    };
    if session.expires_at_epoch_seconds <= unix_now_seconds() {
        return false;
    }
    config
        .role_for_user_id(&session.user_id)
        .await
        .is_some_and(|role| matches!(role.as_str(), "admin" | "manager"))
}

fn default_retention_dry_run() -> bool {
    true
}

impl LoginEventMetrics {
    fn snapshot(&self) -> LoginEventMetricsSnapshot {
        LoginEventMetricsSnapshot {
            events_written: self.written.load(Ordering::Relaxed),
            events_write_failed: self.failed.load(Ordering::Relaxed),
            ip_spoof_header_ignored: self.spoofed_headers_ignored.load(Ordering::Relaxed),
        }
    }
}

async fn logout() -> Response {
    StatusCode::NO_CONTENT.into_response()
}

async fn jwks(State(config): State<AuthConfig>) -> Response {
    let keys = config
        .session_signer
        .as_ref()
        .map(|signer| {
            vec![JwkResponse {
                kty: "OKP",
                crv: "Ed25519",
                kid: signer.kid.clone(),
                alg: "EdDSA",
                key_use: "sig",
                x: signer.public_x.clone(),
            }]
        })
        .unwrap_or_default();
    Json(JwksResponse { keys }).into_response()
}

async fn webauthn_register_start(State(config): State<AuthConfig>, headers: HeaderMap) -> Response {
    let Some(token) = bearer_token(&headers) else {
        return invalid_credentials();
    };
    let Some(session) = parse_session_token(&config, token) else {
        return invalid_credentials();
    };
    if session.expires_at_epoch_seconds <= unix_now_seconds() {
        return invalid_credentials();
    }
    let Some(account) = config.account_by_user_id(&session.user_id) else {
        return invalid_credentials();
    };
    let Some(webauthn_config) = config.webauthn_config.as_ref() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "error": "ERROR_CODE_WEBAUTHN_UNAVAILABLE" })),
        )
            .into_response();
    };
    let Some(user_uuid) = account.webauthn_user_uuid.as_deref() else {
        return (
            StatusCode::CONFLICT,
            Json(json!({ "error": "ERROR_CODE_WEBAUTHN_USER_UUID_REQUIRED" })),
        )
            .into_response();
    };
    let now = i64::try_from(unix_now_seconds()).unwrap_or(i64::MAX);
    let Ok(start) = start_and_store_passkey_registration(
        config.webauthn_store.as_ref(),
        webauthn_config,
        &account.user_id,
        user_uuid,
        &account.email,
        &account.email,
        now,
    )
    .await
    else {
        return (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "error": "ERROR_CODE_WEBAUTHN_REGISTRATION_START_FAILED" })),
        )
            .into_response();
    };

    Json(WebauthnRegistrationStartResponse {
        challenge_id: start.challenge_id,
        expires_at_epoch_seconds: start.expires_at_epoch_seconds,
        creation_options: start.creation_options,
    })
    .into_response()
}

async fn webauthn_register_finish(
    State(config): State<AuthConfig>,
    headers: HeaderMap,
    Json(request): Json<WebauthnRegistrationFinishRequest>,
) -> Response {
    let Some(token) = bearer_token(&headers) else {
        return invalid_credentials();
    };
    let Some(session) = parse_session_token(&config, token) else {
        return invalid_credentials();
    };
    if session.expires_at_epoch_seconds <= unix_now_seconds() {
        return invalid_credentials();
    }
    let Some(account) = config.account_by_user_id(&session.user_id) else {
        return invalid_credentials();
    };
    let Some(webauthn_config) = config.webauthn_config.as_ref() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "error": "ERROR_CODE_WEBAUTHN_UNAVAILABLE" })),
        )
            .into_response();
    };
    let Some(user_uuid) = account.webauthn_user_uuid.as_deref() else {
        return (
            StatusCode::CONFLICT,
            Json(json!({ "error": "ERROR_CODE_WEBAUTHN_USER_UUID_REQUIRED" })),
        )
            .into_response();
    };
    if request.challenge_id.trim().is_empty() || !request.registration_data.is_object() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "ERROR_CODE_INVALID_WEBAUTHN_REGISTRATION_FINISH" })),
        )
            .into_response();
    }
    let now = i64::try_from(unix_now_seconds()).unwrap_or(i64::MAX);
    let Ok(challenge) = config
        .webauthn_store
        .consume_challenge(&request.challenge_id, now)
        .await
    else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "ERROR_CODE_WEBAUTHN_CHALLENGE_INVALID" })),
        )
            .into_response();
    };
    if challenge.user_id != account.user_id || challenge.ceremony != WebauthnCeremony::Registration
    {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "ERROR_CODE_WEBAUTHN_CHALLENGE_INVALID" })),
        )
            .into_response();
    }
    let registration_credential_id =
        webauthn_registration_credential_id(&request.registration_data);
    let Ok(passkey) = finish_passkey_registration(
        webauthn_config,
        request.registration_data,
        challenge.state_json,
    ) else {
        return (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "error": "ERROR_CODE_WEBAUTHN_REGISTRATION_FINISH_FAILED" })),
        )
            .into_response();
    };
    let Some(credential_id) = webauthn_credential_id(&passkey).or(registration_credential_id)
    else {
        return (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "error": "ERROR_CODE_WEBAUTHN_CREDENTIAL_ID_MISSING" })),
        )
            .into_response();
    };
    let record =
        registered_webauthn_credential_record(account, user_uuid, &credential_id, passkey, now);
    if config.webauthn_store.put_credential(record).await.is_err() {
        return (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "error": "ERROR_CODE_WEBAUTHN_CREDENTIAL_STORE_FAILED" })),
        )
            .into_response();
    }

    Json(json!({ "credential_id": credential_id })).into_response()
}

async fn webauthn_assert_start(State(config): State<AuthConfig>, headers: HeaderMap) -> Response {
    let Some(token) = bearer_token(&headers) else {
        return invalid_credentials();
    };
    let Some(session) = parse_session_token(&config, token) else {
        return invalid_credentials();
    };
    if session.expires_at_epoch_seconds <= unix_now_seconds() {
        return invalid_credentials();
    }
    let Some(account) = config.account_by_user_id(&session.user_id) else {
        return invalid_credentials();
    };
    let Some(webauthn_config) = config.webauthn_config.as_ref() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "error": "ERROR_CODE_WEBAUTHN_UNAVAILABLE" })),
        )
            .into_response();
    };
    let Ok(credentials) = config
        .webauthn_store
        .list_active_credentials(&account.user_id)
        .await
    else {
        return (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "error": "ERROR_CODE_WEBAUTHN_CREDENTIAL_LOOKUP_FAILED" })),
        )
            .into_response();
    };
    if credentials.is_empty() {
        return (
            StatusCode::CONFLICT,
            Json(json!({ "error": "ERROR_CODE_WEBAUTHN_CREDENTIAL_REQUIRED" })),
        )
            .into_response();
    }
    let now = i64::try_from(unix_now_seconds()).unwrap_or(i64::MAX);
    let Ok(start) = start_and_store_passkey_assertion(
        config.webauthn_store.as_ref(),
        webauthn_config,
        &account.user_id,
        &credentials,
        now,
    )
    .await
    else {
        return (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "error": "ERROR_CODE_WEBAUTHN_ASSERTION_START_FAILED" })),
        )
            .into_response();
    };

    Json(WebauthnAssertionStartResponse {
        challenge_id: start.challenge_id,
        expires_at_epoch_seconds: start.expires_at_epoch_seconds,
        request_options: start.request_options,
    })
    .into_response()
}

async fn webauthn_assert_finish(
    State(config): State<AuthConfig>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    Json(request): Json<WebauthnAssertionFinishRequest>,
) -> Response {
    let remote_addr = connect_info.map(|ConnectInfo(addr)| addr);
    let Some(token) = bearer_token(&headers) else {
        return invalid_credentials();
    };
    let Some(session) = parse_session_token(&config, token) else {
        return invalid_credentials();
    };
    if session.expires_at_epoch_seconds <= unix_now_seconds() {
        return invalid_credentials();
    }
    let Some(account) = config.account_by_user_id(&session.user_id) else {
        return invalid_credentials();
    };
    let Some(webauthn_config) = config.webauthn_config.as_ref() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "error": "ERROR_CODE_WEBAUTHN_UNAVAILABLE" })),
        )
            .into_response();
    };
    if request.challenge_id.trim().is_empty() || !request.assertion_data.is_object() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "ERROR_CODE_INVALID_WEBAUTHN_ASSERTION_FINISH" })),
        )
            .into_response();
    }
    let now = i64::try_from(unix_now_seconds()).unwrap_or(i64::MAX);
    let Ok(challenge) = config
        .webauthn_store
        .consume_challenge(&request.challenge_id, now)
        .await
    else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "ERROR_CODE_WEBAUTHN_CHALLENGE_INVALID" })),
        )
            .into_response();
    };
    if challenge.user_id != account.user_id || challenge.ceremony != WebauthnCeremony::Assertion {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "ERROR_CODE_WEBAUTHN_CHALLENGE_INVALID" })),
        )
            .into_response();
    }
    let Ok(credentials) = config
        .webauthn_store
        .list_active_credentials(&account.user_id)
        .await
    else {
        return (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "error": "ERROR_CODE_WEBAUTHN_CREDENTIAL_LOOKUP_FAILED" })),
        )
            .into_response();
    };
    let Ok(assertion) = finish_passkey_assertion(
        webauthn_config,
        request.assertion_data,
        challenge.state_json,
        &credentials,
    ) else {
        return (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "error": "ERROR_CODE_WEBAUTHN_ASSERTION_FINISH_FAILED" })),
        )
            .into_response();
    };
    let store_result =
        store_successful_webauthn_assertion(&config, credentials, assertion, now).await;
    if let Err(response) = store_result {
        return response;
    };

    successful_login(
        &config,
        SuccessfulLoginInput {
            headers: &headers,
            remote_addr,
            login_surface: "user_web",
            account,
            remember: false,
            mfa_at_epoch_seconds: Some(unix_now_seconds()),
            mfa_amr_method: Some("hwk"),
            auth_method: "passkey",
        },
    )
    .await
}

fn registered_webauthn_credential_record(
    account: &LocalLoginAccount,
    user_uuid: &str,
    credential_id: &str,
    mut passkey: Value,
    now_epoch_seconds: i64,
) -> WebauthnCredentialRecord {
    if let Some(passkey_object) = passkey.as_object_mut() {
        passkey_object
            .entry("credential_id")
            .or_insert_with(|| json!(credential_id));
    }

    WebauthnCredentialRecord {
        credential_id: credential_id.to_owned(),
        user_id: account.user_id.clone(),
        user_webauthn_uuid: user_uuid.to_owned(),
        display_name: account.email.clone(),
        public_key_cose: passkey.clone(),
        signature_counter: 0,
        transports: json!([]),
        user_verified: true,
        backup_eligible: false,
        backup_state: false,
        registration_policy: "required".to_owned(),
        attestation_format: None,
        attestation: json!({}),
        created_at_epoch_seconds: now_epoch_seconds,
        last_used_at_epoch_seconds: None,
        revoked_at_epoch_seconds: None,
        metadata: json!({ "passkey": passkey }),
    }
}

async fn store_successful_webauthn_assertion(
    config: &AuthConfig,
    credentials: Vec<WebauthnCredentialRecord>,
    assertion: PasskeyAssertionFinish,
    now_epoch_seconds: i64,
) -> Result<(), Response> {
    let Some(mut credential) = credentials
        .into_iter()
        .find(|record| record.credential_id == assertion.credential_id)
    else {
        return Err((
            StatusCode::BAD_GATEWAY,
            Json(json!({ "error": "ERROR_CODE_WEBAUTHN_CREDENTIAL_NOT_FOUND" })),
        )
            .into_response());
    };

    credential.public_key_cose = assertion.updated_passkey;
    credential.signature_counter = assertion.signature_counter;
    credential.user_verified = assertion.user_verified;
    credential.backup_eligible = assertion.backup_eligible;
    credential.backup_state = assertion.backup_state;
    credential.last_used_at_epoch_seconds = Some(now_epoch_seconds);
    credential.metadata = json!({ "authentication_result": assertion.authentication_result });
    config
        .webauthn_store
        .put_credential(credential)
        .await
        .map_err(|_| {
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({ "error": "ERROR_CODE_WEBAUTHN_CREDENTIAL_STORE_FAILED" })),
            )
                .into_response()
        })
}

async fn webauthn_credential_revoke(
    State(config): State<AuthConfig>,
    headers: HeaderMap,
    Json(request): Json<WebauthnCredentialRevokeRequest>,
) -> Response {
    let Some(token) = bearer_token(&headers) else {
        return invalid_credentials();
    };
    let Some(session) = parse_session_token(&config, token) else {
        return invalid_credentials();
    };
    if session.expires_at_epoch_seconds <= unix_now_seconds() {
        return invalid_credentials();
    }
    let Some(account) = config.account_by_user_id(&session.user_id) else {
        return invalid_credentials();
    };
    let credential_id = request.credential_id.trim();
    if credential_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "ERROR_CODE_WEBAUTHN_CREDENTIAL_ID_REQUIRED" })),
        )
            .into_response();
    }
    let Ok(credentials) = config
        .webauthn_store
        .list_active_credentials(&account.user_id)
        .await
    else {
        return (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "error": "ERROR_CODE_WEBAUTHN_CREDENTIAL_LOOKUP_FAILED" })),
        )
            .into_response();
    };
    if !credentials
        .iter()
        .any(|credential| credential.credential_id == credential_id)
    {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "ERROR_CODE_WEBAUTHN_CREDENTIAL_NOT_FOUND" })),
        )
            .into_response();
    }
    let now = i64::try_from(unix_now_seconds()).unwrap_or(i64::MAX);
    if config
        .webauthn_store
        .revoke_credential(credential_id, now)
        .await
        .is_err()
    {
        return (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "error": "ERROR_CODE_WEBAUTHN_CREDENTIAL_REVOKE_FAILED" })),
        )
            .into_response();
    }

    Json(json!({ "credential_id": credential_id, "revoked": true })).into_response()
}

fn webauthn_credential_id(passkey: &Value) -> Option<String> {
    passkey
        .get("cred_id")
        .or_else(|| passkey.get("credential_id"))
        .and_then(|value| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .or_else(|| serde_json::to_string(value).ok())
        })
        .filter(|value| !value.trim().is_empty())
}

fn webauthn_registration_credential_id(registration_data: &Value) -> Option<String> {
    registration_data
        .get("id")
        .or_else(|| registration_data.get("rawId"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .filter(|value| !value.trim().is_empty())
}

impl LocalLoginAccount {
    fn matches_identifier(&self, identifier: &LoginIdentifier) -> bool {
        match identifier {
            LoginIdentifier::Email(email) => self.email == *email,
            LoginIdentifier::Phone(phone) => self.phone == *phone,
        }
    }

    fn session_amr_with_method(mfa_amr_method: Option<&'static str>) -> Vec<&'static str> {
        let mut methods = vec!["pwd"];
        if let Some(method) = mfa_amr_method {
            methods.push(method);
        }
        methods
    }

    fn requires_mfa_challenge(&self) -> bool {
        self.totp_secret_base32.is_some() || !self.recovery_codes.is_empty()
    }

    fn verify_mfa_challenge(
        &self,
        totp_code: Option<&str>,
        recovery_code: Option<&str>,
        now_seconds: u64,
        config: &AuthConfig,
    ) -> Option<&'static str> {
        if totp_code.is_some_and(|code| {
            self.totp_secret_base32
                .as_deref()
                .is_some_and(|secret| verify_totp(secret, code, now_seconds))
        }) {
            return Some("otp");
        }
        if recovery_code.is_some_and(|candidate| config.consume_recovery_code(self, candidate)) {
            return Some("recovery");
        }
        None
    }

    fn verify_password(&self, candidate: &str) -> bool {
        if self.password.starts_with("$argon2id$") {
            return verify_argon2id_password(&self.password, candidate);
        }
        self.password == candidate
    }
}

fn verify_argon2id_password(encoded_hash: &str, candidate: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(encoded_hash) else {
        return false;
    };
    if !argon2id_params_meet_minimum(&parsed) {
        return false;
    }
    Argon2::default()
        .verify_password(candidate.as_bytes(), &parsed)
        .is_ok()
}

fn argon2id_params_meet_minimum(hash: &PasswordHash<'_>) -> bool {
    if hash.algorithm.as_str() != "argon2id" {
        return false;
    }
    let memory_cost = hash
        .params
        .get("m")
        .and_then(|value| value.decimal().ok())
        .unwrap_or_default();
    let time_cost = hash
        .params
        .get("t")
        .and_then(|value| value.decimal().ok())
        .unwrap_or_default();
    let parallelism = hash
        .params
        .get("p")
        .and_then(|value| value.decimal().ok())
        .unwrap_or_default();
    memory_cost >= 19_456 && time_cost >= 2 && parallelism >= 1
}

fn verify_totp(secret_base32: &str, candidate: &str, now_seconds: u64) -> bool {
    let Some(secret) = base32_decode(secret_base32) else {
        return false;
    };
    let code = candidate.trim();
    if code.len() != 6 || !code.bytes().all(|byte| byte.is_ascii_digit()) {
        return false;
    }
    let counter = now_seconds / TOTP_STEP_SECONDS;
    [
        counter.saturating_sub(1),
        counter,
        counter.saturating_add(1),
    ]
    .into_iter()
    .any(|window_counter| {
        generate_totp_code(&secret, window_counter)
            .is_some_and(|expected| constant_time_equals_str(&expected, code))
    })
}

fn generate_totp_code(secret: &[u8], counter: u64) -> Option<String> {
    let mut mac = Hmac::<Sha1>::new_from_slice(secret).ok()?;
    mac.update(&counter.to_be_bytes());
    let digest = mac.finalize().into_bytes();
    let offset = usize::from(digest[19] & 0x0f);
    let binary = ((u32::from(digest[offset]) & 0x7f) << 24)
        | (u32::from(digest[offset + 1]) << 16)
        | (u32::from(digest[offset + 2]) << 8)
        | u32::from(digest[offset + 3]);
    Some(format!("{:06}", binary % TOTP_DIGITS_MODULUS))
}

fn base32_decode(input: &str) -> Option<Vec<u8>> {
    let mut bits = 0_u32;
    let mut bit_count = 0_u8;
    let mut out = Vec::new();
    for byte in input.bytes() {
        if byte == b'=' || byte == b' ' || byte == b'-' {
            continue;
        }
        let value = match byte.to_ascii_uppercase() {
            b'A'..=b'Z' => byte.to_ascii_uppercase() - b'A',
            b'2'..=b'7' => byte - b'2' + 26,
            _ => return None,
        };
        bits = (bits << 5) | u32::from(value);
        bit_count += 5;
        if bit_count >= 8 {
            bit_count -= 8;
            out.push(((bits >> bit_count) & 0xff) as u8);
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

fn constant_time_equals_str(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0_u8;
    for (left, right) in a.bytes().zip(b.bytes()) {
        diff |= left ^ right;
    }
    diff == 0
}

fn recovery_code_consumed_key(user_id: &str, recovery_code: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    let mut hasher = Sha1::new();
    hasher.update(user_id.as_bytes());
    hasher.update(b":");
    hasher.update(recovery_code.as_bytes());
    let digest = hasher.finalize();

    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push(char::from(HEX[usize::from(byte >> 4)]));
        out.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    out
}

fn read_consumed_recovery_codes(path: &PathBuf) -> HashSet<String> {
    fs::read_to_string(path)
        .ok()
        .map(|content| {
            content
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn persist_consumed_recovery_codes(
    path: &PathBuf,
    consumed: &HashSet<String>,
) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut entries = consumed.iter().cloned().collect::<Vec<_>>();
    entries.sort();
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)?;
    for entry in entries {
        writeln!(file, "{entry}")?;
    }
    Ok(())
}

async fn successful_login(config: &AuthConfig, input: SuccessfulLoginInput<'_>) -> Response {
    let now = unix_now_seconds();
    let expires_at_epoch_seconds = now + LOCAL_ACCESS_TOKEN_TTL_SECONDS;
    let scope = if input.remember {
        "remembered"
    } else {
        "session"
    };

    let session_token = config
        .session_signer
        .as_ref()
        .and_then(|signer| {
            signer.sign_account_session(
                input.account,
                now,
                expires_at_epoch_seconds,
                input.mfa_at_epoch_seconds,
                input.mfa_amr_method,
            )
        })
        .unwrap_or_else(|| {
            format!(
                "local-user-service:{scope}:{}:{expires_at_epoch_seconds}",
                input.account.user_id
            )
        });

    record_login_success(
        config,
        input.headers,
        input.remote_addr,
        input.login_surface,
        input.account,
        input.auth_method,
    )
    .await;

    Json(LoginResponse {
        user_id: input.account.user_id.clone(),
        role: input.account.role.clone(),
        affiliate_id: input.account.affiliate_id.clone(),
        mfa_at_epoch_seconds: input.mfa_at_epoch_seconds,
        session_token,
        expires_at_epoch_seconds,
    })
    .into_response()
}

async fn record_login_success(
    config: &AuthConfig,
    headers: &HeaderMap,
    remote_addr: Option<SocketAddr>,
    login_surface: &str,
    account: &LocalLoginAccount,
    auth_method: &str,
) {
    let client_ip = client_ip_from_request(
        headers,
        remote_addr.map(|addr| addr.ip()),
        &config.trusted_proxy_config,
    );
    record_ip_spoof_metric(config, &client_ip);
    let proxy_headers_trusted = client_ip.trusted_proxy_header_used
        || config
            .trusted_proxy_config
            .is_trusted_remote(remote_addr.map(|addr| addr.ip()));
    let event = LoginEventInput {
        login_surface: login_surface.to_owned(),
        principal_type: principal_type_from_role(&account.role),
        principal_id: Some(account.user_id.clone()),
        user_id: matches!(
            principal_type_from_role(&account.role),
            crate::login_events::LoginPrincipalType::User
        )
        .then(|| account.user_id.clone()),
        principal_external_id: account.affiliate_id.clone(),
        account_identifier_hash: None,
        account_identifier_hash_key_version: None,
        event_type: LoginEventType::Success,
        client_ip,
        user_agent: parse_user_agent(headers),
        geo: geo_from_headers(headers, proxy_headers_trusted),
        session_id: None,
        auth_method: auth_method.to_owned(),
        failure_reason: None,
        risk_flags: json!([]),
        request_id: header_value(headers, "x-request-id"),
        correlation_id: header_value(headers, "x-correlation-id"),
    };
    write_login_event(config, event).await;
}

async fn record_login_failure_for_account(
    config: &AuthConfig,
    headers: &HeaderMap,
    remote_addr: Option<SocketAddr>,
    login_surface: &str,
    account: &LocalLoginAccount,
    failure_reason: &str,
    auth_method: &str,
) {
    let client_ip = client_ip_from_request(
        headers,
        remote_addr.map(|addr| addr.ip()),
        &config.trusted_proxy_config,
    );
    record_ip_spoof_metric(config, &client_ip);
    let proxy_headers_trusted = client_ip.trusted_proxy_header_used
        || config
            .trusted_proxy_config
            .is_trusted_remote(remote_addr.map(|addr| addr.ip()));
    let event = LoginEventInput {
        login_surface: login_surface.to_owned(),
        principal_type: principal_type_from_role(&account.role),
        principal_id: Some(account.user_id.clone()),
        user_id: matches!(
            principal_type_from_role(&account.role),
            crate::login_events::LoginPrincipalType::User
        )
        .then(|| account.user_id.clone()),
        principal_external_id: account.affiliate_id.clone(),
        account_identifier_hash: None,
        account_identifier_hash_key_version: None,
        event_type: LoginEventType::Failure,
        client_ip,
        user_agent: parse_user_agent(headers),
        geo: geo_from_headers(headers, proxy_headers_trusted),
        session_id: None,
        auth_method: auth_method.to_owned(),
        failure_reason: Some(failure_reason.to_owned()),
        risk_flags: json!([]),
        request_id: header_value(headers, "x-request-id"),
        correlation_id: header_value(headers, "x-correlation-id"),
    };
    write_login_event(config, event).await;
}

async fn record_login_failure(
    config: &AuthConfig,
    headers: &HeaderMap,
    remote_addr: Option<SocketAddr>,
    login_surface: &str,
    identifier: Option<&LoginIdentifier>,
    failure_reason: &str,
    auth_method: &str,
) {
    let (hash, key_version) = identifier.map_or((None, None), |identifier| {
        let (hash, key_version) = identifier_hmac(
            identifier.as_normalized_str(),
            config.login_identifier_hmac_key.as_slice(),
            &config.login_identifier_hmac_key_version,
        );
        (Some(hash), Some(key_version))
    });
    let client_ip = client_ip_from_request(
        headers,
        remote_addr.map(|addr| addr.ip()),
        &config.trusted_proxy_config,
    );
    record_ip_spoof_metric(config, &client_ip);
    let proxy_headers_trusted = client_ip.trusted_proxy_header_used
        || config
            .trusted_proxy_config
            .is_trusted_remote(remote_addr.map(|addr| addr.ip()));
    let event = LoginEventInput {
        login_surface: login_surface.to_owned(),
        principal_type: principal_type_from_surface(login_surface),
        principal_id: None,
        user_id: None,
        principal_external_id: None,
        account_identifier_hash: hash,
        account_identifier_hash_key_version: key_version,
        event_type: LoginEventType::Failure,
        client_ip,
        user_agent: parse_user_agent(headers),
        geo: geo_from_headers(headers, proxy_headers_trusted),
        session_id: None,
        auth_method: auth_method.to_owned(),
        failure_reason: Some(failure_reason.to_owned()),
        risk_flags: json!([]),
        request_id: header_value(headers, "x-request-id"),
        correlation_id: header_value(headers, "x-correlation-id"),
    };
    write_login_event(config, event).await;
}

async fn write_login_event(config: &AuthConfig, event: LoginEventInput) {
    if let Some(store) = config.login_event_store.as_ref() {
        if store.write_event(event).await.is_ok() {
            config
                .login_event_metrics
                .written
                .fetch_add(1, Ordering::Relaxed);
        } else {
            config
                .login_event_metrics
                .failed
                .fetch_add(1, Ordering::Relaxed);
            tracing::error!("login event write failed");
        }
    }
}

fn record_ip_spoof_metric(config: &AuthConfig, client_ip: &crate::login_events::ClientIp) {
    if client_ip.spoofed_header_ignored {
        config
            .login_event_metrics
            .spoofed_headers_ignored
            .fetch_add(1, Ordering::Relaxed);
    }
}

fn header_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)?
        .to_str()
        .ok()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

struct LocalSessionToken {
    user_id: String,
    expires_at_epoch_seconds: u64,
    mfa_at_epoch_seconds: Option<u64>,
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("authorization")?
        .to_str()
        .ok()?
        .trim()
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn parse_local_session_token(token: &str) -> Option<LocalSessionToken> {
    let rest = token.strip_prefix("local-user-service:")?;
    let (_scope, user_and_expiry) = rest.split_once(':')?;
    let (user_id, expiry) = user_and_expiry.rsplit_once(':')?;
    let expires_at_epoch_seconds = expiry.parse::<u64>().ok()?;
    if user_id.trim().is_empty() {
        return None;
    }
    Some(LocalSessionToken {
        user_id: user_id.to_owned(),
        expires_at_epoch_seconds,
        mfa_at_epoch_seconds: None,
    })
}

fn parse_session_token(config: &AuthConfig, token: &str) -> Option<LocalSessionToken> {
    if token.split('.').count() == 3 {
        return config
            .session_signer
            .as_ref()
            .and_then(|signer| signer.verify_session_token(token));
    }
    parse_local_session_token(token)
}

impl SessionSigner {
    fn from_private_jwk(raw: &str) -> Option<Self> {
        let jwk: Value = serde_json::from_str(raw).ok()?;
        if jwk.get("kty")?.as_str()? != "OKP" || jwk.get("crv")?.as_str()? != "Ed25519" {
            return None;
        }
        let seed = base64_url_decode(jwk.get("d")?.as_str()?)?;
        let seed: [u8; 32] = seed.try_into().ok()?;
        let signing_key = SigningKey::from_bytes(&seed);
        let public_x = base64_url_encode(&signing_key.verifying_key().to_bytes());
        let kid = jwk
            .get("kid")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("user-service-session-ed25519")
            .to_owned();
        Some(Self {
            kid,
            signing_key,
            public_x,
        })
    }

    fn sign_account_session(
        &self,
        account: &LocalLoginAccount,
        issued_at: u64,
        expires_at_epoch_seconds: u64,
        mfa_at_epoch_seconds: Option<u64>,
        mfa_amr_method: Option<&'static str>,
    ) -> Option<String> {
        let header = json!({
            "alg": "EdDSA",
            "kid": self.kid,
            "typ": "JWT"
        });
        let mut payload = json!({
            "sub": account.user_id,
            "role": account.role,
            "iat": issued_at,
            "exp": expires_at_epoch_seconds
        });
        if let Some(affiliate_id) = &account.affiliate_id {
            payload["affiliateId"] = json!(affiliate_id);
        }
        if let Some(mfa_at) = mfa_at_epoch_seconds {
            payload["mfa_at"] = json!(mfa_at);
            payload["amr"] = json!(LocalLoginAccount::session_amr_with_method(mfa_amr_method));
        }
        let header = base64_url_encode(serde_json::to_string(&header).ok()?.as_bytes());
        let payload = base64_url_encode(serde_json::to_string(&payload).ok()?.as_bytes());
        let signing_input = format!("{header}.{payload}");
        let signature = self.signing_key.sign(signing_input.as_bytes());
        Some(format!(
            "{signing_input}.{}",
            base64_url_encode(&signature.to_bytes())
        ))
    }

    fn verify_session_token(&self, token: &str) -> Option<LocalSessionToken> {
        let mut parts = token.split('.');
        let header = parts.next()?;
        let payload = parts.next()?;
        let signature = parts.next()?;
        if parts.next().is_some() {
            return None;
        }
        let header_json: Value = serde_json::from_slice(&base64_url_decode(header)?).ok()?;
        if header_json.get("alg")?.as_str()? != "EdDSA" {
            return None;
        }
        if header_json
            .get("kid")
            .and_then(Value::as_str)
            .is_some_and(|kid| kid != self.kid)
        {
            return None;
        }
        let verifying_key = VerifyingKey::from(&self.signing_key);
        let signature_bytes: [u8; 64] = base64_url_decode(signature)?.try_into().ok()?;
        let signature = ed25519_dalek::Signature::from_bytes(&signature_bytes);
        verifying_key
            .verify(format!("{header}.{payload}").as_bytes(), &signature)
            .ok()?;
        let payload: Value = serde_json::from_slice(&base64_url_decode(payload)?).ok()?;
        let user_id = payload.get("sub")?.as_str()?.to_owned();
        let expires_at_epoch_seconds = payload.get("exp")?.as_u64()?;
        let mfa_at_epoch_seconds = payload.get("mfa_at").and_then(Value::as_u64);
        Some(LocalSessionToken {
            user_id,
            expires_at_epoch_seconds,
            mfa_at_epoch_seconds,
        })
    }
}

fn base64_url_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity((bytes.len() * 4).div_ceil(3));
    let mut index = 0;
    while index < bytes.len() {
        let b0 = bytes[index];
        let b1 = bytes.get(index + 1).copied();
        let b2 = bytes.get(index + 2).copied();
        out.push(TABLE[(b0 >> 2) as usize] as char);
        out.push(
            TABLE[(((b0 & 0b0000_0011) << 4) | (b1.unwrap_or_default() >> 4)) as usize] as char,
        );
        if let Some(b1) = b1 {
            out.push(
                TABLE[(((b1 & 0b0000_1111) << 2) | (b2.unwrap_or_default() >> 6)) as usize] as char,
            );
        }
        if let Some(b2) = b2 {
            out.push(TABLE[(b2 & 0b0011_1111) as usize] as char);
        }
        index += 3;
    }
    out
}

fn base64_url_decode(value: &str) -> Option<Vec<u8>> {
    let mut bits = 0_u32;
    let mut bit_count = 0_u8;
    let mut out = Vec::new();
    for byte in value.bytes() {
        let six_bits = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'-' => 62,
            b'_' => 63,
            b'=' => continue,
            _ => return None,
        };
        bits = (bits << 6) | u32::from(six_bits);
        bit_count += 6;
        if bit_count >= 8 {
            bit_count -= 8;
            out.push(((bits >> bit_count) & 0xff) as u8);
        }
    }
    Some(out)
}

fn invalid_credentials() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({ "error": "ERROR_CODE_INVALID_CREDENTIALS" })),
    )
        .into_response()
}

fn normalize_identifier(value: &str) -> Option<LoginIdentifier> {
    if value.trim().contains('@') {
        return normalize_email(value).map(LoginIdentifier::Email);
    }

    normalize_phone(value).map(LoginIdentifier::Phone)
}

fn normalize_email(value: &str) -> Option<String> {
    let email = value.trim().to_ascii_lowercase();
    let (local, domain) = email.split_once('@')?;

    if local.is_empty()
        || domain.is_empty()
        || !domain.contains('.')
        || domain.starts_with('.')
        || domain.ends_with('.')
        || email.chars().any(char::is_whitespace)
    {
        return None;
    }

    Some(email)
}

fn normalize_phone(value: &str) -> Option<String> {
    let compact = value
        .trim()
        .chars()
        .filter(|character| !matches!(character, ' ' | '\t' | '\n' | '\r' | '(' | ')' | '.' | '-'))
        .collect::<String>();
    let digits = compact.strip_prefix('+').unwrap_or(&compact);

    if digits.is_empty() || !digits.chars().all(|character| character.is_ascii_digit()) {
        return None;
    }

    let subscriber = digits
        .strip_prefix("886")
        .or_else(|| digits.strip_prefix('0'))
        .unwrap_or(digits);

    if subscriber.len() == 9
        && subscriber.starts_with('9')
        && subscriber
            .chars()
            .all(|character| character.is_ascii_digit())
    {
        Some(format!("+886{subscriber}"))
    } else {
        None
    }
}

fn identifier_kind_matches(kind: &str, identifier: &LoginIdentifier) -> bool {
    matches!(
        (kind, identifier),
        ("", _) | ("email", LoginIdentifier::Email(_)) | ("phone", LoginIdentifier::Phone(_))
    )
}

fn unix_now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}
