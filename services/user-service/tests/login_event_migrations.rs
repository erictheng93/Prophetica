#![allow(clippy::expect_used)]

use std::{
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

use sqlx::{
    Row,
    postgres::{PgConnectOptions, PgPoolOptions},
};
use ulid::Ulid;

#[tokio::test]
async fn postgres_login_event_migration_enforces_contract() {
    let Some(pool) = postgres_test_pool().await else {
        return;
    };

    insert_valid_login_failure_with_identifier_hash(&pool).await;
    insert_valid_affiliate_login_success(&pool).await;

    assert_check_violation(
        &pool,
        r"
        INSERT INTO login_events (
            id, login_surface, principal_type, principal_id, event_type,
            ip_address, ip_source, auth_method
        )
        VALUES (
            $1, 'user_web', 'user', 'U-MISSING-USER-ID', 'login_success',
            '203.0.113.10', 'remote_addr', 'password'
        )
        ",
        "user login_success without user_id should fail",
    )
    .await;

    assert_check_violation(
        &pool,
        r"
        INSERT INTO login_events (
            id, login_surface, principal_type, event_type, ip_address, ip_source,
            auth_method, account_identifier_hash
        )
        VALUES (
            $1, 'admin_console', 'admin', 'login_failure', '203.0.113.11',
            'remote_addr', 'password', 'hmac-missing-key-version'
        )
        ",
        "identifier hash without key version should fail",
    )
    .await;

    assert_check_violation(
        &pool,
        r"
        INSERT INTO login_events (
            id, login_surface, principal_type, principal_id, event_type,
            ip_address, ip_source, geo_accuracy, auth_method
        )
        VALUES (
            $1, 'admin_console', 'admin', 'U-ADMIN-GEO', 'login_success',
            '203.0.113.12', 'remote_addr', 'approximate', 'password'
        )
        ",
        "approximate geo without geo_source should fail",
    )
    .await;

    let indexes: Vec<String> = sqlx::query(
        r"
        SELECT indexname
        FROM pg_indexes
        WHERE schemaname = current_schema()
          AND tablename = 'login_events'
        ORDER BY indexname
        ",
    )
    .fetch_all(&pool)
    .await
    .expect("index query should run")
    .into_iter()
    .map(|row| row.try_get("indexname").expect("indexname should decode"))
    .collect();

    for expected in [
        "login_events_event_type_time_idx",
        "login_events_identifier_hash_time_idx",
        "login_events_ip_time_idx",
        "login_events_principal_time_idx",
        "login_events_user_time_idx",
    ] {
        assert!(
            indexes.iter().any(|index| index == expected),
            "missing expected login_events index: {expected}"
        );
    }
}

async fn insert_valid_login_failure_with_identifier_hash(pool: &sqlx::PgPool) {
    sqlx::query(
        r"
        INSERT INTO login_events (
            id, login_surface, principal_type, event_type, ip_address, ip_source,
            account_identifier_hash, account_identifier_hash_key_version,
            failure_reason, auth_method
        )
        VALUES (
            $1, 'admin_console', 'admin', 'login_failure', '203.0.113.20',
            'remote_addr', 'hmac-ok', 'test-v1', 'invalid_credentials', 'password'
        )
        ",
    )
    .bind(unique_id("login-failure"))
    .execute(pool)
    .await
    .expect("valid login_failure should insert");
}

async fn insert_valid_affiliate_login_success(pool: &sqlx::PgPool) {
    sqlx::query(
        r"
        INSERT INTO login_events (
            id, login_surface, principal_type, principal_id, principal_external_id,
            event_type, ip_address, ip_source, geo_source, geo_accuracy,
            user_agent_raw, auth_method, risk_flags
        )
        VALUES (
            $1, 'affiliate_portal', 'affiliate', 'U-AFF-MIGRATION',
            'AFF-MIGRATION', 'login_success', '203.0.113.21', 'remote_addr',
            'cloudflare', 'approximate', 'Mozilla/5.0', 'password', '[]'::jsonb
        )
        ",
    )
    .bind(unique_id("affiliate-success"))
    .execute(pool)
    .await
    .expect("valid affiliate login_success should insert");
}

async fn assert_check_violation(pool: &sqlx::PgPool, sql: &str, context: &str) {
    let error = sqlx::query(sql)
        .bind(unique_id("constraint"))
        .execute(pool)
        .await
        .expect_err(context);
    let database_error = error
        .as_database_error()
        .expect("constraint failure should be a database error");
    assert_eq!(
        database_error.code().as_deref(),
        Some("23514"),
        "{context}: expected check_violation"
    );
}

async fn postgres_test_pool() -> Option<sqlx::PgPool> {
    let Some(database_url) = std::env::var("USER_SERVICE_TEST_DATABASE_URL").ok() else {
        eprintln!("skipping login event migration test: USER_SERVICE_TEST_DATABASE_URL is not set");
        return None;
    };
    let bootstrap_pool = sqlx::PgPool::connect(&database_url)
        .await
        .unwrap_or_else(|error| panic!("test database should be reachable: {error}"));
    let schema = unique_schema("login_event_migrations");
    sqlx::query(&format!("CREATE SCHEMA IF NOT EXISTS {schema}"))
        .execute(&bootstrap_pool)
        .await
        .unwrap_or_else(|error| panic!("test schema should be created: {error}"));
    bootstrap_pool.close().await;

    let options = PgConnectOptions::from_str(&database_url)
        .unwrap_or_else(|error| panic!("test database URL should parse: {error}"))
        .options([("search_path", schema.as_str())]);
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap_or_else(|error| panic!("test schema database should be reachable: {error}"));

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .unwrap_or_else(|error| panic!("user-service migrations should run: {error}"));

    Some(pool)
}

fn unique_id(prefix: &str) -> String {
    format!("{prefix}-{}", Ulid::new())
}

fn unique_schema(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|error| panic!("system time should be after unix epoch: {error}"))
        .as_nanos();
    format!("{prefix}_{nanos}")
}
