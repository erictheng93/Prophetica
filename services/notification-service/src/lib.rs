use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{PgPool, Row};

pub const SERVICE_NAME: &str = "notification-service";
pub const DEFAULT_PORT: u16 = 8093;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationChannel {
    Email,
    InApp,
}

impl NotificationChannel {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Email => "email",
            Self::InApp => "in_app",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NotificationReminderRequest {
    pub dedupe_key: String,
    pub reminder_type: String,
    pub target_user_id: Option<String>,
    pub action_id: Option<String>,
    pub title: String,
    pub body: String,
    pub channels: Vec<NotificationChannel>,
    pub recipients: Vec<NotificationRecipient>,
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationRecipient {
    pub recipient_role: String,
    pub recipient_user_id: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NotificationMessage {
    pub notification_id: String,
    pub dedupe_key: String,
    pub reminder_type: String,
    pub recipient_role: String,
    pub recipient_user_id: Option<String>,
    pub channel: NotificationChannel,
    pub delivery_address: Option<String>,
    pub subject: String,
    pub body: String,
    pub status: String,
    pub target_user_id: Option<String>,
    pub action_id: Option<String>,
    pub metadata: Value,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NotificationReminderResponse {
    pub messages: Vec<NotificationMessage>,
    pub deduped: bool,
    pub source: &'static str,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NotificationMessageListResponse {
    pub messages: Vec<NotificationMessage>,
    pub source: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct NotificationMessageQuery {
    pub recipient_role: Option<String>,
    pub status: Option<String>,
    pub dedupe_key: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationStoreError {
    message: String,
}

impl NotificationStoreError {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

#[async_trait]
pub trait NotificationStore {
    async fn create_reminder(
        &self,
        request: NotificationReminderRequest,
    ) -> Result<NotificationReminderResponse, NotificationStoreError>;

    async fn list_messages(
        &self,
        query: NotificationMessageQuery,
    ) -> Result<NotificationMessageListResponse, NotificationStoreError>;
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryNotificationStore {
    state: Arc<Mutex<InMemoryNotificationState>>,
}

#[derive(Debug, Default)]
struct InMemoryNotificationState {
    next_id: u64,
    messages: Vec<NotificationMessage>,
}

#[async_trait]
impl NotificationStore for InMemoryNotificationStore {
    async fn create_reminder(
        &self,
        request: NotificationReminderRequest,
    ) -> Result<NotificationReminderResponse, NotificationStoreError> {
        let mut state = self
            .state
            .lock()
            .map_err(|error| NotificationStoreError::new(error.to_string()))?;
        let mut messages = Vec::new();
        let mut inserted_any = false;

        for recipient in &request.recipients {
            for channel in &request.channels {
                if let Some(existing) = state.messages.iter().find(|message| {
                    message.dedupe_key == request.dedupe_key
                        && message.recipient_role == recipient.recipient_role
                        && message.recipient_user_id == recipient.recipient_user_id
                        && message.channel == *channel
                }) {
                    messages.push(existing.clone());
                    continue;
                }

                state.next_id += 1;
                let message = notification_message_from_request(
                    format!("notif-{:06}", state.next_id),
                    &request,
                    recipient,
                    *channel,
                    "queued".to_owned(),
                    "in-memory".to_owned(),
                );
                state.messages.push(message.clone());
                messages.push(message);
                inserted_any = true;
            }
        }

        Ok(NotificationReminderResponse {
            messages,
            deduped: !inserted_any,
            source: "notification-service:outbox",
        })
    }

    async fn list_messages(
        &self,
        query: NotificationMessageQuery,
    ) -> Result<NotificationMessageListResponse, NotificationStoreError> {
        let state = self
            .state
            .lock()
            .map_err(|error| NotificationStoreError::new(error.to_string()))?;
        let limit = query.limit.unwrap_or(200).clamp(1, 500);
        let messages = state
            .messages
            .iter()
            .filter(|message| {
                query
                    .recipient_role
                    .as_deref()
                    .is_none_or(|role| message.recipient_role == role)
                    && query
                        .status
                        .as_deref()
                        .is_none_or(|status| message.status == status)
                    && query
                        .dedupe_key
                        .as_deref()
                        .is_none_or(|dedupe_key| message.dedupe_key == dedupe_key)
            })
            .take(limit)
            .cloned()
            .collect();

        Ok(NotificationMessageListResponse {
            messages,
            source: "notification-service:outbox",
        })
    }
}

#[derive(Debug, Clone)]
pub struct PostgresNotificationStore {
    pool: PgPool,
}

impl PostgresNotificationStore {
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl NotificationStore for PostgresNotificationStore {
    async fn create_reminder(
        &self,
        request: NotificationReminderRequest,
    ) -> Result<NotificationReminderResponse, NotificationStoreError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| NotificationStoreError::new(error.to_string()))?;
        let mut messages = Vec::new();
        let mut inserted_any = false;
        for recipient in &request.recipients {
            for channel in &request.channels {
                let recipient_user_id = recipient.recipient_user_id.as_deref().unwrap_or("");
                let delivery_address = delivery_address(recipient, *channel);
                let row = sqlx::query(
                    r"
                    INSERT INTO notification_messages (
                        dedupe_key, reminder_type, recipient_role, recipient_user_id,
                        channel, delivery_address, subject, body, status, target_user_id,
                        action_id, metadata
                    )
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'queued', $9, $10, $11)
                    ON CONFLICT (dedupe_key, recipient_role, recipient_user_id, channel)
                    DO UPDATE SET dedupe_key = notification_messages.dedupe_key
                    RETURNING notification_id, dedupe_key, reminder_type, recipient_role,
                              recipient_user_id, channel, delivery_address, subject, body,
                              status, target_user_id, action_id, metadata,
                              created_at::text AS created_at, (xmax = 0) AS inserted
                    ",
                )
                .bind(&request.dedupe_key)
                .bind(&request.reminder_type)
                .bind(&recipient.recipient_role)
                .bind(recipient_user_id)
                .bind(channel.as_str())
                .bind(delivery_address)
                .bind(&request.title)
                .bind(&request.body)
                .bind(&request.target_user_id)
                .bind(&request.action_id)
                .bind(reminder_metadata(&request))
                .fetch_one(&mut *tx)
                .await
                .map_err(|error| NotificationStoreError::new(error.to_string()))?;

                let inserted: bool = row
                    .try_get("inserted")
                    .map_err(|error| NotificationStoreError::new(error.to_string()))?;
                inserted_any |= inserted;
                messages.push(notification_message_from_row(&row)?);
            }
        }
        tx.commit()
            .await
            .map_err(|error| NotificationStoreError::new(error.to_string()))?;

        Ok(NotificationReminderResponse {
            messages,
            deduped: !inserted_any,
            source: "notification-service:outbox",
        })
    }

    async fn list_messages(
        &self,
        query: NotificationMessageQuery,
    ) -> Result<NotificationMessageListResponse, NotificationStoreError> {
        let limit = i64::try_from(query.limit.unwrap_or(200).clamp(1, 500))
            .map_err(|error| NotificationStoreError::new(error.to_string()))?;
        let rows = sqlx::query(
            r"
            SELECT notification_id, dedupe_key, reminder_type, recipient_role,
                   recipient_user_id, channel, delivery_address, subject, body,
                   status, target_user_id, action_id, metadata, created_at::text AS created_at
            FROM notification_messages
            WHERE ($1::TEXT IS NULL OR recipient_role = $1)
              AND ($2::TEXT IS NULL OR status = $2)
              AND ($3::TEXT IS NULL OR dedupe_key = $3)
            ORDER BY created_at DESC, id DESC
            LIMIT $4
            ",
        )
        .bind(query.recipient_role.as_deref())
        .bind(query.status.as_deref())
        .bind(query.dedupe_key.as_deref())
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| NotificationStoreError::new(error.to_string()))?;

        Ok(NotificationMessageListResponse {
            messages: rows
                .iter()
                .map(notification_message_from_row)
                .collect::<Result<Vec<_>, _>>()?,
            source: "notification-service:outbox",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ErrorResponse {
    error: &'static str,
}

pub fn router<S>(store: S) -> Router
where
    S: NotificationStore + Clone + Send + Sync + 'static,
{
    Router::new()
        .route(
            "/internal/notifications/reminders",
            post(create_reminder::<S>),
        )
        .route("/internal/notifications/messages", get(list_messages::<S>))
        .with_state(store)
}

async fn create_reminder<S>(
    State(store): State<S>,
    Json(request): Json<NotificationReminderRequest>,
) -> Result<(StatusCode, Json<NotificationReminderResponse>), (StatusCode, Json<ErrorResponse>)>
where
    S: NotificationStore,
{
    validate_reminder_request(&request)?;
    let response = store
        .create_reminder(request)
        .await
        .map_err(|_| internal_error())?;
    let status = if response.deduped {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };
    Ok((status, Json(response)))
}

async fn list_messages<S>(
    State(store): State<S>,
    Query(query): Query<NotificationMessageQuery>,
) -> Result<Json<NotificationMessageListResponse>, (StatusCode, Json<ErrorResponse>)>
where
    S: NotificationStore,
{
    store
        .list_messages(query)
        .await
        .map(Json)
        .map_err(|_| internal_error())
}

fn validate_reminder_request(
    request: &NotificationReminderRequest,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    let reminder_type = request.reminder_type.trim();
    if request.dedupe_key.trim().is_empty()
        || !matches!(reminder_type, "pending_case" | "approval_required")
        || request.title.trim().is_empty()
        || request.body.trim().is_empty()
        || request.channels.is_empty()
        || request.recipients.is_empty()
    {
        return Err(invalid_reminder());
    }

    let sends_email = request.channels.contains(&NotificationChannel::Email);
    for recipient in &request.recipients {
        if recipient.recipient_role.trim().is_empty()
            || (sends_email
                && recipient
                    .email
                    .as_deref()
                    .is_none_or(|email| email.trim().is_empty()))
        {
            return Err(invalid_reminder());
        }
    }
    Ok(())
}

fn invalid_reminder() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: "ERROR_CODE_INVALID_NOTIFICATION_REMINDER",
        }),
    )
}

fn internal_error() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            error: "ERROR_CODE_NOTIFICATION_INTERNAL",
        }),
    )
}

fn notification_message_from_request(
    notification_id: String,
    request: &NotificationReminderRequest,
    recipient: &NotificationRecipient,
    channel: NotificationChannel,
    status: String,
    created_at: String,
) -> NotificationMessage {
    NotificationMessage {
        notification_id,
        dedupe_key: request.dedupe_key.clone(),
        reminder_type: request.reminder_type.clone(),
        recipient_role: recipient.recipient_role.clone(),
        recipient_user_id: recipient.recipient_user_id.clone(),
        channel,
        delivery_address: delivery_address(recipient, channel),
        subject: request.title.clone(),
        body: request.body.clone(),
        status,
        target_user_id: request.target_user_id.clone(),
        action_id: request.action_id.clone(),
        metadata: reminder_metadata(request),
        created_at,
    }
}

fn reminder_metadata(request: &NotificationReminderRequest) -> Value {
    request
        .metadata
        .clone()
        .unwrap_or_else(|| Value::Object(serde_json::Map::new()))
}

fn delivery_address(
    recipient: &NotificationRecipient,
    channel: NotificationChannel,
) -> Option<String> {
    match channel {
        NotificationChannel::Email => recipient.email.clone(),
        NotificationChannel::InApp => None,
    }
}

fn notification_message_from_row(
    row: &sqlx::postgres::PgRow,
) -> Result<NotificationMessage, NotificationStoreError> {
    let channel: String = row
        .try_get("channel")
        .map_err(|error| NotificationStoreError::new(error.to_string()))?;
    let channel = match channel.as_str() {
        "email" => NotificationChannel::Email,
        "in_app" => NotificationChannel::InApp,
        _ => return Err(NotificationStoreError::new("unknown notification channel")),
    };
    let recipient_user_id: String = row
        .try_get("recipient_user_id")
        .map_err(|error| NotificationStoreError::new(error.to_string()))?;

    Ok(NotificationMessage {
        notification_id: row
            .try_get("notification_id")
            .map_err(|error| NotificationStoreError::new(error.to_string()))?,
        dedupe_key: row
            .try_get("dedupe_key")
            .map_err(|error| NotificationStoreError::new(error.to_string()))?,
        reminder_type: row
            .try_get("reminder_type")
            .map_err(|error| NotificationStoreError::new(error.to_string()))?,
        recipient_role: row
            .try_get("recipient_role")
            .map_err(|error| NotificationStoreError::new(error.to_string()))?,
        recipient_user_id: if recipient_user_id.is_empty() {
            None
        } else {
            Some(recipient_user_id)
        },
        channel,
        delivery_address: row
            .try_get("delivery_address")
            .map_err(|error| NotificationStoreError::new(error.to_string()))?,
        subject: row
            .try_get("subject")
            .map_err(|error| NotificationStoreError::new(error.to_string()))?,
        body: row
            .try_get("body")
            .map_err(|error| NotificationStoreError::new(error.to_string()))?,
        status: row
            .try_get("status")
            .map_err(|error| NotificationStoreError::new(error.to_string()))?,
        target_user_id: row
            .try_get("target_user_id")
            .map_err(|error| NotificationStoreError::new(error.to_string()))?,
        action_id: row
            .try_get("action_id")
            .map_err(|error| NotificationStoreError::new(error.to_string()))?,
        metadata: row
            .try_get("metadata")
            .map_err(|error| NotificationStoreError::new(error.to_string()))?,
        created_at: row
            .try_get("created_at")
            .map_err(|error| NotificationStoreError::new(error.to_string()))?,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettlementNotificationChannel {
    Email,
    InApp,
    WebSocket,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettlementNotificationReceipt {
    pub user_id: String,
    pub channel: SettlementNotificationChannel,
    pub delivered_within_24h: bool,
}

impl SettlementNotificationReceipt {
    #[must_use]
    pub fn delivered(user_id: impl Into<String>, channel: SettlementNotificationChannel) -> Self {
        Self {
            user_id: user_id.into(),
            channel,
            delivered_within_24h: true,
        }
    }

    #[must_use]
    pub fn missed(user_id: impl Into<String>, channel: SettlementNotificationChannel) -> Self {
        Self {
            user_id: user_id.into(),
            channel,
            delivered_within_24h: false,
        }
    }
}

#[must_use]
pub fn settlement_announcement_delivery_rate_bps(
    receipts: &[SettlementNotificationReceipt],
) -> u32 {
    if receipts.is_empty() {
        return 10_000;
    }

    let delivered = receipts
        .iter()
        .filter(|receipt| receipt.delivered_within_24h)
        .count();
    u32::try_from((delivered * 10_000) / receipts.len()).unwrap_or(10_000)
}

#[must_use]
pub fn meets_settlement_announcement_slo(receipts: &[SettlementNotificationReceipt]) -> bool {
    if receipts.is_empty() {
        return true;
    }

    let delivered = receipts
        .iter()
        .filter(|receipt| receipt.delivered_within_24h)
        .count();
    delivered * 1_000 >= receipts.len() * 999
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{
        NotificationStore, SettlementNotificationChannel, SettlementNotificationReceipt,
        meets_settlement_announcement_slo, settlement_announcement_delivery_rate_bps,
    };
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use sqlx::{
        PgPool,
        postgres::{PgConnectOptions, PgPoolOptions},
    };
    use std::{
        str::FromStr,
        time::{SystemTime, UNIX_EPOCH},
    };
    use tower::ServiceExt;

    #[test]
    fn settlement_announcement_delivery_rate_counts_24h_receipts() {
        let receipts = [
            SettlementNotificationReceipt::delivered(
                "user_01",
                SettlementNotificationChannel::Email,
            ),
            SettlementNotificationReceipt::delivered(
                "user_01",
                SettlementNotificationChannel::InApp,
            ),
            SettlementNotificationReceipt::missed(
                "user_02",
                SettlementNotificationChannel::WebSocket,
            ),
        ];

        assert_eq!(settlement_announcement_delivery_rate_bps(&receipts), 6_666);
    }

    #[test]
    fn settlement_announcement_slo_requires_99_9_percent_delivery() {
        let mut receipts = (0..1_000)
            .map(|i| {
                SettlementNotificationReceipt::delivered(
                    format!("user_{i}"),
                    SettlementNotificationChannel::Email,
                )
            })
            .collect::<Vec<_>>();

        assert!(meets_settlement_announcement_slo(&receipts));

        receipts.push(SettlementNotificationReceipt::missed(
            "late_user",
            SettlementNotificationChannel::Email,
        ));
        receipts.push(SettlementNotificationReceipt::missed(
            "late_user_2",
            SettlementNotificationChannel::Email,
        ));
        assert!(!meets_settlement_announcement_slo(&receipts));
    }

    #[tokio::test]
    async fn reminder_api_records_email_and_in_app_outbox_messages() {
        let app = super::router(super::InMemoryNotificationStore::default());

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/internal/notifications/reminders")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "dedupe_key": "member-action:mem-act-000001:approval-required",
                            "reminder_type": "approval_required",
                            "target_user_id": "MEM-1042",
                            "action_id": "mem-act-000001",
                            "title": "會員工單等待二次審批",
                            "body": "OPS-1042 需要第二位管理員核准。",
                            "channels": ["email", "in_app"],
                            "recipients": [{
                                "recipient_role": "ops_control",
                                "recipient_user_id": "manager-2",
                                "email": "ops-control@prophetica.internal"
                            }],
                            "metadata": {
                                "ticket_id": "OPS-1042"
                            }
                        })
                        .to_string(),
                    ))
                    .expect("request builds"),
            )
            .await
            .expect("request handled");

        assert_eq!(response.status(), StatusCode::CREATED);
        let body = response_json(response).await;
        assert_eq!(body["source"], "notification-service:outbox");
        assert_eq!(body["messages"].as_array().map(Vec::len), Some(2));
        assert_eq!(body["messages"][0]["status"], "queued");
        assert_eq!(body["messages"][0]["channel"], "email");
        assert_eq!(body["messages"][1]["channel"], "in_app");

        let listed = app
            .oneshot(
                Request::builder()
                    .uri(
                        "/internal/notifications/messages?recipient_role=ops_control&status=queued",
                    )
                    .body(Body::empty())
                    .expect("request builds"),
            )
            .await
            .expect("list request handled");
        assert_eq!(listed.status(), StatusCode::OK);
        let listed_body = response_json(listed).await;
        assert_eq!(listed_body["messages"].as_array().map(Vec::len), Some(2));
    }

    #[tokio::test]
    async fn reminder_api_deduplicates_by_recipient_channel_and_dedupe_key() {
        let app = super::router(super::InMemoryNotificationStore::default());
        let payload = json!({
            "dedupe_key": "pending-case:MEM-1042:withdrawals_locked",
            "reminder_type": "pending_case",
            "target_user_id": "MEM-1042",
            "title": "會員待處理案件",
            "body": "MEM-1042 有待處理限制案件。",
            "channels": ["email", "in_app"],
            "recipients": [{
                "recipient_role": "risk_ops",
                "email": "risk-ops@prophetica.internal"
            }],
            "metadata": {
                "restriction_reason": "withdrawals_locked"
            }
        });

        let first = app
            .clone()
            .oneshot(json_post("/internal/notifications/reminders", &payload))
            .await
            .expect("first request handled");
        assert_eq!(first.status(), StatusCode::CREATED);
        assert_eq!(
            response_json(first).await["messages"]
                .as_array()
                .map(Vec::len),
            Some(2)
        );

        let second = app
            .oneshot(json_post("/internal/notifications/reminders", &payload))
            .await
            .expect("second request handled");
        assert_eq!(second.status(), StatusCode::OK);
        let body = response_json(second).await;
        assert_eq!(body["messages"].as_array().map(Vec::len), Some(2));
        assert_eq!(body["deduped"], true);
    }

    #[tokio::test]
    async fn postgres_store_persists_and_deduplicates_reminders() {
        let Some(pool) = postgres_test_pool().await else {
            return;
        };
        let store = super::PostgresNotificationStore::new(pool);
        let request = super::NotificationReminderRequest {
            dedupe_key: format!("test:pending-case:{}", unique_test_id()),
            reminder_type: "pending_case".to_owned(),
            target_user_id: Some("MEM-PG-NOTIFY".to_owned()),
            action_id: None,
            title: "Pending member case requires review".to_owned(),
            body: "MEM-PG-NOTIFY has a pending case.".to_owned(),
            channels: vec![
                super::NotificationChannel::Email,
                super::NotificationChannel::InApp,
            ],
            recipients: vec![super::NotificationRecipient {
                recipient_role: "risk_ops".to_owned(),
                recipient_user_id: None,
                email: Some("risk-ops@prophetica.internal".to_owned()),
            }],
            metadata: Some(json!({ "test": true })),
        };

        let first = store
            .create_reminder(request.clone())
            .await
            .expect("first reminder persists");
        assert_eq!(first.messages.len(), 2);
        assert!(!first.deduped);

        let second = store
            .create_reminder(request)
            .await
            .expect("second reminder dedupes");
        assert_eq!(second.messages.len(), 2);
        assert!(second.deduped);

        let listed = store
            .list_messages(super::NotificationMessageQuery {
                recipient_role: Some("risk_ops".to_owned()),
                status: Some("queued".to_owned()),
                dedupe_key: Some(first.messages[0].dedupe_key.clone()),
                limit: None,
            })
            .await
            .expect("messages list from postgres");
        assert_eq!(listed.messages.len(), 2);
    }

    fn json_post(uri: &str, body: &Value) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .expect("request builds")
    }

    async fn response_json(response: axum::response::Response) -> Value {
        let collected = response
            .into_body()
            .collect()
            .await
            .expect("response body should collect");
        serde_json::from_slice(&collected.to_bytes()).expect("response should decode as json")
    }

    async fn postgres_test_pool() -> Option<PgPool> {
        let database_url = std::env::var("NOTIFICATION_SERVICE_TEST_DATABASE_URL")
            .or_else(|_| std::env::var("USER_SERVICE_TEST_DATABASE_URL"))
            .ok()?;
        let bootstrap_pool = PgPool::connect(&database_url)
            .await
            .expect("test database should be reachable");
        let schema = format!("notification_service_{}", unique_test_id());
        sqlx::query(&format!("CREATE SCHEMA IF NOT EXISTS {schema}"))
            .execute(&bootstrap_pool)
            .await
            .expect("test schema should be created");
        bootstrap_pool.close().await;
        let options = PgConnectOptions::from_str(&database_url)
            .expect("test database URL should parse")
            .options([("search_path", schema.as_str())]);
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("test schema database should be reachable");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("notification-service migrations should run");
        Some(pool)
    }

    fn unique_test_id() -> String {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos()
            .to_string()
    }
}
