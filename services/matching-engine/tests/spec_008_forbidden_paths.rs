#![allow(clippy::expect_used)]

#[test]
fn matching_engine_does_not_define_region_local_rpo_downgrade_path() {
    let source_files = [
        include_str!("../src/service.rs"),
        include_str!("../src/durability.rs"),
        include_str!("../src/wal.rs"),
        include_str!("../src/projection.rs"),
    ];
    let forbidden_terms = [
        "RPO <= 5s same region",
        "RPO ≤ 5s same region",
        "same-region fallback",
        "region-local fallback",
        "degraded_rpo",
        "DegradedRpo",
        "fail_open",
        "FailOpen",
    ];

    for source in source_files {
        for forbidden in forbidden_terms {
            assert!(
                !source.contains(forbidden),
                "matching-engine must fail closed, forbidden fallback found: {forbidden}"
            );
        }
    }
}
