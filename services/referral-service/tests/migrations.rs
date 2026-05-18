#[test]
fn referral_migrations_do_not_reference_wallet_ledger_tables() {
    let migrations_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");

    for entry in std::fs::read_dir(&migrations_dir)
        .unwrap_or_else(|error| panic!("migrations directory should be readable: {error}"))
    {
        let path = entry
            .unwrap_or_else(|error| panic!("migration path should be readable: {error}"))
            .path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("sql") {
            continue;
        }
        let migration = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("migration should be readable: {error}"));

        assert!(
            !migration.contains("ledger_transactions"),
            "{} must not depend on wallet-ledger-service tables",
            path.display()
        );
    }
}
