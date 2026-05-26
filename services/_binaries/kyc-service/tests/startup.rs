use std::{
    process::Command,
    thread,
    time::{Duration, Instant},
};

#[test]
fn production_without_required_secrets_exits_nonzero() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_kyc-service"))
        .env("PROPHETICA_ENV", "production")
        .env("KYC_SERVICE_PORT", "0")
        .env_remove("KYC_STORE")
        .env_remove("DATABASE_URL")
        .env("KYC_PROVIDER_WEBHOOK_SECRET", "")
        .env("KYC_DOCUMENT_HASH_SALT", "")
        .env("INTERNAL_CALL_HMAC_SECRET", "")
        .spawn()
        .unwrap_or_else(|error| panic!("kyc-service should spawn: {error}"));

    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        let status = match child.try_wait() {
            Ok(status) => status,
            Err(error) => panic!("kyc-service status should be available: {error}"),
        };
        if let Some(status) = status {
            assert!(!status.success(), "kyc-service should fail fast");
            return;
        }
        thread::sleep(Duration::from_millis(50));
    }

    let _ = child.kill();
    let _ = child.wait();
    panic!("kyc-service started instead of failing fast without production secrets");
}
