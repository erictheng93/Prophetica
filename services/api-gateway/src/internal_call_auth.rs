//! Shared HMAC verification for admin internal calls.
//!
//! SPEC-005 §5.3.3: the admin server route signs each call to api-gateway with
//! an HMAC over `method\npath\nbody_hash\ntimestamp\nactor\nrole`; api-gateway
//! verifies the signature and a timestamp skew window before forwarding.

use axum::http::HeaderMap;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

/// Timestamp skew limit, matching the existing analytics verification (300s).
const TIMESTAMP_SKEW_LIMIT_SECS: i64 = 300;

#[derive(Debug)]
pub enum InternalCallRejection {
    Unauthorized,
}

#[derive(Debug)]
pub struct VerifiedActor {
    pub actor_id: String,
    pub role: String,
}

/// Verifies an admin internal call's signed headers against `secret`.
///
/// # Errors
///
/// Returns [`InternalCallRejection::Unauthorized`] when a required header is
/// missing, the timestamp skew exceeds the limit, or the signature mismatches.
pub fn verify_internal_call(
    headers: &HeaderMap,
    method: &str,
    path: &str,
    body: &[u8],
    secret: &str,
    now: i64,
) -> Result<VerifiedActor, InternalCallRejection> {
    let actor_id = header(headers, "x-actor-id").ok_or(InternalCallRejection::Unauthorized)?;
    let role = header(headers, "x-operator-role").ok_or(InternalCallRejection::Unauthorized)?;
    let timestamp = header(headers, "x-internal-call-timestamp")
        .and_then(|value| value.parse::<i64>().ok())
        .ok_or(InternalCallRejection::Unauthorized)?;
    if (now - timestamp).abs() > TIMESTAMP_SKEW_LIMIT_SECS {
        return Err(InternalCallRejection::Unauthorized);
    }
    let signature =
        header(headers, "x-internal-call-signature").ok_or(InternalCallRejection::Unauthorized)?;
    let expected = sign_internal_call(method, path, body, timestamp, &actor_id, &role, secret);
    if !constant_time_eq(signature.as_bytes(), expected.as_bytes()) {
        return Err(InternalCallRejection::Unauthorized);
    }
    Ok(VerifiedActor { actor_id, role })
}

/// Computes the canonical HMAC-SHA256 signature for an internal call.
#[must_use]
// The `.expect` below cannot actually panic: HMAC accepts a key of any length.
#[allow(clippy::missing_panics_doc)]
pub fn sign_internal_call(
    method: &str,
    path: &str,
    body: &[u8],
    timestamp: i64,
    actor_id: &str,
    role: &str,
    secret: &str,
) -> String {
    let body_hash = sha256_hex(body);
    let canonical = format!("{method}\n{path}\n{body_hash}\n{timestamp}\n{actor_id}\n{role}");
    // HMAC accepts a key of any length, so this construction cannot fail.
    #[allow(clippy::expect_used)]
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts a key of any length");
    mac.update(canonical.as_bytes());
    hex_lower(&mac.finalize().into_bytes())
}

fn header(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex_lower(&Sha256::digest(bytes))
}

fn hex_lower(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut acc, byte| {
            let _ = write!(acc, "{byte:02x}");
            acc
        })
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0_u8, |acc, (left, right)| acc | (left ^ right))
        == 0
}
