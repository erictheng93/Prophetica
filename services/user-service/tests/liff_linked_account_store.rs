#![allow(clippy::expect_used)]

use std::{
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

use sqlx::{
    PgPool,
    postgres::{PgConnectOptions, PgPoolOptions},
};
use user_service::auth::{LiffLinkedAccountStore, PostgresLiffLinkedAccountStore};

#[tokio::test]
async fn postgres_liff_linked_account_store_finds_active_line_id_member() {
    let Some(pool) = postgres_test_pool().await else {
        return;
    };
    insert_member(&pool, "U-LIFF-PG-ACTIVE", "Uline-active", None).await;
    insert_member(&pool, "U-LIFF-PG-DELETED", "Uline-deleted", Some("now()")).await;

    let store = PostgresLiffLinkedAccountStore::new(pool);
    let active = store
        .account_by_line_user_id("Uline-active")
        .await
        .expect("active line_id lookup should run")
        .expect("active line_id should resolve");
    let deleted = store
        .account_by_line_user_id("Uline-deleted")
        .await
        .expect("deleted line_id lookup should run");
    let missing = store
        .account_by_line_user_id("Uline-missing")
        .await
        .expect("missing line_id lookup should run");

    assert_eq!(active.user_id, "U-LIFF-PG-ACTIVE");
    assert_eq!(active.line_user_id, "Uline-active");
    assert_eq!(active.email, "U-LIFF-PG-ACTIVE@example.com");
    assert_eq!(active.phone, "0912345621");
    assert!(deleted.is_none());
    assert!(missing.is_none());
}

async fn insert_member(pool: &PgPool, user_id: &str, line_id: &str, deleted_at_sql: Option<&str>) {
    let deleted_at = deleted_at_sql.unwrap_or("NULL");
    let sql = format!(
        r"
        INSERT INTO user_members (
            user_id, display_name, status, tier, email, phone, line_id, country,
            timezone, kyc_status, risk_level, risk_score, restrictions,
            device_count, two_factor_enabled, deleted_at
        )
        VALUES (
            $1, $2, 'active', 'Tier 0', $3, '0912345621', $4, 'TW',
            'Asia/Taipei', 'verified', 'low', 10, '[]'::jsonb,
            0, false, {deleted_at}
        )
        ",
    );
    sqlx::query(&sql)
        .bind(user_id)
        .bind(format!("LIFF {user_id}"))
        .bind(format!("{user_id}@example.com"))
        .bind(line_id)
        .execute(pool)
        .await
        .expect("member fixture should insert");
}

async fn postgres_test_pool() -> Option<PgPool> {
    let Some(database_url) = std::env::var("USER_SERVICE_TEST_DATABASE_URL").ok() else {
        eprintln!(
            "skipping liff linked account store test: USER_SERVICE_TEST_DATABASE_URL is not set"
        );
        return None;
    };
    let bootstrap_pool = PgPool::connect(&database_url)
        .await
        .unwrap_or_else(|error| panic!("test database should be reachable: {error}"));
    let schema = unique_schema("liff_linked_account_store");
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

fn unique_schema(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|error| panic!("system time should be after unix epoch: {error}"))
        .as_nanos();
    format!("{prefix}_{nanos}")
}
