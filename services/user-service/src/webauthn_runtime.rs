use std::env;

use serde_json::Value;
use thiserror::Error;
use ulid::Ulid;
use webauthn_rs::prelude::*;

use crate::webauthn_store::{
    WebauthnCeremony, WebauthnChallengeRecord, WebauthnCredentialRecord, WebauthnStore,
    WebauthnStoreError,
};

const WEBAUTHN_CHALLENGE_TTL_SECONDS: i64 = 300;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebauthnRuntimeConfig {
    pub rp_id: String,
    pub rp_origin: String,
}

impl WebauthnRuntimeConfig {
    #[must_use]
    pub fn from_env() -> Option<Self> {
        let rp_id = env::var("USER_SERVICE_WEBAUTHN_RP_ID").ok()?;
        let rp_origin = env::var("USER_SERVICE_WEBAUTHN_RP_ORIGIN").ok()?;
        let config = Self { rp_id, rp_origin };
        webauthn_from_config(&config).ok().map(|_| config)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WebauthnRuntimeError {
    #[error("invalid WebAuthn relying-party configuration")]
    InvalidRelyingPartyConfig,
    #[error("invalid WebAuthn user UUID")]
    InvalidUserUuid,
    #[error("failed to start WebAuthn registration")]
    RegistrationStartFailed,
    #[error("failed to serialize WebAuthn challenge")]
    SerializationFailed,
    #[error("failed to deserialize WebAuthn payload")]
    DeserializationFailed,
    #[error("failed to finish WebAuthn registration")]
    RegistrationFinishFailed,
    #[error("no active WebAuthn credentials are available")]
    NoCredentials,
    #[error("failed to start WebAuthn authentication")]
    AuthenticationStartFailed,
    #[error("failed to finish WebAuthn authentication")]
    AuthenticationFinishFailed,
    #[error("authenticated WebAuthn credential was not found")]
    CredentialNotFound,
    #[error("failed to persist WebAuthn challenge state")]
    StoreFailed,
}

#[derive(Debug, PartialEq)]
pub struct PasskeyRegistrationStart {
    pub challenge_id: String,
    pub creation_options: Value,
    pub state_json: Value,
}

#[derive(Debug, PartialEq)]
pub struct StoredPasskeyRegistrationStart {
    pub challenge_id: String,
    pub creation_options: Value,
    pub expires_at_epoch_seconds: i64,
}

#[derive(Debug, PartialEq)]
pub struct PasskeyAssertionStart {
    pub challenge_id: String,
    pub request_options: Value,
    pub state_json: Value,
}

#[derive(Debug, PartialEq)]
pub struct StoredPasskeyAssertionStart {
    pub challenge_id: String,
    pub request_options: Value,
    pub expires_at_epoch_seconds: i64,
}

#[derive(Debug, PartialEq)]
pub struct PasskeyAssertionFinish {
    pub credential_id: String,
    pub updated_passkey: Value,
    pub authentication_result: Value,
    pub signature_counter: i64,
    pub user_verified: bool,
    pub backup_eligible: bool,
    pub backup_state: bool,
}

/// Starts a passkey registration ceremony and returns serialized creation options plus state.
///
/// # Errors
///
/// Returns [`WebauthnRuntimeError`] when the relying-party origin or user UUID is invalid,
/// `WebAuthn` registration cannot be started, or the generated options/state cannot be serialized.
pub fn start_passkey_registration(
    config: &WebauthnRuntimeConfig,
    user_uuid: &str,
    username: &str,
    display_name: &str,
) -> Result<PasskeyRegistrationStart, WebauthnRuntimeError> {
    let origin = Url::parse(&config.rp_origin)
        .map_err(|_| WebauthnRuntimeError::InvalidRelyingPartyConfig)?;
    let webauthn = WebauthnBuilder::new(&config.rp_id, &origin)
        .map_err(|_| WebauthnRuntimeError::InvalidRelyingPartyConfig)?
        .build()
        .map_err(|_| WebauthnRuntimeError::InvalidRelyingPartyConfig)?;
    let user_uuid =
        Uuid::parse_str(user_uuid).map_err(|_| WebauthnRuntimeError::InvalidUserUuid)?;
    let (creation_options, state) = webauthn
        .start_passkey_registration(user_uuid, username, display_name, None)
        .map_err(|_| WebauthnRuntimeError::RegistrationStartFailed)?;

    Ok(PasskeyRegistrationStart {
        challenge_id: Ulid::new().to_string(),
        creation_options: serde_json::to_value(creation_options)
            .map_err(|_| WebauthnRuntimeError::SerializationFailed)?,
        state_json: serde_json::to_value(state)
            .map_err(|_| WebauthnRuntimeError::SerializationFailed)?,
    })
}

pub fn finish_passkey_registration(
    config: &WebauthnRuntimeConfig,
    registration_data: Value,
    state_json: Value,
) -> Result<Value, WebauthnRuntimeError> {
    let webauthn = webauthn_from_config(config)?;
    let registration_data: RegisterPublicKeyCredential = serde_json::from_value(registration_data)
        .map_err(|_| WebauthnRuntimeError::DeserializationFailed)?;
    let state: PasskeyRegistration = serde_json::from_value(state_json)
        .map_err(|_| WebauthnRuntimeError::DeserializationFailed)?;
    let passkey = webauthn
        .finish_passkey_registration(&registration_data, &state)
        .map_err(|_| WebauthnRuntimeError::RegistrationFinishFailed)?;

    serde_json::to_value(passkey).map_err(|_| WebauthnRuntimeError::SerializationFailed)
}

pub fn start_passkey_assertion(
    config: &WebauthnRuntimeConfig,
    credentials: &[WebauthnCredentialRecord],
) -> Result<PasskeyAssertionStart, WebauthnRuntimeError> {
    if credentials.is_empty() {
        return Err(WebauthnRuntimeError::NoCredentials);
    }
    let webauthn = webauthn_from_config(config)?;
    let passkeys = credentials
        .iter()
        .map(|record| serde_json::from_value(record.public_key_cose.clone()))
        .collect::<Result<Vec<Passkey>, _>>()
        .map_err(|_| WebauthnRuntimeError::DeserializationFailed)?;
    let (request_options, state) = webauthn
        .start_passkey_authentication(&passkeys)
        .map_err(|_| WebauthnRuntimeError::AuthenticationStartFailed)?;

    Ok(PasskeyAssertionStart {
        challenge_id: Ulid::new().to_string(),
        request_options: serde_json::to_value(request_options)
            .map_err(|_| WebauthnRuntimeError::SerializationFailed)?,
        state_json: serde_json::to_value(state)
            .map_err(|_| WebauthnRuntimeError::SerializationFailed)?,
    })
}

pub fn finish_passkey_assertion(
    config: &WebauthnRuntimeConfig,
    assertion_data: Value,
    state_json: Value,
    credentials: &[WebauthnCredentialRecord],
) -> Result<PasskeyAssertionFinish, WebauthnRuntimeError> {
    if credentials.is_empty() {
        return Err(WebauthnRuntimeError::NoCredentials);
    }
    let webauthn = webauthn_from_config(config)?;
    let assertion_data: PublicKeyCredential = serde_json::from_value(assertion_data)
        .map_err(|_| WebauthnRuntimeError::DeserializationFailed)?;
    let state: PasskeyAuthentication = serde_json::from_value(state_json)
        .map_err(|_| WebauthnRuntimeError::DeserializationFailed)?;
    let result = webauthn
        .finish_passkey_authentication(&assertion_data, &state)
        .map_err(|_| WebauthnRuntimeError::AuthenticationFinishFailed)?;
    let signature_counter = i64::from(result.counter());
    let user_verified = result.user_verified();
    let backup_eligible = result.backup_eligible();
    let backup_state = result.backup_state();
    let authentication_result =
        serde_json::to_value(&result).map_err(|_| WebauthnRuntimeError::SerializationFailed)?;

    for record in credentials {
        let mut passkey: Passkey = serde_json::from_value(record.public_key_cose.clone())
            .map_err(|_| WebauthnRuntimeError::DeserializationFailed)?;
        if passkey.update_credential(&result).is_some() {
            let credential_id = record.credential_id.clone();
            let updated_passkey = serde_json::to_value(passkey)
                .map_err(|_| WebauthnRuntimeError::SerializationFailed)?;
            return Ok(PasskeyAssertionFinish {
                credential_id,
                updated_passkey,
                authentication_result,
                signature_counter,
                user_verified,
                backup_eligible,
                backup_state,
            });
        }
    }

    Err(WebauthnRuntimeError::CredentialNotFound)
}

/// Starts a passkey registration ceremony and persists the challenge state.
///
/// # Errors
///
/// Returns [`WebauthnRuntimeError`] when registration setup fails or the challenge state cannot be
/// stored.
pub async fn start_and_store_passkey_registration(
    store: &dyn WebauthnStore,
    config: &WebauthnRuntimeConfig,
    user_id: &str,
    user_uuid: &str,
    username: &str,
    display_name: &str,
    now_epoch_seconds: i64,
) -> Result<StoredPasskeyRegistrationStart, WebauthnRuntimeError> {
    let start = start_passkey_registration(config, user_uuid, username, display_name)?;
    let expires_at_epoch_seconds = now_epoch_seconds + WEBAUTHN_CHALLENGE_TTL_SECONDS;
    store
        .put_challenge(WebauthnChallengeRecord {
            challenge_id: start.challenge_id.clone(),
            user_id: user_id.to_owned(),
            ceremony: WebauthnCeremony::Registration,
            state_json: start.state_json,
            expires_at_epoch_seconds,
            consumed_at_epoch_seconds: None,
            created_at_epoch_seconds: now_epoch_seconds,
        })
        .await
        .map_err(map_store_error)?;

    Ok(StoredPasskeyRegistrationStart {
        challenge_id: start.challenge_id,
        creation_options: start.creation_options,
        expires_at_epoch_seconds,
    })
}

pub async fn start_and_store_passkey_assertion(
    store: &dyn WebauthnStore,
    config: &WebauthnRuntimeConfig,
    user_id: &str,
    credentials: &[WebauthnCredentialRecord],
    now_epoch_seconds: i64,
) -> Result<StoredPasskeyAssertionStart, WebauthnRuntimeError> {
    let start = start_passkey_assertion(config, credentials)?;
    let expires_at_epoch_seconds = now_epoch_seconds + WEBAUTHN_CHALLENGE_TTL_SECONDS;
    store
        .put_challenge(WebauthnChallengeRecord {
            challenge_id: start.challenge_id.clone(),
            user_id: user_id.to_owned(),
            ceremony: WebauthnCeremony::Assertion,
            state_json: start.state_json,
            expires_at_epoch_seconds,
            consumed_at_epoch_seconds: None,
            created_at_epoch_seconds: now_epoch_seconds,
        })
        .await
        .map_err(map_store_error)?;

    Ok(StoredPasskeyAssertionStart {
        challenge_id: start.challenge_id,
        request_options: start.request_options,
        expires_at_epoch_seconds,
    })
}

fn webauthn_from_config(config: &WebauthnRuntimeConfig) -> Result<Webauthn, WebauthnRuntimeError> {
    let origin = Url::parse(&config.rp_origin)
        .map_err(|_| WebauthnRuntimeError::InvalidRelyingPartyConfig)?;
    WebauthnBuilder::new(&config.rp_id, &origin)
        .map_err(|_| WebauthnRuntimeError::InvalidRelyingPartyConfig)?
        .build()
        .map_err(|_| WebauthnRuntimeError::InvalidRelyingPartyConfig)
}

fn map_store_error(_error: WebauthnStoreError) -> WebauthnRuntimeError {
    WebauthnRuntimeError::StoreFailed
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn registration_options_are_generated_by_webauthn_rs() {
        let start = start_passkey_registration(
            &WebauthnRuntimeConfig {
                rp_id: "example.test".to_owned(),
                rp_origin: "https://example.test".to_owned(),
            },
            "018f6f70-1c40-7000-8000-000000000001",
            "user@example.test",
            "User Example",
        )
        .expect("registration options should be generated");

        assert!(start.creation_options.get("publicKey").is_some());
        assert!(start.state_json.is_object());
    }

    #[test]
    fn registration_options_reject_invalid_origin() {
        assert_eq!(
            start_passkey_registration(
                &WebauthnRuntimeConfig {
                    rp_id: "example.test".to_owned(),
                    rp_origin: "not a url".to_owned(),
                },
                "018f6f70-1c40-7000-8000-000000000001",
                "user@example.test",
                "User Example",
            ),
            Err(WebauthnRuntimeError::InvalidRelyingPartyConfig)
        );
    }

    #[tokio::test]
    async fn registration_start_persists_challenge_state() {
        let store = crate::webauthn_store::InMemoryWebauthnStore::default();
        let start = start_and_store_passkey_registration(
            &store,
            &WebauthnRuntimeConfig {
                rp_id: "example.test".to_owned(),
                rp_origin: "https://example.test".to_owned(),
            },
            "user-1",
            "018f6f70-1c40-7000-8000-000000000001",
            "user@example.test",
            "User Example",
            1000,
        )
        .await
        .expect("registration start should be persisted");

        assert!(start.creation_options.get("publicKey").is_some());
        assert_eq!(start.expires_at_epoch_seconds, 1300);
        let stored = store
            .consume_challenge(&start.challenge_id, 1001)
            .await
            .expect("stored registration challenge should be consumable");
        assert_eq!(stored.user_id, "user-1");
        assert_eq!(stored.ceremony, WebauthnCeremony::Registration);
        assert!(stored.state_json.is_object());
    }

    #[test]
    fn registration_finish_uses_webauthn_rs_payload_types() {
        assert_eq!(
            finish_passkey_registration(
                &WebauthnRuntimeConfig {
                    rp_id: "example.test".to_owned(),
                    rp_origin: "https://example.test".to_owned(),
                },
                serde_json::json!({ "not": "a credential" }),
                serde_json::json!({ "not": "state" }),
            ),
            Err(WebauthnRuntimeError::DeserializationFailed)
        );
    }

    #[test]
    fn assertion_start_requires_credentials() {
        assert_eq!(
            start_passkey_assertion(
                &WebauthnRuntimeConfig {
                    rp_id: "example.test".to_owned(),
                    rp_origin: "https://example.test".to_owned(),
                },
                &[],
            ),
            Err(WebauthnRuntimeError::NoCredentials)
        );
    }

    #[test]
    fn assertion_finish_uses_webauthn_rs_payload_types() {
        assert_eq!(
            finish_passkey_assertion(
                &WebauthnRuntimeConfig {
                    rp_id: "example.test".to_owned(),
                    rp_origin: "https://example.test".to_owned(),
                },
                serde_json::json!({ "not": "a credential" }),
                serde_json::json!({ "not": "state" }),
                &[WebauthnCredentialRecord {
                    credential_id: "credential-1".to_owned(),
                    user_id: "user-1".to_owned(),
                    user_webauthn_uuid: "018f6f70-1c40-7000-8000-000000000001".to_owned(),
                    display_name: "User Example".to_owned(),
                    public_key_cose: serde_json::json!({ "not": "a passkey" }),
                    signature_counter: 0,
                    transports: serde_json::json!([]),
                    user_verified: true,
                    backup_eligible: false,
                    backup_state: false,
                    registration_policy: "required".to_owned(),
                    attestation_format: None,
                    attestation: serde_json::json!({}),
                    created_at_epoch_seconds: 1000,
                    last_used_at_epoch_seconds: None,
                    revoked_at_epoch_seconds: None,
                    metadata: serde_json::json!({}),
                }],
            ),
            Err(WebauthnRuntimeError::DeserializationFailed)
        );
    }
}
