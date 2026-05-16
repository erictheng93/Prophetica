#![allow(clippy::expect_used)]

use serde_json::json;
use user_service::webauthn_store::{
    InMemoryWebauthnStore, WebauthnCeremony, WebauthnChallengeRecord, WebauthnCredentialRecord,
    WebauthnStore, WebauthnStoreError,
};

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
