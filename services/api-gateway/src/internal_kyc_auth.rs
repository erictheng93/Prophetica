//! Gateway-side signer for the kyc-service internal tier endpoint.
//!
//! Mirrors `services/kyc-service/src/internal_auth.rs`. Service-to-service
//! signing/verification stays symmetrical with the same canonical string and
//! the same secret env var (`INTERNAL_CALL_HMAC_SECRET`).

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

pub const INTERNAL_CALL_TIMESTAMP_HEADER: &str = "x-internal-call-timestamp";
pub const INTERNAL_CALL_SIGNATURE_HEADER: &str = "x-internal-call-signature";

/// Computes the canonical HMAC-SHA256 signature an internal kyc-service call
/// expects. The wire format must match
/// `kyc_service::internal_auth::sign_internal_call`.
#[must_use]
#[allow(clippy::missing_panics_doc)]
pub fn sign_internal_call_for_kyc(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_is_deterministic_for_same_inputs() {
        let a = sign_internal_call_for_kyc("GET", "/p", b"", 42, "s");
        let b = sign_internal_call_for_kyc("GET", "/p", b"", 42, "s");
        assert_eq!(a, b);
        assert_eq!(a.len(), 64); // 32-byte HMAC hex
    }

    #[test]
    fn changing_any_field_changes_signature() {
        let base = sign_internal_call_for_kyc("GET", "/p", b"", 42, "s");
        assert_ne!(base, sign_internal_call_for_kyc("POST", "/p", b"", 42, "s"));
        assert_ne!(base, sign_internal_call_for_kyc("GET", "/q", b"", 42, "s"));
        assert_ne!(base, sign_internal_call_for_kyc("GET", "/p", b"x", 42, "s"));
        assert_ne!(base, sign_internal_call_for_kyc("GET", "/p", b"", 43, "s"));
        assert_ne!(base, sign_internal_call_for_kyc("GET", "/p", b"", 42, "t"));
    }
}
