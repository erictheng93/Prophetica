use std::{error::Error, fs, path::PathBuf};

const SERVICE_CRATES: &[&str] = &[
    "api-gateway",
    "ws-gateway",
    "user-service",
    "kyc-service",
    "wallet-ledger-service",
    "market-service",
    "matching-engine",
    "position-service",
    "settlement-worker",
    "risk-antifraud-service",
    "referral-service",
    "notification-service",
];

const MVP_BINARIES: &[&str] = &[
    "core-api",
    "realtime-gateway",
    "wallet-ledger-service",
    "matching-engine-hot",
    "matching-engine-cold",
    "settlement-worker",
    "kyc-service",
    "notification-and-risk",
];

#[test]
fn service_topology_uses_mvp_deployable_binaries() -> Result<(), Box<dyn Error>> {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let cargo_toml = fs::read_to_string(workspace_root.join("Cargo.toml"))?;

    for binary in MVP_BINARIES {
        let member = format!("\"services/_binaries/{binary}\"");
        let binary_root = workspace_root.join("services/_binaries").join(binary);

        assert!(
            cargo_toml.contains(&member),
            "workspace is missing {member}"
        );
        assert!(
            binary_root.join("Cargo.toml").exists(),
            "{binary} Cargo.toml is missing"
        );
        assert!(
            binary_root.join("src/main.rs").exists(),
            "{binary} main.rs is missing"
        );
    }

    for service in SERVICE_CRATES {
        let old_service_entrypoint = workspace_root
            .join("services")
            .join(service)
            .join("src/main.rs");
        assert!(
            !old_service_entrypoint.exists(),
            "{service} still exposes its own service binary entrypoint"
        );
    }

    Ok(())
}
