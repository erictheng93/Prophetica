#![allow(clippy::unwrap_used, clippy::expect_used)]

use api_gateway::internal_call_auth::{
    sign_internal_call, verify_internal_call, InternalCallRejection,
};
use axum::http::HeaderMap;

fn headers(
    method: &str,
    path: &str,
    body: &[u8],
    ts: i64,
    actor: &str,
    role: &str,
    secret: &str,
) -> HeaderMap {
    let sig = sign_internal_call(method, path, body, ts, actor, role, secret);
    let mut h = HeaderMap::new();
    h.insert("x-actor-id", actor.parse().unwrap());
    h.insert("x-operator-role", role.parse().unwrap());
    h.insert("x-internal-call-timestamp", ts.to_string().parse().unwrap());
    h.insert("x-internal-call-signature", sig.parse().unwrap());
    h
}

#[test]
fn accepts_valid_signed_call() {
    let secret = "shared-secret-32-bytes-1234567890";
    let h = headers("GET", "/p", b"", 1_000, "U-1", "admin", secret);
    let actor = verify_internal_call(&h, "GET", "/p", b"", secret, 1_000).expect("valid");
    assert_eq!(actor.actor_id, "U-1");
    assert_eq!(actor.role, "admin");
}

#[test]
fn rejects_timestamp_skew_over_limit() {
    let secret = "shared-secret-32-bytes-1234567890";
    let h = headers("GET", "/p", b"", 1_000, "U-1", "admin", secret);
    let err = verify_internal_call(&h, "GET", "/p", b"", secret, 1_000 + 301).unwrap_err();
    assert!(matches!(err, InternalCallRejection::Unauthorized));
}

#[test]
fn rejects_tampered_signature() {
    let secret = "shared-secret-32-bytes-1234567890";
    let mut h = headers("GET", "/p", b"", 1_000, "U-1", "admin", secret);
    h.insert("x-internal-call-signature", "deadbeef".parse().unwrap());
    let err = verify_internal_call(&h, "GET", "/p", b"", secret, 1_000).unwrap_err();
    assert!(matches!(err, InternalCallRejection::Unauthorized));
}
