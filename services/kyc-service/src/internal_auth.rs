//! HMAC-signed internal call verification for kyc-service.
//!
//! SPEC-023 plan v0.1.3 §11 D-4 + Phase 0 review H-3: the internal
//! tier-query endpoint must verify that the caller (api-gateway) is a
//! trusted internal service, not an unauthenticated network peer. The
//! signature scheme mirrors `services/api-gateway/src/internal_call_auth.rs`
//! but drops the `actor_id`/`role` fields since service-to-service calls
//! have no user principal.
//!
//! Canonical string signed by both sides:
//!   `{method}\n{path}\n{sha256_hex(body)}\n{timestamp}`
//!
//! Both signer (gateway) and verifier (kyc-service) read the secret from
//! `INTERNAL_CALL_HMAC_SECRET` so the same env var binds the admin and
//! service-to-service trust paths.

use axum::http::HeaderMap;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

pub const INTERNAL_CALL_TIMESTAMP_HEADER: &str = "x-internal-call-timestamp";
pub const INTERNAL_CALL_SIGNATURE_HEADER: &str = "x-internal-call-signature";
const TIMESTAMP_SKEW_LIMIT_SECS: i64 = 300;

#[derive(Debug, PartialEq, Eq)]
pub enum InternalCallRejection {
    Unauthorized,
}

/// Verifies an internal service-to-service call against `secret`.
///
/// # Errors
///
/// Returns [`InternalCallRejection::Unauthorized`] when a required header
/// is missing, the timestamp skew exceeds the limit, or the signature
/// does not match.
pub fn verify_internal_call(
    headers: &HeaderMap,
    method: &str,
    path: &str,
    body: &[u8],
    secret: &str,
    now: i64,
) -> Result<(), InternalCallRejection> {
    let timestamp = header(headers, INTERNAL_CALL_TIMESTAMP_HEADER)
        .and_then(|value| value.parse::<i64>().ok())
        .ok_or(InternalCallRejection::Unauthorized)?;
    if (now - timestamp).abs() > TIMESTAMP_SKEW_LIMIT_SECS {
        return Err(InternalCallRejection::Unauthorized);
    }
    let signature =
        header(headers, INTERNAL_CALL_SIGNATURE_HEADER).ok_or(InternalCallRejection::Unauthorized)?;
    let expected = sign_internal_call(method, path, body, timestamp, secret);
    if !constant_time_eq(signature.as_bytes(), expected.as_bytes()) {
        return Err(InternalCallRejection::Unauthorized);
    }
    Ok(())
}

/// Computes the canonical HMAC-SHA256 signature for an internal call.
#[must_use]
#[allow(clippy::missing_panics_doc)]
pub fn sign_internal_call(
    method: &str,
    path: &str,
    body: &[u8],
    timestamp: i64,
    secret: &str,
) -> String {
    let body_hash = sha256_hex(body);
    let canonical = format!("{method}\n{path}\n{body_hash}\n{timestamp}");
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn signed_headers(method: &str, path: &str, ts: i64, secret: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            INTERNAL_CALL_TIMESTAMP_HEADER,
            HeaderValue::from_str(&ts.to_string()).expect("ts header"),
        );
        let sig = sign_internal_call(method, path, b"", ts, secret);
        headers.insert(
            INTERNAL_CALL_SIGNATURE_HEADER,
            HeaderValue::from_str(&sig).expect("sig header"),
        );
        headers
    }

    #[test]
    fn matching_signature_passes() {
        let h = signed_headers("GET", "/internal/kyc/users/u/tier", 1_000, "s");
        verify_internal_call(&h, "GET", "/internal/kyc/users/u/tier", b"", "s", 1_000)
            .expect("valid call");
    }

    #[test]
    fn skew_beyond_limit_rejected() {
        let h = signed_headers("GET", "/p", 1_000, "s");
        assert_eq!(
            verify_internal_call(&h, "GET", "/p", b"", "s", 1_000 + 301),
            Err(InternalCallRejection::Unauthorized),
        );
    }

    #[test]
    fn wrong_secret_rejected() {
        let h = signed_headers("GET", "/p", 1_000, "s");
        assert_eq!(
            verify_internal_call(&h, "GET", "/p", b"", "other", 1_000),
            Err(InternalCallRejection::Unauthorized),
        );
    }

    #[test]
    fn missing_headers_rejected() {
        let h = HeaderMap::new();
        assert_eq!(
            verify_internal_call(&h, "GET", "/p", b"", "s", 1_000),
            Err(InternalCallRejection::Unauthorized),
        );
    }

    #[test]
    fn path_mismatch_rejected() {
        let h = signed_headers("GET", "/intended", 1_000, "s");
        assert_eq!(
            verify_internal_call(&h, "GET", "/different", b"", "s", 1_000),
            Err(InternalCallRejection::Unauthorized),
        );
    }
}
