use std::{
    env,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

const LOCAL_ACCESS_TOKEN_TTL_SECONDS: u64 = 60 * 60 * 24;

#[derive(Clone)]
pub struct AuthConfig {
    accounts: Arc<[LocalLoginAccount]>,
}

#[derive(Clone)]
pub struct LocalLoginAccount {
    user_id: String,
    email: String,
    phone: String,
    password: String,
    role: String,
    affiliate_id: Option<String>,
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
}

#[derive(Debug, Serialize)]
struct LoginResponse {
    user_id: String,
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    affiliate_id: Option<String>,
    session_token: String,
    expires_at_epoch_seconds: u64,
}

#[derive(Debug, PartialEq, Eq)]
enum LoginIdentifier {
    Email(String),
    Phone(String),
}

pub fn auth_router() -> Router {
    auth_router_with_config(AuthConfig::from_env())
}

pub fn auth_router_with_config(config: AuthConfig) -> Router {
    Router::new()
        .route("/api/auth/login", post(login))
        .with_state(config)
}

impl AuthConfig {
    #[must_use]
    pub fn new(accounts: Vec<LocalLoginAccount>) -> Self {
        Self {
            accounts: Arc::from(accounts.into_boxed_slice()),
        }
    }

    #[must_use]
    pub fn from_env() -> Self {
        let prophetica_env = env::var("PROPHETICA_ENV").unwrap_or_else(|_| "dev".to_owned());

        if prophetica_env.eq_ignore_ascii_case("production") {
            return Self::new(Vec::new());
        }

        let user_id =
            env::var("USER_SERVICE_LOCAL_LOGIN_USER_ID").unwrap_or_else(|_| "user_42".to_owned());
        let email = env::var("USER_SERVICE_LOCAL_LOGIN_EMAIL")
            .unwrap_or_else(|_| "member@example.com".to_owned());
        let phone =
            env::var("USER_SERVICE_LOCAL_LOGIN_PHONE").unwrap_or_else(|_| "0912345698".to_owned());
        let password =
            env::var("USER_SERVICE_LOCAL_LOGIN_PASSWORD").unwrap_or_else(|_| "member".to_owned());
        let role = env::var("USER_SERVICE_LOCAL_LOGIN_ROLE").unwrap_or_else(|_| "player".to_owned());
        let affiliate_id = env::var("USER_SERVICE_LOCAL_LOGIN_AFFILIATE_ID").ok();

        let accounts = LocalLoginAccount::new(user_id, email, phone, password)
            .map(|account| account.with_profile(role, affiliate_id))
            .map_or_else(Vec::new, |account| vec![account]);

        Self::new(accounts)
    }
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
}

async fn login(State(config): State<AuthConfig>, Json(request): Json<LoginRequest>) -> Response {
    if config.accounts.is_empty() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "error": "ERROR_CODE_AUTH_PROVIDER_UNAVAILABLE" })),
        )
            .into_response();
    }

    let Some(identifier) = normalize_identifier(&request.identifier) else {
        return invalid_credentials();
    };

    if !identifier_kind_matches(&request.identifier_kind, &identifier) {
        return invalid_credentials();
    }

    if request.password.is_empty() {
        return invalid_credentials();
    }

    let account = config.accounts.iter().find(|account| {
        account.matches_identifier(&identifier) && account.password == request.password
    });

    match account {
        Some(account) => successful_login(account, request.remember),
        None => invalid_credentials(),
    }
}

impl LocalLoginAccount {
    fn matches_identifier(&self, identifier: &LoginIdentifier) -> bool {
        match identifier {
            LoginIdentifier::Email(email) => self.email == *email,
            LoginIdentifier::Phone(phone) => self.phone == *phone,
        }
    }
}

fn successful_login(account: &LocalLoginAccount, remember: bool) -> Response {
    let now = unix_now_seconds();
    let expires_at_epoch_seconds = now + LOCAL_ACCESS_TOKEN_TTL_SECONDS;
    let scope = if remember { "remembered" } else { "session" };

    Json(LoginResponse {
        user_id: account.user_id.clone(),
        role: account.role.clone(),
        affiliate_id: account.affiliate_id.clone(),
        session_token: format!(
            "local-user-service:{scope}:{}:{expires_at_epoch_seconds}",
            account.user_id
        ),
        expires_at_epoch_seconds,
    })
    .into_response()
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
