use std::{
    net::IpAddr,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use axum::http::HeaderMap;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha1::Sha1;
use sqlx::{PgPool, Row};
use ulid::Ulid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoginPrincipalType {
    User,
    Admin,
    Manager,
    Affiliate,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoginEventType {
    Success,
    Failure,
    SessionRefresh,
}

#[derive(Clone, Debug)]
pub struct LoginEventInput {
    pub login_surface: String,
    pub principal_type: LoginPrincipalType,
    pub principal_id: Option<String>,
    pub user_id: Option<String>,
    pub principal_external_id: Option<String>,
    pub account_identifier_hash: Option<String>,
    pub account_identifier_hash_key_version: Option<String>,
    pub event_type: LoginEventType,
    pub client_ip: ClientIp,
    pub user_agent: ParsedUserAgent,
    pub geo: GeoLookupResult,
    pub session_id: Option<String>,
    pub auth_method: String,
    pub failure_reason: Option<String>,
    pub risk_flags: Value,
    pub request_id: Option<String>,
    pub correlation_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ClientIp {
    pub ip_address: IpAddr,
    pub ip_source: String,
}

#[derive(Clone, Debug)]
pub struct GeoLookupResult {
    pub country: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
    pub source: Option<String>,
    pub accuracy: String,
    pub asn: Option<String>,
    pub isp: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ParsedUserAgent {
    pub raw: String,
    pub browser_name: Option<String>,
    pub browser_version: Option<String>,
    pub os_name: Option<String>,
    pub os_version: Option<String>,
    pub device_type: String,
    pub device_vendor: Option<String>,
    pub device_model: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct LoginEventRecord {
    pub id: String,
    pub login_surface: String,
    pub principal_type: String,
    pub principal_id: Option<String>,
    pub user_id: Option<String>,
    pub principal_external_id: Option<String>,
    pub account_identifier_hash: Option<String>,
    pub account_identifier_hash_key_version: Option<String>,
    pub event_type: String,
    pub occurred_at_epoch_seconds: u64,
    pub ip_address: String,
    pub ip_source: String,
    pub geo_country: Option<String>,
    pub geo_region: Option<String>,
    pub geo_city: Option<String>,
    pub geo_source: Option<String>,
    pub geo_accuracy: String,
    pub asn: Option<String>,
    pub isp: Option<String>,
    pub user_agent_raw: String,
    pub browser_name: Option<String>,
    pub browser_version: Option<String>,
    pub os_name: Option<String>,
    pub os_version: Option<String>,
    pub device_type: String,
    pub device_vendor: Option<String>,
    pub device_model: Option<String>,
    pub session_id: Option<String>,
    pub auth_method: String,
    pub failure_reason: Option<String>,
    pub risk_flags: Value,
    pub request_id: Option<String>,
    pub correlation_id: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum LoginEventStoreError {
    #[error("login event store unavailable")]
    Unavailable,
}

#[async_trait]
pub trait LoginEventStore: Send + Sync {
    async fn write_event(
        &self,
        event: LoginEventInput,
    ) -> Result<LoginEventRecord, LoginEventStoreError>;

    async fn list_for_principal(
        &self,
        principal_id: &str,
        limit: usize,
    ) -> Result<Vec<LoginEventRecord>, LoginEventStoreError>;

    async fn list_recent(
        &self,
        limit: usize,
    ) -> Result<Vec<LoginEventRecord>, LoginEventStoreError>;
}

#[derive(Clone, Default)]
pub struct InMemoryLoginEventStore {
    events: Arc<Mutex<Vec<LoginEventRecord>>>,
}

#[derive(Clone)]
pub struct PostgresLoginEventStore {
    pool: PgPool,
}

impl InMemoryLoginEventStore {
    #[must_use]
    pub fn events(&self) -> Vec<LoginEventRecord> {
        self.events
            .lock()
            .map(|events| events.clone())
            .unwrap_or_default()
    }
}

impl PostgresLoginEventStore {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl LoginEventStore for InMemoryLoginEventStore {
    async fn write_event(
        &self,
        event: LoginEventInput,
    ) -> Result<LoginEventRecord, LoginEventStoreError> {
        let record = event.into_record();
        let mut events = self
            .events
            .lock()
            .map_err(|_| LoginEventStoreError::Unavailable)?;
        events.push(record.clone());
        Ok(record)
    }

    async fn list_for_principal(
        &self,
        principal_id: &str,
        limit: usize,
    ) -> Result<Vec<LoginEventRecord>, LoginEventStoreError> {
        let mut events = self
            .events
            .lock()
            .map_err(|_| LoginEventStoreError::Unavailable)?
            .iter()
            .filter(|event| event.principal_id.as_deref() == Some(principal_id))
            .cloned()
            .collect::<Vec<_>>();
        events.sort_by(|left, right| {
            right
                .occurred_at_epoch_seconds
                .cmp(&left.occurred_at_epoch_seconds)
        });
        events.truncate(limit);
        Ok(events)
    }

    async fn list_recent(
        &self,
        limit: usize,
    ) -> Result<Vec<LoginEventRecord>, LoginEventStoreError> {
        let mut events = self
            .events
            .lock()
            .map_err(|_| LoginEventStoreError::Unavailable)?
            .clone();
        events.sort_by(|left, right| {
            right
                .occurred_at_epoch_seconds
                .cmp(&left.occurred_at_epoch_seconds)
        });
        events.truncate(limit);
        Ok(events)
    }
}

#[async_trait]
impl LoginEventStore for PostgresLoginEventStore {
    async fn write_event(
        &self,
        event: LoginEventInput,
    ) -> Result<LoginEventRecord, LoginEventStoreError> {
        let record = event.into_record();
        sqlx::query(
            "
            INSERT INTO login_events (
                id, login_surface, principal_type, principal_id, user_id,
                principal_external_id, account_identifier_hash,
                account_identifier_hash_key_version, event_type, ip_address,
                ip_source, geo_country, geo_region, geo_city, geo_source,
                geo_accuracy, asn, isp, user_agent_raw, browser_name,
                browser_version, os_name, os_version, device_type, device_vendor,
                device_model, session_id, auth_method, failure_reason, risk_flags,
                request_id, correlation_id
            )
            VALUES (
                $1, $2, $3::login_principal_type, $4, $5, $6, $7, $8,
                $9::login_event_type, $10::inet, $11, $12, $13, $14, $15,
                $16::login_geo_accuracy, $17, $18, $19, $20, $21, $22, $23,
                $24::login_device_type, $25, $26, $27, $28, $29, $30, $31, $32
            )
            ",
        )
        .bind(&record.id)
        .bind(&record.login_surface)
        .bind(&record.principal_type)
        .bind(&record.principal_id)
        .bind(&record.user_id)
        .bind(&record.principal_external_id)
        .bind(&record.account_identifier_hash)
        .bind(&record.account_identifier_hash_key_version)
        .bind(&record.event_type)
        .bind(&record.ip_address)
        .bind(&record.ip_source)
        .bind(&record.geo_country)
        .bind(&record.geo_region)
        .bind(&record.geo_city)
        .bind(&record.geo_source)
        .bind(&record.geo_accuracy)
        .bind(&record.asn)
        .bind(&record.isp)
        .bind(&record.user_agent_raw)
        .bind(&record.browser_name)
        .bind(&record.browser_version)
        .bind(&record.os_name)
        .bind(&record.os_version)
        .bind(&record.device_type)
        .bind(&record.device_vendor)
        .bind(&record.device_model)
        .bind(&record.session_id)
        .bind(&record.auth_method)
        .bind(&record.failure_reason)
        .bind(&record.risk_flags)
        .bind(&record.request_id)
        .bind(&record.correlation_id)
        .execute(&self.pool)
        .await
        .map_err(|_| LoginEventStoreError::Unavailable)?;
        Ok(record)
    }

    async fn list_for_principal(
        &self,
        principal_id: &str,
        limit: usize,
    ) -> Result<Vec<LoginEventRecord>, LoginEventStoreError> {
        let rows = sqlx::query(
            "
            SELECT id, login_surface, principal_type::text AS principal_type,
                   principal_id, user_id, principal_external_id,
                   account_identifier_hash, account_identifier_hash_key_version,
                   event_type::text AS event_type,
                   EXTRACT(EPOCH FROM occurred_at)::bigint AS occurred_at_epoch_seconds,
                   ip_address::text AS ip_address, ip_source, geo_country,
                   geo_region, geo_city, geo_source, geo_accuracy::text AS geo_accuracy,
                   asn, isp, user_agent_raw, browser_name, browser_version,
                   os_name, os_version, device_type::text AS device_type,
                   device_vendor, device_model, session_id, auth_method,
                   failure_reason, risk_flags, request_id, correlation_id
            FROM login_events
            WHERE principal_id = $1
            ORDER BY occurred_at DESC
            LIMIT $2
            ",
        )
        .bind(principal_id)
        .bind(i64::try_from(limit).unwrap_or(50))
        .fetch_all(&self.pool)
        .await
        .map_err(|_| LoginEventStoreError::Unavailable)?;

        rows.iter().map(row_to_record).collect()
    }

    async fn list_recent(
        &self,
        limit: usize,
    ) -> Result<Vec<LoginEventRecord>, LoginEventStoreError> {
        let rows = sqlx::query(
            "
            SELECT id, login_surface, principal_type::text AS principal_type,
                   principal_id, user_id, principal_external_id,
                   account_identifier_hash, account_identifier_hash_key_version,
                   event_type::text AS event_type,
                   EXTRACT(EPOCH FROM occurred_at)::bigint AS occurred_at_epoch_seconds,
                   ip_address::text AS ip_address, ip_source, geo_country,
                   geo_region, geo_city, geo_source, geo_accuracy::text AS geo_accuracy,
                   asn, isp, user_agent_raw, browser_name, browser_version,
                   os_name, os_version, device_type::text AS device_type,
                   device_vendor, device_model, session_id, auth_method,
                   failure_reason, risk_flags, request_id, correlation_id
            FROM login_events
            ORDER BY occurred_at DESC
            LIMIT $1
            ",
        )
        .bind(i64::try_from(limit).unwrap_or(50))
        .fetch_all(&self.pool)
        .await
        .map_err(|_| LoginEventStoreError::Unavailable)?;

        rows.iter().map(row_to_record).collect()
    }
}

impl LoginEventInput {
    fn into_record(self) -> LoginEventRecord {
        LoginEventRecord {
            id: Ulid::new().to_string(),
            login_surface: self.login_surface,
            principal_type: self.principal_type.as_str().to_owned(),
            principal_id: self.principal_id,
            user_id: self.user_id,
            principal_external_id: self.principal_external_id,
            account_identifier_hash: self.account_identifier_hash,
            account_identifier_hash_key_version: self.account_identifier_hash_key_version,
            event_type: self.event_type.as_str().to_owned(),
            occurred_at_epoch_seconds: unix_now_seconds(),
            ip_address: self.client_ip.ip_address.to_string(),
            ip_source: self.client_ip.ip_source,
            geo_country: self.geo.country,
            geo_region: self.geo.region,
            geo_city: self.geo.city,
            geo_source: self.geo.source,
            geo_accuracy: self.geo.accuracy,
            asn: self.geo.asn,
            isp: self.geo.isp,
            user_agent_raw: self.user_agent.raw,
            browser_name: self.user_agent.browser_name,
            browser_version: self.user_agent.browser_version,
            os_name: self.user_agent.os_name,
            os_version: self.user_agent.os_version,
            device_type: self.user_agent.device_type,
            device_vendor: self.user_agent.device_vendor,
            device_model: self.user_agent.device_model,
            session_id: self.session_id,
            auth_method: self.auth_method,
            failure_reason: self.failure_reason,
            risk_flags: self.risk_flags,
            request_id: self.request_id,
            correlation_id: self.correlation_id,
        }
    }
}

impl LoginPrincipalType {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Admin => "admin",
            Self::Manager => "manager",
            Self::Affiliate => "affiliate",
        }
    }
}

impl LoginEventType {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Success => "login_success",
            Self::Failure => "login_failure",
            Self::SessionRefresh => "session_refresh",
        }
    }
}

#[must_use]
pub fn principal_type_from_role(role: &str) -> LoginPrincipalType {
    match role.trim() {
        "admin" => LoginPrincipalType::Admin,
        "manager" => LoginPrincipalType::Manager,
        "affiliate" => LoginPrincipalType::Affiliate,
        _ => LoginPrincipalType::User,
    }
}

#[must_use]
pub fn principal_type_from_surface(surface: &str) -> LoginPrincipalType {
    match surface.trim() {
        "admin_console" => LoginPrincipalType::Admin,
        "affiliate_portal" => LoginPrincipalType::Affiliate,
        _ => LoginPrincipalType::User,
    }
}

#[must_use]
pub fn normalize_login_surface(value: Option<&str>) -> String {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        Some("admin_console" | "affiliate_portal" | "liff" | "user_web") => {
            value.unwrap_or("user_web").trim().to_owned()
        }
        _ => "user_web".to_owned(),
    }
}

#[must_use]
pub fn client_ip_from_headers(headers: &HeaderMap) -> ClientIp {
    header_ip(headers, "cf-connecting-ip")
        .map(|ip| ClientIp {
            ip_address: ip,
            ip_source: "cf_connecting_ip".to_owned(),
        })
        .or_else(|| {
            x_forwarded_for_ip(headers).map(|ip| ClientIp {
                ip_address: ip,
                ip_source: "x_forwarded_for".to_owned(),
            })
        })
        .or_else(|| {
            header_ip(headers, "x-real-ip").map(|ip| ClientIp {
                ip_address: ip,
                ip_source: "x_real_ip".to_owned(),
            })
        })
        .unwrap_or_else(|| ClientIp {
            ip_address: IpAddr::from([127, 0, 0, 1]),
            ip_source: "remote_addr".to_owned(),
        })
}

#[must_use]
pub fn geo_from_headers(headers: &HeaderMap) -> GeoLookupResult {
    let country = header_string(headers, "cf-ipcountry");
    if country.is_some() {
        return GeoLookupResult {
            country,
            region: header_string(headers, "cf-region"),
            city: header_string(headers, "cf-ipcity"),
            source: Some("cloudflare".to_owned()),
            accuracy: "approximate".to_owned(),
            asn: header_string(headers, "cf-asn"),
            isp: None,
        };
    }
    GeoLookupResult {
        country: None,
        region: None,
        city: None,
        source: None,
        accuracy: "unknown".to_owned(),
        asn: None,
        isp: None,
    }
}

#[must_use]
pub fn parse_user_agent(headers: &HeaderMap) -> ParsedUserAgent {
    let raw = header_string(headers, "user-agent").unwrap_or_default();
    let lower = raw.to_ascii_lowercase();
    let device_type = if lower.contains("bot") || lower.contains("crawler") {
        "bot"
    } else if lower.contains("iphone") || lower.contains("android") || lower.contains("mobile") {
        "mobile"
    } else if lower.contains("ipad") || lower.contains("tablet") {
        "tablet"
    } else if raw.trim().is_empty() || raw == "not-a-real-browser" {
        "unknown"
    } else {
        "desktop"
    };
    let browser_name = if lower.contains("chrome/") {
        Some("Chrome".to_owned())
    } else if lower.contains("safari/") {
        Some("Safari".to_owned())
    } else if lower.contains("firefox/") {
        Some("Firefox".to_owned())
    } else {
        None
    };
    let os_name = if lower.contains("windows") {
        Some("Windows".to_owned())
    } else if lower.contains("iphone") || lower.contains("ipad") {
        Some("iOS".to_owned())
    } else if lower.contains("android") {
        Some("Android".to_owned())
    } else if lower.contains("mac os") {
        Some("macOS".to_owned())
    } else {
        None
    };

    ParsedUserAgent {
        raw,
        browser_name,
        browser_version: None,
        os_name,
        os_version: None,
        device_type: device_type.to_owned(),
        device_vendor: None,
        device_model: None,
    }
}

#[must_use]
pub fn identifier_hmac(identifier: &str, key: &[u8], key_version: &str) -> (String, String) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let Ok(mut mac) = Hmac::<Sha1>::new_from_slice(key) else {
        return (String::new(), key_version.to_owned());
    };
    mac.update(identifier.as_bytes());
    let digest = mac.finalize().into_bytes();
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push(char::from(HEX[usize::from(byte >> 4)]));
        out.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    (out, key_version.to_owned())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SelfLoginEventResponse {
    pub id: String,
    pub occurred_at_epoch_seconds: u64,
    pub event_type: String,
    pub login_surface: String,
    pub ip_masked: String,
    pub ip_source: String,
    pub location_label: String,
    pub device_label: String,
    pub browser_name: Option<String>,
    pub auth_method: String,
}

#[must_use]
pub fn to_self_response(event: &LoginEventRecord) -> SelfLoginEventResponse {
    SelfLoginEventResponse {
        id: event.id.clone(),
        occurred_at_epoch_seconds: event.occurred_at_epoch_seconds,
        event_type: event.event_type.clone(),
        login_surface: event.login_surface.clone(),
        ip_masked: mask_ip(&event.ip_address),
        ip_source: event.ip_source.clone(),
        location_label: location_label(event),
        device_label: device_label(event),
        browser_name: event.browser_name.clone(),
        auth_method: event.auth_method.clone(),
    }
}

#[allow(clippy::too_many_lines)]
fn row_to_record(row: &sqlx::postgres::PgRow) -> Result<LoginEventRecord, LoginEventStoreError> {
    let occurred_at = row
        .try_get::<i64, _>("occurred_at_epoch_seconds")
        .map_err(|_| LoginEventStoreError::Unavailable)?;
    Ok(LoginEventRecord {
        id: row
            .try_get("id")
            .map_err(|_| LoginEventStoreError::Unavailable)?,
        login_surface: row
            .try_get("login_surface")
            .map_err(|_| LoginEventStoreError::Unavailable)?,
        principal_type: row
            .try_get("principal_type")
            .map_err(|_| LoginEventStoreError::Unavailable)?,
        principal_id: row
            .try_get("principal_id")
            .map_err(|_| LoginEventStoreError::Unavailable)?,
        user_id: row
            .try_get("user_id")
            .map_err(|_| LoginEventStoreError::Unavailable)?,
        principal_external_id: row
            .try_get("principal_external_id")
            .map_err(|_| LoginEventStoreError::Unavailable)?,
        account_identifier_hash: row
            .try_get("account_identifier_hash")
            .map_err(|_| LoginEventStoreError::Unavailable)?,
        account_identifier_hash_key_version: row
            .try_get("account_identifier_hash_key_version")
            .map_err(|_| LoginEventStoreError::Unavailable)?,
        event_type: row
            .try_get("event_type")
            .map_err(|_| LoginEventStoreError::Unavailable)?,
        occurred_at_epoch_seconds: u64::try_from(occurred_at).unwrap_or_default(),
        ip_address: row
            .try_get("ip_address")
            .map_err(|_| LoginEventStoreError::Unavailable)?,
        ip_source: row
            .try_get("ip_source")
            .map_err(|_| LoginEventStoreError::Unavailable)?,
        geo_country: row
            .try_get("geo_country")
            .map_err(|_| LoginEventStoreError::Unavailable)?,
        geo_region: row
            .try_get("geo_region")
            .map_err(|_| LoginEventStoreError::Unavailable)?,
        geo_city: row
            .try_get("geo_city")
            .map_err(|_| LoginEventStoreError::Unavailable)?,
        geo_source: row
            .try_get("geo_source")
            .map_err(|_| LoginEventStoreError::Unavailable)?,
        geo_accuracy: row
            .try_get("geo_accuracy")
            .map_err(|_| LoginEventStoreError::Unavailable)?,
        asn: row
            .try_get("asn")
            .map_err(|_| LoginEventStoreError::Unavailable)?,
        isp: row
            .try_get("isp")
            .map_err(|_| LoginEventStoreError::Unavailable)?,
        user_agent_raw: row
            .try_get("user_agent_raw")
            .map_err(|_| LoginEventStoreError::Unavailable)?,
        browser_name: row
            .try_get("browser_name")
            .map_err(|_| LoginEventStoreError::Unavailable)?,
        browser_version: row
            .try_get("browser_version")
            .map_err(|_| LoginEventStoreError::Unavailable)?,
        os_name: row
            .try_get("os_name")
            .map_err(|_| LoginEventStoreError::Unavailable)?,
        os_version: row
            .try_get("os_version")
            .map_err(|_| LoginEventStoreError::Unavailable)?,
        device_type: row
            .try_get("device_type")
            .map_err(|_| LoginEventStoreError::Unavailable)?,
        device_vendor: row
            .try_get("device_vendor")
            .map_err(|_| LoginEventStoreError::Unavailable)?,
        device_model: row
            .try_get("device_model")
            .map_err(|_| LoginEventStoreError::Unavailable)?,
        session_id: row
            .try_get("session_id")
            .map_err(|_| LoginEventStoreError::Unavailable)?,
        auth_method: row
            .try_get("auth_method")
            .map_err(|_| LoginEventStoreError::Unavailable)?,
        failure_reason: row
            .try_get("failure_reason")
            .map_err(|_| LoginEventStoreError::Unavailable)?,
        risk_flags: row
            .try_get("risk_flags")
            .map_err(|_| LoginEventStoreError::Unavailable)?,
        request_id: row
            .try_get("request_id")
            .map_err(|_| LoginEventStoreError::Unavailable)?,
        correlation_id: row
            .try_get("correlation_id")
            .map_err(|_| LoginEventStoreError::Unavailable)?,
    })
}

fn mask_ip(value: &str) -> String {
    if let Ok(ip) = value.parse::<IpAddr>() {
        return match ip {
            IpAddr::V4(ip) => {
                let octets = ip.octets();
                format!("{}.{}.{}.xxx", octets[0], octets[1], octets[2])
            }
            IpAddr::V6(ip) => {
                let segments = ip.segments();
                format!(
                    "{:x}:{:x}:{:x}:xxxx::",
                    segments[0], segments[1], segments[2]
                )
            }
        };
    }
    "unknown".to_owned()
}

fn location_label(event: &LoginEventRecord) -> String {
    event
        .geo_city
        .as_ref()
        .or(event.geo_region.as_ref())
        .or(event.geo_country.as_ref())
        .cloned()
        .unwrap_or_else(|| "未知".to_owned())
}

fn device_label(event: &LoginEventRecord) -> String {
    if event.device_type == "unknown" {
        "未知".to_owned()
    } else {
        event.device_type.clone()
    }
}

fn header_ip(headers: &HeaderMap, name: &str) -> Option<IpAddr> {
    header_string(headers, name)?.parse().ok()
}

fn x_forwarded_for_ip(headers: &HeaderMap) -> Option<IpAddr> {
    header_string(headers, "x-forwarded-for")?
        .split(',')
        .map(str::trim)
        .find_map(|candidate| candidate.parse().ok())
}

fn header_string(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)?
        .to_str()
        .ok()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn unix_now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}
