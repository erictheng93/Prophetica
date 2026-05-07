use thiserror::Error;

const RANDOM_SPACE: u32 = 32_u32.pow(4);
const MAX_COLLISION_RETRIES: usize = 5;
const CROCKFORD_BASE32: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

pub struct ReferralCodeGenerator;

impl ReferralCodeGenerator {
    /// Generates a SPEC-011 v0.2.1 referral code from a username and random candidates.
    ///
    /// Format: 12 lowercase BLAKE3 hex chars, underscore, then four Crockford
    /// Base32 chars. Collision detection is delegated to the caller so this
    /// helper can be used inside the registration transaction.
    ///
    /// # Errors
    ///
    /// Returns [`ReferralCodeError::CollisionLimitExceeded`] when all five
    /// candidates collide.
    pub fn generate<I, F>(
        username: &str,
        random_candidates: I,
        mut code_exists: F,
    ) -> Result<String, ReferralCodeError>
    where
        I: IntoIterator<Item = u32>,
        F: FnMut(&str) -> bool,
    {
        let username_hash = blake3::hash(username.as_bytes()).to_hex().to_string();
        let prefix = &username_hash[..12];

        for random_value in random_candidates.into_iter().take(MAX_COLLISION_RETRIES) {
            let suffix = encode_crockford_base32_4(random_value % RANDOM_SPACE);
            let candidate = format!("{prefix}_{suffix}");
            if !code_exists(&candidate) {
                return Ok(candidate);
            }
        }

        Err(ReferralCodeError::CollisionLimitExceeded)
    }
}

#[must_use]
fn encode_crockford_base32_4(value: u32) -> String {
    let indexes = [
        ((value >> 15) & 31) as usize,
        ((value >> 10) & 31) as usize,
        ((value >> 5) & 31) as usize,
        (value & 31) as usize,
    ];

    indexes
        .into_iter()
        .map(|index| char::from(CROCKFORD_BASE32[index]))
        .collect()
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReferralCodeError {
    #[error("ERROR_CODE_REFERRAL_CODE_COLLISION")]
    CollisionLimitExceeded,
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{ReferralCodeError, ReferralCodeGenerator};

    #[test]
    fn generates_blake3_prefix_and_crockford_suffix() {
        let code = ReferralCodeGenerator::generate("alice", [33_u32], |_| false)
            .unwrap_or_else(|error| panic!("code should generate: {error}"));
        let expected_prefix = &blake3::hash(b"alice").to_hex().to_string()[..12];

        assert_eq!(code, format!("{expected_prefix}_0011"));
        assert_eq!(code.len(), 17);
    }

    #[test]
    fn retries_collisions_and_returns_first_free_candidate() {
        let mut attempts = 0;
        let code = ReferralCodeGenerator::generate("alice", [1_u32, 2, 33], |candidate| {
            attempts += 1;
            candidate.ends_with("0001") || candidate.ends_with("0002")
        })
        .unwrap_or_else(|error| panic!("third candidate should be available: {error}"));

        assert!(code.ends_with("0011"));
        assert_eq!(attempts, 3);
    }

    #[test]
    fn errors_after_five_collisions() {
        let Err(error) =
            ReferralCodeGenerator::generate("alice", [1_u32, 2, 3, 4, 5, 33], |_| true)
        else {
            panic!("five collisions should fail");
        };

        assert_eq!(error, ReferralCodeError::CollisionLimitExceeded);
    }

    #[test]
    fn suffix_uses_crockford_alphabet_without_ambiguous_chars() {
        let code = ReferralCodeGenerator::generate("alice", [1_048_575_u32], |_| false)
            .unwrap_or_else(|error| panic!("code should generate: {error}"));
        let suffix = code.split_once('_').map_or_else(
            || panic!("code should contain underscore"),
            |(_, suffix)| suffix,
        );

        assert_eq!(suffix, "ZZZZ");
        assert!(!suffix.contains(['I', 'L', 'O', 'U']));
    }
}
