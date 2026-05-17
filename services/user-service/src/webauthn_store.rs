use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{PgPool, Row};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebauthnCeremony {
    Registration,
    Assertion,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WebauthnCredentialRecord {
    pub credential_id: String,
    pub user_id: String,
    pub user_webauthn_uuid: String,
    pub display_name: String,
    pub public_key_cose: Value,
    pub signature_counter: i64,
    pub transports: Value,
    pub user_verified: bool,
    pub backup_eligible: bool,
    pub backup_state: bool,
    pub registration_policy: String,
    pub attestation_format: Option<String>,
    pub attestation: Value,
    pub created_at_epoch_seconds: i64,
    pub last_used_at_epoch_seconds: Option<i64>,
    pub revoked_at_epoch_seconds: Option<i64>,
    pub metadata: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WebauthnChallengeRecord {
    pub challenge_id: String,
    pub user_id: String,
    pub ceremony: WebauthnCeremony,
    pub state_json: Value,
    pub expires_at_epoch_seconds: i64,
    pub consumed_at_epoch_seconds: Option<i64>,
    pub created_at_epoch_seconds: i64,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WebauthnStoreError {
    #[error("webauthn record is invalid")]
    InvalidRecord,
    #[error("webauthn challenge was not found")]
    ChallengeNotFound,
    #[error("webauthn challenge was already consumed")]
    ChallengeConsumed,
    #[error("webauthn challenge expired")]
    ChallengeExpired,
    #[error("webauthn store lock is poisoned")]
    LockPoisoned,
    #[error("webauthn store database error: {0}")]
    Database(String),
}

#[async_trait]
pub trait WebauthnStore: Send + Sync {
    async fn put_challenge(
        &self,
        record: WebauthnChallengeRecord,
    ) -> Result<(), WebauthnStoreError>;

    async fn consume_challenge(
        &self,
        challenge_id: &str,
        now_epoch_seconds: i64,
    ) -> Result<WebauthnChallengeRecord, WebauthnStoreError>;

    async fn put_credential(
        &self,
        record: WebauthnCredentialRecord,
    ) -> Result<(), WebauthnStoreError>;

    async fn list_active_credentials(
        &self,
        user_id: &str,
    ) -> Result<Vec<WebauthnCredentialRecord>, WebauthnStoreError>;

    async fn mark_credential_used(
        &self,
        credential_id: &str,
        signature_counter: i64,
        used_at_epoch_seconds: i64,
    ) -> Result<(), WebauthnStoreError>;

    async fn revoke_credential(
        &self,
        credential_id: &str,
        revoked_at_epoch_seconds: i64,
    ) -> Result<(), WebauthnStoreError>;
}

#[derive(Clone, Default)]
pub struct InMemoryWebauthnStore {
    inner: Arc<Mutex<InMemoryWebauthnState>>,
}

#[derive(Clone)]
pub struct PostgresWebauthnStore {
    pool: PgPool,
}

impl PostgresWebauthnStore {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(Default)]
struct InMemoryWebauthnState {
    challenges: HashMap<String, WebauthnChallengeRecord>,
    credentials: HashMap<String, WebauthnCredentialRecord>,
}

#[async_trait]
impl WebauthnStore for InMemoryWebauthnStore {
    async fn put_challenge(
        &self,
        record: WebauthnChallengeRecord,
    ) -> Result<(), WebauthnStoreError> {
        if record.challenge_id.trim().is_empty()
            || record.user_id.trim().is_empty()
            || record.expires_at_epoch_seconds <= record.created_at_epoch_seconds
        {
            return Err(WebauthnStoreError::InvalidRecord);
        }
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| WebauthnStoreError::LockPoisoned)?;
        inner.challenges.insert(record.challenge_id.clone(), record);
        Ok(())
    }

    async fn consume_challenge(
        &self,
        challenge_id: &str,
        now_epoch_seconds: i64,
    ) -> Result<WebauthnChallengeRecord, WebauthnStoreError> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| WebauthnStoreError::LockPoisoned)?;
        let record = inner
            .challenges
            .get_mut(challenge_id)
            .ok_or(WebauthnStoreError::ChallengeNotFound)?;
        if record.consumed_at_epoch_seconds.is_some() {
            return Err(WebauthnStoreError::ChallengeConsumed);
        }
        if record.expires_at_epoch_seconds <= now_epoch_seconds {
            return Err(WebauthnStoreError::ChallengeExpired);
        }
        record.consumed_at_epoch_seconds = Some(now_epoch_seconds);
        Ok(record.clone())
    }

    async fn put_credential(
        &self,
        record: WebauthnCredentialRecord,
    ) -> Result<(), WebauthnStoreError> {
        if record.credential_id.trim().is_empty()
            || record.user_id.trim().is_empty()
            || record.user_webauthn_uuid.trim().is_empty()
        {
            return Err(WebauthnStoreError::InvalidRecord);
        }
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| WebauthnStoreError::LockPoisoned)?;
        inner
            .credentials
            .insert(record.credential_id.clone(), record);
        Ok(())
    }

    async fn list_active_credentials(
        &self,
        user_id: &str,
    ) -> Result<Vec<WebauthnCredentialRecord>, WebauthnStoreError> {
        let inner = self
            .inner
            .lock()
            .map_err(|_| WebauthnStoreError::LockPoisoned)?;
        Ok(inner
            .credentials
            .values()
            .filter(|record| record.user_id == user_id && record.revoked_at_epoch_seconds.is_none())
            .cloned()
            .collect())
    }

    async fn mark_credential_used(
        &self,
        credential_id: &str,
        signature_counter: i64,
        used_at_epoch_seconds: i64,
    ) -> Result<(), WebauthnStoreError> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| WebauthnStoreError::LockPoisoned)?;
        let record = inner
            .credentials
            .get_mut(credential_id)
            .ok_or(WebauthnStoreError::InvalidRecord)?;
        record.signature_counter = signature_counter;
        record.last_used_at_epoch_seconds = Some(used_at_epoch_seconds);
        Ok(())
    }

    async fn revoke_credential(
        &self,
        credential_id: &str,
        revoked_at_epoch_seconds: i64,
    ) -> Result<(), WebauthnStoreError> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| WebauthnStoreError::LockPoisoned)?;
        let record = inner
            .credentials
            .get_mut(credential_id)
            .ok_or(WebauthnStoreError::InvalidRecord)?;
        record.revoked_at_epoch_seconds = Some(revoked_at_epoch_seconds);
        Ok(())
    }
}

#[async_trait]
impl WebauthnStore for PostgresWebauthnStore {
    async fn put_challenge(
        &self,
        record: WebauthnChallengeRecord,
    ) -> Result<(), WebauthnStoreError> {
        if record.challenge_id.trim().is_empty()
            || record.user_id.trim().is_empty()
            || record.expires_at_epoch_seconds <= record.created_at_epoch_seconds
        {
            return Err(WebauthnStoreError::InvalidRecord);
        }
        let ceremony = ceremony_as_str(&record.ceremony);
        sqlx::query(
            r"
            INSERT INTO user_webauthn_challenges (
                challenge_id,
                user_id,
                ceremony,
                state_json,
                expires_at,
                consumed_at,
                created_at
            )
            VALUES (
                $1,
                $2,
                $3,
                $4,
                to_timestamp($5),
                CASE WHEN $6::BIGINT IS NULL THEN NULL ELSE to_timestamp($6) END,
                to_timestamp($7)
            )
            ON CONFLICT (challenge_id) DO UPDATE SET
                state_json = EXCLUDED.state_json,
                expires_at = EXCLUDED.expires_at,
                consumed_at = EXCLUDED.consumed_at
            ",
        )
        .bind(&record.challenge_id)
        .bind(&record.user_id)
        .bind(ceremony)
        .bind(&record.state_json)
        .bind(record.expires_at_epoch_seconds)
        .bind(record.consumed_at_epoch_seconds)
        .bind(record.created_at_epoch_seconds)
        .execute(&self.pool)
        .await
        .map_err(sqlx_error)?;
        Ok(())
    }

    async fn consume_challenge(
        &self,
        challenge_id: &str,
        now_epoch_seconds: i64,
    ) -> Result<WebauthnChallengeRecord, WebauthnStoreError> {
        let row = sqlx::query(
            r"
            SELECT
                challenge_id,
                user_id,
                ceremony,
                state_json,
                EXTRACT(EPOCH FROM expires_at)::BIGINT AS expires_at_epoch_seconds,
                EXTRACT(EPOCH FROM consumed_at)::BIGINT AS consumed_at_epoch_seconds,
                EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at_epoch_seconds
            FROM user_webauthn_challenges
            WHERE challenge_id = $1
            ",
        )
        .bind(challenge_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(sqlx_error)?
        .ok_or(WebauthnStoreError::ChallengeNotFound)?;
        let mut record = row_to_challenge(&row)?;
        if record.consumed_at_epoch_seconds.is_some() {
            return Err(WebauthnStoreError::ChallengeConsumed);
        }
        if record.expires_at_epoch_seconds <= now_epoch_seconds {
            return Err(WebauthnStoreError::ChallengeExpired);
        }
        sqlx::query(
            r"
            UPDATE user_webauthn_challenges
            SET consumed_at = to_timestamp($2)
            WHERE challenge_id = $1
            ",
        )
        .bind(challenge_id)
        .bind(now_epoch_seconds)
        .execute(&self.pool)
        .await
        .map_err(sqlx_error)?;
        record.consumed_at_epoch_seconds = Some(now_epoch_seconds);
        Ok(record)
    }

    async fn put_credential(
        &self,
        record: WebauthnCredentialRecord,
    ) -> Result<(), WebauthnStoreError> {
        if record.credential_id.trim().is_empty()
            || record.user_id.trim().is_empty()
            || record.user_webauthn_uuid.trim().is_empty()
        {
            return Err(WebauthnStoreError::InvalidRecord);
        }
        sqlx::query(
            r"
            INSERT INTO user_webauthn_credentials (
                credential_id,
                user_id,
                user_webauthn_uuid,
                display_name,
                public_key_cose,
                signature_counter,
                transports,
                user_verified,
                backup_eligible,
                backup_state,
                registration_policy,
                attestation_format,
                attestation,
                created_at,
                last_used_at,
                revoked_at,
                metadata
            )
            VALUES (
                $1,
                $2,
                CAST($3 AS uuid),
                $4,
                $5,
                $6,
                $7,
                $8,
                $9,
                $10,
                $11,
                $12,
                $13,
                to_timestamp($14),
                CASE WHEN $15::BIGINT IS NULL THEN NULL ELSE to_timestamp($15) END,
                CASE WHEN $16::BIGINT IS NULL THEN NULL ELSE to_timestamp($16) END,
                $17
            )
            ON CONFLICT (credential_id) DO UPDATE SET
                display_name = EXCLUDED.display_name,
                public_key_cose = EXCLUDED.public_key_cose,
                signature_counter = EXCLUDED.signature_counter,
                transports = EXCLUDED.transports,
                user_verified = EXCLUDED.user_verified,
                backup_eligible = EXCLUDED.backup_eligible,
                backup_state = EXCLUDED.backup_state,
                registration_policy = EXCLUDED.registration_policy,
                attestation_format = EXCLUDED.attestation_format,
                attestation = EXCLUDED.attestation,
                last_used_at = EXCLUDED.last_used_at,
                revoked_at = EXCLUDED.revoked_at,
                metadata = EXCLUDED.metadata
            ",
        )
        .bind(&record.credential_id)
        .bind(&record.user_id)
        .bind(&record.user_webauthn_uuid)
        .bind(&record.display_name)
        .bind(&record.public_key_cose)
        .bind(record.signature_counter)
        .bind(&record.transports)
        .bind(record.user_verified)
        .bind(record.backup_eligible)
        .bind(record.backup_state)
        .bind(&record.registration_policy)
        .bind(&record.attestation_format)
        .bind(&record.attestation)
        .bind(record.created_at_epoch_seconds)
        .bind(record.last_used_at_epoch_seconds)
        .bind(record.revoked_at_epoch_seconds)
        .bind(&record.metadata)
        .execute(&self.pool)
        .await
        .map_err(sqlx_error)?;
        Ok(())
    }

    async fn list_active_credentials(
        &self,
        user_id: &str,
    ) -> Result<Vec<WebauthnCredentialRecord>, WebauthnStoreError> {
        let rows = sqlx::query(
            r"
            SELECT
                credential_id,
                user_id,
                user_webauthn_uuid::TEXT AS user_webauthn_uuid,
                display_name,
                public_key_cose,
                signature_counter,
                transports,
                user_verified,
                backup_eligible,
                backup_state,
                registration_policy,
                attestation_format,
                attestation,
                EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at_epoch_seconds,
                EXTRACT(EPOCH FROM last_used_at)::BIGINT AS last_used_at_epoch_seconds,
                EXTRACT(EPOCH FROM revoked_at)::BIGINT AS revoked_at_epoch_seconds,
                metadata
            FROM user_webauthn_credentials
            WHERE user_id = $1 AND revoked_at IS NULL
            ORDER BY created_at ASC
            ",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(sqlx_error)?;

        rows.iter().map(row_to_credential).collect()
    }

    async fn mark_credential_used(
        &self,
        credential_id: &str,
        signature_counter: i64,
        used_at_epoch_seconds: i64,
    ) -> Result<(), WebauthnStoreError> {
        let result = sqlx::query(
            r"
            UPDATE user_webauthn_credentials
            SET signature_counter = $2, last_used_at = to_timestamp($3)
            WHERE credential_id = $1
            ",
        )
        .bind(credential_id)
        .bind(signature_counter)
        .bind(used_at_epoch_seconds)
        .execute(&self.pool)
        .await
        .map_err(sqlx_error)?;
        if result.rows_affected() == 0 {
            return Err(WebauthnStoreError::InvalidRecord);
        }
        Ok(())
    }

    async fn revoke_credential(
        &self,
        credential_id: &str,
        revoked_at_epoch_seconds: i64,
    ) -> Result<(), WebauthnStoreError> {
        let result = sqlx::query(
            r"
            UPDATE user_webauthn_credentials
            SET revoked_at = to_timestamp($2)
            WHERE credential_id = $1
            ",
        )
        .bind(credential_id)
        .bind(revoked_at_epoch_seconds)
        .execute(&self.pool)
        .await
        .map_err(sqlx_error)?;
        if result.rows_affected() == 0 {
            return Err(WebauthnStoreError::InvalidRecord);
        }
        Ok(())
    }
}

fn ceremony_as_str(ceremony: &WebauthnCeremony) -> &'static str {
    match ceremony {
        WebauthnCeremony::Registration => "registration",
        WebauthnCeremony::Assertion => "assertion",
    }
}

fn ceremony_from_str(value: &str) -> Result<WebauthnCeremony, WebauthnStoreError> {
    match value {
        "registration" => Ok(WebauthnCeremony::Registration),
        "assertion" => Ok(WebauthnCeremony::Assertion),
        _ => Err(WebauthnStoreError::InvalidRecord),
    }
}

fn row_to_challenge(
    row: &sqlx::postgres::PgRow,
) -> Result<WebauthnChallengeRecord, WebauthnStoreError> {
    let ceremony: String = row.try_get("ceremony").map_err(sqlx_error)?;
    Ok(WebauthnChallengeRecord {
        challenge_id: row.try_get("challenge_id").map_err(sqlx_error)?,
        user_id: row.try_get("user_id").map_err(sqlx_error)?,
        ceremony: ceremony_from_str(&ceremony)?,
        state_json: row.try_get("state_json").map_err(sqlx_error)?,
        expires_at_epoch_seconds: row
            .try_get("expires_at_epoch_seconds")
            .map_err(sqlx_error)?,
        consumed_at_epoch_seconds: row
            .try_get("consumed_at_epoch_seconds")
            .map_err(sqlx_error)?,
        created_at_epoch_seconds: row
            .try_get("created_at_epoch_seconds")
            .map_err(sqlx_error)?,
    })
}

fn row_to_credential(
    row: &sqlx::postgres::PgRow,
) -> Result<WebauthnCredentialRecord, WebauthnStoreError> {
    Ok(WebauthnCredentialRecord {
        credential_id: row.try_get("credential_id").map_err(sqlx_error)?,
        user_id: row.try_get("user_id").map_err(sqlx_error)?,
        user_webauthn_uuid: row.try_get("user_webauthn_uuid").map_err(sqlx_error)?,
        display_name: row.try_get("display_name").map_err(sqlx_error)?,
        public_key_cose: row.try_get("public_key_cose").map_err(sqlx_error)?,
        signature_counter: row.try_get("signature_counter").map_err(sqlx_error)?,
        transports: row.try_get("transports").map_err(sqlx_error)?,
        user_verified: row.try_get("user_verified").map_err(sqlx_error)?,
        backup_eligible: row.try_get("backup_eligible").map_err(sqlx_error)?,
        backup_state: row.try_get("backup_state").map_err(sqlx_error)?,
        registration_policy: row.try_get("registration_policy").map_err(sqlx_error)?,
        attestation_format: row.try_get("attestation_format").map_err(sqlx_error)?,
        attestation: row.try_get("attestation").map_err(sqlx_error)?,
        created_at_epoch_seconds: row
            .try_get("created_at_epoch_seconds")
            .map_err(sqlx_error)?,
        last_used_at_epoch_seconds: row
            .try_get("last_used_at_epoch_seconds")
            .map_err(sqlx_error)?,
        revoked_at_epoch_seconds: row
            .try_get("revoked_at_epoch_seconds")
            .map_err(sqlx_error)?,
        metadata: row.try_get("metadata").map_err(sqlx_error)?,
    })
}

fn sqlx_error(error: sqlx::Error) -> WebauthnStoreError {
    let message = error.to_string();
    drop(error);
    WebauthnStoreError::Database(message)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use serde_json::json;

    use super::*;

    #[tokio::test]
    async fn challenge_can_only_be_consumed_once_before_expiry() {
        let store = InMemoryWebauthnStore::default();
        store
            .put_challenge(WebauthnChallengeRecord {
                challenge_id: "challenge-1".to_owned(),
                user_id: "user-1".to_owned(),
                ceremony: WebauthnCeremony::Registration,
                state_json: json!({ "challenge": "opaque" }),
                expires_at_epoch_seconds: 200,
                consumed_at_epoch_seconds: None,
                created_at_epoch_seconds: 100,
            })
            .await
            .expect("challenge should be stored");

        let consumed = store
            .consume_challenge("challenge-1", 150)
            .await
            .expect("challenge should be consumed");
        assert_eq!(consumed.consumed_at_epoch_seconds, Some(150));
        assert_eq!(
            store.consume_challenge("challenge-1", 151).await,
            Err(WebauthnStoreError::ChallengeConsumed)
        );
    }

    #[tokio::test]
    async fn revoked_credentials_are_not_listed_as_active() {
        let store = InMemoryWebauthnStore::default();
        store
            .put_credential(fixture_credential("credential-1"))
            .await
            .expect("credential should be stored");
        assert_eq!(
            store.list_active_credentials("user-1").await.unwrap().len(),
            1
        );

        store
            .mark_credential_used("credential-1", 42, 250)
            .await
            .expect("credential should be marked used");
        store
            .revoke_credential("credential-1", 300)
            .await
            .expect("credential should be revoked");

        assert_eq!(store.list_active_credentials("user-1").await.unwrap(), []);
    }

    fn fixture_credential(credential_id: &str) -> WebauthnCredentialRecord {
        WebauthnCredentialRecord {
            credential_id: credential_id.to_owned(),
            user_id: "user-1".to_owned(),
            user_webauthn_uuid: "018f6f70-1c40-7000-8000-000000000001".to_owned(),
            display_name: "User One".to_owned(),
            public_key_cose: json!({ "kty": "EC2" }),
            signature_counter: 0,
            transports: json!(["internal"]),
            user_verified: true,
            backup_eligible: true,
            backup_state: false,
            registration_policy: "required".to_owned(),
            attestation_format: Some("none".to_owned()),
            attestation: json!({}),
            created_at_epoch_seconds: 100,
            last_used_at_epoch_seconds: None,
            revoked_at_epoch_seconds: None,
            metadata: json!({}),
        }
    }
}
