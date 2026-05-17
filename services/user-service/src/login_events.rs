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
use sha2::Sha256;
use sqlx::{PgPool, Postgres, QueryBuilder, Row};
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
    pub trusted_proxy_header_used: bool,
    pub spoofed_header_ignored: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct LoginEventRetentionResult {
    pub dry_run: bool,
    pub cutoff_epoch_seconds: u64,
    pub matched_events: usize,
    pub deleted_events: usize,
}

#[derive(Clone, Debug)]
pub struct TrustedProxyConfig {
    cidrs: Vec<IpCidr>,
}

#[derive(Clone, Debug)]
struct IpCidr {
    network: IpAddr,
    prefix_len: u8,
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

#[derive(Clone, Debug, Default)]
pub struct AdminLoginEventFilter {
    pub limit: usize,
    pub principal_type: Option<String>,
    pub principal_id: Option<String>,
    pub user_id: Option<String>,
    pub ip_address: Option<String>,
    pub event_type: Option<String>,
    pub from_epoch_seconds: Option<u64>,
    pub to_epoch_seconds: Option<u64>,
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

    async fn list_admin(
        &self,
        filter: AdminLoginEventFilter,
    ) -> Result<Vec<LoginEventRecord>, LoginEventStoreError>;

    async fn delete_older_than(
        &self,
        cutoff_epoch_seconds: u64,
        dry_run: bool,
    ) -> Result<LoginEventRetentionResult, LoginEventStoreError>;
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

impl Default for TrustedProxyConfig {
    fn default() -> Self {
        Self {
            cidrs: ["127.0.0.1/32", "::1/128"]
                .into_iter()
                .filter_map(IpCidr::parse)
                .collect(),
        }
    }
}

impl TrustedProxyConfig {
    #[must_use]
    pub fn from_csv(value: &str) -> Self {
        let cidrs = value
            .split(',')
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .filter_map(IpCidr::parse)
            .collect::<Vec<_>>();
        Self { cidrs }
    }

    #[must_use]
    pub fn is_trusted_remote(&self, remote_ip: Option<IpAddr>) -> bool {
        let remote_ip = remote_ip.unwrap_or_else(|| IpAddr::from([127, 0, 0, 1]));
        self.contains(remote_ip)
    }

    fn contains(&self, ip: IpAddr) -> bool {
        self.cidrs.iter().any(|cidr| cidr.contains(ip))
    }
}

impl IpCidr {
    fn parse(value: &str) -> Option<Self> {
        let (ip, prefix_len) = value
            .split_once('/')
            .map_or((value, None), |(ip, prefix)| (ip, Some(prefix)));
        let network = ip.parse::<IpAddr>().ok()?;
        let max_prefix = match network {
            IpAddr::V4(_) => 32,
            IpAddr::V6(_) => 128,
        };
        let prefix_len = prefix_len
            .map(str::parse::<u8>)
            .transpose()
            .ok()?
            .unwrap_or(max_prefix);
        if prefix_len > max_prefix {
            return None;
        }
        Some(Self {
            network,
            prefix_len,
        })
    }

    fn contains(&self, ip: IpAddr) -> bool {
        match (self.network, ip) {
            (IpAddr::V4(network), IpAddr::V4(ip)) => {
                prefix_matches_u32(u32::from(network), u32::from(ip), self.prefix_len)
            }
            (IpAddr::V6(network), IpAddr::V6(ip)) => {
                prefix_matches_u128(u128::from(network), u128::from(ip), self.prefix_len)
            }
            _ => false,
        }
    }
}

fn prefix_matches_u32(network: u32, ip: u32, prefix_len: u8) -> bool {
    if prefix_len == 0 {
        return true;
    }
    let shift = 32_u8.saturating_sub(prefix_len);
    (network >> shift) == (ip >> shift)
}

fn prefix_matches_u128(network: u128, ip: u128, prefix_len: u8) -> bool {
    if prefix_len == 0 {
        return true;
    }
    let shift = 128_u8.saturating_sub(prefix_len);
    (network >> shift) == (ip >> shift)
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

    async fn list_admin(
        &self,
        filter: AdminLoginEventFilter,
    ) -> Result<Vec<LoginEventRecord>, LoginEventStoreError> {
        let mut events = self
            .events
            .lock()
            .map_err(|_| LoginEventStoreError::Unavailable)?
            .iter()
            .filter(|event| admin_filter_matches(event, &filter))
            .cloned()
            .collect::<Vec<_>>();
        events.sort_by(|left, right| {
            right
                .occurred_at_epoch_seconds
                .cmp(&left.occurred_at_epoch_seconds)
        });
        events.truncate(filter.limit.clamp(1, 200));
        Ok(events)
    }

    async fn delete_older_than(
        &self,
        cutoff_epoch_seconds: u64,
        dry_run: bool,
    ) -> Result<LoginEventRetentionResult, LoginEventStoreError> {
        let mut events = self
            .events
            .lock()
            .map_err(|_| LoginEventStoreError::Unavailable)?;
        let matched_events = events
            .iter()
            .filter(|event| event.occurred_at_epoch_seconds < cutoff_epoch_seconds)
            .count();
        if !dry_run {
            events.retain(|event| event.occurred_at_epoch_seconds >= cutoff_epoch_seconds);
        }
        Ok(LoginEventRetentionResult {
            dry_run,
            cutoff_epoch_seconds,
            matched_events,
            deleted_events: if dry_run { 0 } else { matched_events },
        })
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

    async fn list_admin(
        &self,
        filter: AdminLoginEventFilter,
    ) -> Result<Vec<LoginEventRecord>, LoginEventStoreError> {
        let mut query = QueryBuilder::<Postgres>::new(
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
            WHERE 1 = 1
            ",
        );
        if let Some(principal_type) = non_empty_filter(filter.principal_type.as_ref()) {
            query
                .push(" AND principal_type = ")
                .push_bind(principal_type)
                .push("::login_principal_type");
        }
        if let Some(principal_id) = non_empty_filter(filter.principal_id.as_ref()) {
            query.push(" AND principal_id = ").push_bind(principal_id);
        }
        if let Some(user_id) = non_empty_filter(filter.user_id.as_ref()) {
            query.push(" AND user_id = ").push_bind(user_id);
        }
        if let Some(ip_address) = non_empty_filter(filter.ip_address.as_ref()) {
            query
                .push(" AND ip_address = ")
                .push_bind(ip_address)
                .push("::inet");
        }
        if let Some(event_type) = non_empty_filter(filter.event_type.as_ref()) {
            query
                .push(" AND event_type = ")
                .push_bind(event_type)
                .push("::login_event_type");
        }
        if let Some(from_epoch_seconds) = filter.from_epoch_seconds {
            query
                .push(" AND occurred_at >= to_timestamp(")
                .push_bind(i64::try_from(from_epoch_seconds).unwrap_or(i64::MAX))
                .push(")");
        }
        if let Some(to_epoch_seconds) = filter.to_epoch_seconds {
            query
                .push(" AND occurred_at <= to_timestamp(")
                .push_bind(i64::try_from(to_epoch_seconds).unwrap_or(i64::MAX))
                .push(")");
        }
        query.push(" ORDER BY occurred_at DESC LIMIT ");
        query.push_bind(i64::try_from(filter.limit.clamp(1, 200)).unwrap_or(50));

        let rows = query
            .build()
            .fetch_all(&self.pool)
            .await
            .map_err(|_| LoginEventStoreError::Unavailable)?;

        rows.iter().map(row_to_record).collect()
    }

    async fn delete_older_than(
        &self,
        cutoff_epoch_seconds: u64,
        dry_run: bool,
    ) -> Result<LoginEventRetentionResult, LoginEventStoreError> {
        let cutoff = i64::try_from(cutoff_epoch_seconds).unwrap_or(i64::MAX);
        let matched_events = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM login_events WHERE occurred_at < to_timestamp($1)",
        )
        .bind(cutoff)
        .fetch_one(&self.pool)
        .await
        .map_err(|_| LoginEventStoreError::Unavailable)?;

        if !dry_run {
            sqlx::query("DELETE FROM login_events WHERE occurred_at < to_timestamp($1)")
                .bind(cutoff)
                .execute(&self.pool)
                .await
                .map_err(|_| LoginEventStoreError::Unavailable)?;
        }

        let matched_events = usize::try_from(matched_events).unwrap_or_default();
        Ok(LoginEventRetentionResult {
            dry_run,
            cutoff_epoch_seconds,
            matched_events,
            deleted_events: if dry_run { 0 } else { matched_events },
        })
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
pub fn client_ip_from_request(
    headers: &HeaderMap,
    remote_ip: Option<IpAddr>,
    trusted_proxies: &TrustedProxyConfig,
) -> ClientIp {
    let remote_ip = remote_ip.unwrap_or_else(|| IpAddr::from([127, 0, 0, 1]));
    if !trusted_proxies.contains(remote_ip) {
        return ClientIp {
            ip_address: remote_ip,
            ip_source: "remote_addr".to_owned(),
            trusted_proxy_header_used: false,
            spoofed_header_ignored: has_forwarded_ip_header(headers),
        };
    }

    header_ip(headers, "cf-connecting-ip")
        .map(|ip| ClientIp {
            ip_address: ip,
            ip_source: "cf_connecting_ip".to_owned(),
            trusted_proxy_header_used: true,
            spoofed_header_ignored: false,
        })
        .or_else(|| {
            x_forwarded_for_ip(headers, trusted_proxies).map(|ip| ClientIp {
                ip_address: ip,
                ip_source: "x_forwarded_for".to_owned(),
                trusted_proxy_header_used: true,
                spoofed_header_ignored: false,
            })
        })
        .or_else(|| {
            header_ip(headers, "x-real-ip").map(|ip| ClientIp {
                ip_address: ip,
                ip_source: "x_real_ip".to_owned(),
                trusted_proxy_header_used: true,
                spoofed_header_ignored: false,
            })
        })
        .unwrap_or(ClientIp {
            ip_address: remote_ip,
            ip_source: "remote_addr".to_owned(),
            trusted_proxy_header_used: false,
            spoofed_header_ignored: false,
        })
}

#[must_use]
pub fn geo_from_headers(headers: &HeaderMap, proxy_headers_trusted: bool) -> GeoLookupResult {
    if !proxy_headers_trusted {
        return unknown_geo();
    }
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
    unknown_geo()
}

#[must_use]
pub fn unknown_geo() -> GeoLookupResult {
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
    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(key) else {
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

fn admin_filter_matches(event: &LoginEventRecord, filter: &AdminLoginEventFilter) -> bool {
    field_matches(&event.principal_type, filter.principal_type.as_ref())
        && option_field_matches(event.principal_id.as_ref(), filter.principal_id.as_ref())
        && option_field_matches(event.user_id.as_ref(), filter.user_id.as_ref())
        && field_matches(&event.ip_address, filter.ip_address.as_ref())
        && field_matches(&event.event_type, filter.event_type.as_ref())
        && filter
            .from_epoch_seconds
            .is_none_or(|from| event.occurred_at_epoch_seconds >= from)
        && filter
            .to_epoch_seconds
            .is_none_or(|to| event.occurred_at_epoch_seconds <= to)
}

fn field_matches(value: &str, filter: Option<&String>) -> bool {
    non_empty_filter(filter).is_none_or(|expected| value == expected)
}

fn option_field_matches(value: Option<&String>, filter: Option<&String>) -> bool {
    non_empty_filter(filter).is_none_or(|expected| value.map(String::as_str) == Some(expected))
}

fn non_empty_filter(value: Option<&String>) -> Option<&str> {
    value
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn header_ip(headers: &HeaderMap, name: &str) -> Option<IpAddr> {
    header_string(headers, name)?.parse().ok()
}

fn has_forwarded_ip_header(headers: &HeaderMap) -> bool {
    ["cf-connecting-ip", "x-forwarded-for", "x-real-ip"]
        .into_iter()
        .any(|name| header_string(headers, name).is_some())
}

fn x_forwarded_for_ip(headers: &HeaderMap, trusted_proxies: &TrustedProxyConfig) -> Option<IpAddr> {
    header_string(headers, "x-forwarded-for")?
        .split(',')
        .map(str::trim)
        .filter_map(|candidate| candidate.parse().ok())
        .find(|ip| !trusted_proxies.contains(*ip))
        .or_else(|| {
            header_string(headers, "x-forwarded-for")?
                .split(',')
                .map(str::trim)
                .find_map(|candidate| candidate.parse().ok())
        })
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

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};

    use axum::http::{HeaderMap, HeaderValue};

    use super::{TrustedProxyConfig, client_ip_from_request, geo_from_headers, identifier_hmac};

    #[test]
    fn resolver_ignores_spoofed_headers_from_untrusted_remote() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", HeaderValue::from_static("198.51.100.44"));
        headers.insert("cf-connecting-ip", HeaderValue::from_static("203.0.113.44"));
        headers.insert("cf-ipcountry", HeaderValue::from_static("TW"));
        let trusted = TrustedProxyConfig::from_csv("10.0.0.0/8");

        let client_ip = client_ip_from_request(
            &headers,
            Some(IpAddr::V4(Ipv4Addr::new(203, 0, 113, 10))),
            &trusted,
        );
        let geo = geo_from_headers(&headers, client_ip.trusted_proxy_header_used);

        assert_eq!(client_ip.ip_address.to_string(), "203.0.113.10");
        assert_eq!(client_ip.ip_source, "remote_addr");
        assert_eq!(geo.accuracy, "unknown");
        assert!(geo.country.is_none());
    }

    #[test]
    fn resolver_prefers_cloudflare_then_xff_then_x_real_ip_from_trusted_remote() {
        let trusted = TrustedProxyConfig::from_csv("10.0.0.0/8, 127.0.0.1/32");
        let remote_ip = Some(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 9)));
        let mut cloudflare_headers = HeaderMap::new();
        cloudflare_headers.insert("cf-connecting-ip", HeaderValue::from_static("203.0.113.45"));
        let cloudflare = client_ip_from_request(&cloudflare_headers, remote_ip, &trusted);
        assert_eq!(cloudflare.ip_address.to_string(), "203.0.113.45");
        assert_eq!(cloudflare.ip_source, "cf_connecting_ip");

        let mut xff_headers = HeaderMap::new();
        xff_headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("198.51.100.22, 10.0.0.5"),
        );
        let xff = client_ip_from_request(&xff_headers, remote_ip, &trusted);
        assert_eq!(xff.ip_address.to_string(), "198.51.100.22");
        assert_eq!(xff.ip_source, "x_forwarded_for");

        let mut real_ip_headers = HeaderMap::new();
        real_ip_headers.insert("x-real-ip", HeaderValue::from_static("192.0.2.8"));
        let real_ip = client_ip_from_request(&real_ip_headers, remote_ip, &trusted);
        assert_eq!(real_ip.ip_address.to_string(), "192.0.2.8");
        assert_eq!(real_ip.ip_source, "x_real_ip");
    }

    #[test]
    fn identifier_hmac_uses_sha256_length_digest() {
        let (hash, version) = identifier_hmac("member@example.com", b"secret", "v7");

        assert_eq!(hash.len(), 64);
        assert_eq!(version, "v7");
    }
}
