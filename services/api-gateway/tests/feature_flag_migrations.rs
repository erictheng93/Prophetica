#![allow(clippy::expect_used)]

use sdk::postgres_migrations::{API_GATEWAY_MIGRATION_TABLE, run_postgres_migrations};
use sqlx::{
    PgPool, Row,
    postgres::{PgConnectOptions, PgPoolOptions},
};
use std::{
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

#[tokio::test]
async fn api_gateway_migrations_create_feature_flags_on_clean_database() {
    let Some(pool) = postgres_test_pool("api_gateway_feature_flags_clean").await else {
        return;
    };

    run_api_gateway_migrations(&pool).await;

    let payments_enabled: bool =
        sqlx::query_scalar("SELECT enabled FROM feature_flags WHERE key = $1")
            .bind("payments.real_money_enabled")
            .fetch_one(&pool)
            .await
            .expect("payments.real_money_enabled row should exist");
    let engine_enabled: bool =
        sqlx::query_scalar("SELECT enabled FROM feature_flags WHERE key = $1")
            .bind("engine.halted")
            .fetch_one(&pool)
            .await
            .expect("engine.halted row should exist");
    let metadata_default: serde_json::Value =
        sqlx::query_scalar("SELECT metadata FROM feature_flags WHERE key = $1")
            .bind("payments.real_money_enabled")
            .fetch_one(&pool)
            .await
            .expect("metadata column should exist");

    assert!(!payments_enabled);
    assert!(!engine_enabled);
    assert_eq!(metadata_default, serde_json::json!({}));
}

#[tokio::test]
async fn api_gateway_and_wallet_migrations_are_order_independent() {
    let Some(api_first_pool) = postgres_test_pool("api_gateway_feature_flags_api_first").await
    else {
        return;
    };

    run_api_gateway_migrations(&api_first_pool).await;
    sqlx::migrate!("../wallet-ledger-service/migrations")
        .run(&api_first_pool)
        .await
        .expect("wallet-ledger migrations should run after api-gateway migrations");
    assert_feature_flags_schema(&api_first_pool).await;

    let wallet_first_pool = postgres_test_pool("api_gateway_feature_flags_wallet_first")
        .await
        .expect("database URL was present for first migration order");
    sqlx::migrate!("../wallet-ledger-service/migrations")
        .run(&wallet_first_pool)
        .await
        .expect("wallet-ledger migrations should run before api-gateway migrations");
    run_api_gateway_migrations(&wallet_first_pool).await;
    assert_feature_flags_schema(&wallet_first_pool).await;
}

async fn run_api_gateway_migrations(pool: &PgPool) {
    run_postgres_migrations(
        sqlx::migrate!("../api-gateway/migrations"),
        pool,
        API_GATEWAY_MIGRATION_TABLE,
    )
    .await
    .expect("api-gateway migrations should run");
}

async fn assert_feature_flags_schema(pool: &PgPool) {
    let columns = sqlx::query(
        r"
        SELECT column_name, data_type, is_nullable, column_default
        FROM information_schema.columns
        WHERE table_name = 'feature_flags'
        ORDER BY ordinal_position
        ",
    )
    .fetch_all(pool)
    .await
    .expect("feature_flags columns should be introspectable");

    let observed = columns
        .iter()
        .map(|row| {
            (
                row.get::<String, _>("column_name"),
                row.get::<String, _>("data_type"),
                row.get::<String, _>("is_nullable"),
                row.get::<Option<String>, _>("column_default"),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        observed,
        vec![
            ("key".to_owned(), "text".to_owned(), "NO".to_owned(), None,),
            (
                "enabled".to_owned(),
                "boolean".to_owned(),
                "NO".to_owned(),
                Some("false".to_owned()),
            ),
            (
                "subject_id".to_owned(),
                "text".to_owned(),
                "YES".to_owned(),
                None,
            ),
            (
                "reason".to_owned(),
                "text".to_owned(),
                "YES".to_owned(),
                None,
            ),
            (
                "updated_by".to_owned(),
                "text".to_owned(),
                "NO".to_owned(),
                None,
            ),
            (
                "updated_at".to_owned(),
                "timestamp with time zone".to_owned(),
                "NO".to_owned(),
                Some("now()".to_owned()),
            ),
            (
                "metadata".to_owned(),
                "jsonb".to_owned(),
                "NO".to_owned(),
                Some("'{}'::jsonb".to_owned()),
            ),
        ]
    );

    let constraint_count: i64 = sqlx::query_scalar(
        r"
        SELECT count(*)
        FROM pg_constraint
        WHERE conname = 'feature_flags_engine_halted_subject'
          AND conrelid = 'feature_flags'::regclass
        ",
    )
    .fetch_one(pool)
    .await
    .expect("feature_flags check constraint should be introspectable");

    assert_eq!(constraint_count, 1);
}

async fn postgres_test_pool(prefix: &str) -> Option<PgPool> {
    let Some(database_url) = std::env::var("USER_SERVICE_TEST_DATABASE_URL").ok() else {
        eprintln!(
            "skipping api-gateway feature flag migration test: USER_SERVICE_TEST_DATABASE_URL is not set"
        );
        return None;
    };
    let bootstrap_pool = PgPool::connect(&database_url)
        .await
        .expect("test database should be reachable");
    let schema = unique_test_schema(prefix);
    sqlx::query(&format!("CREATE SCHEMA IF NOT EXISTS {schema}"))
        .execute(&bootstrap_pool)
        .await
        .expect("test schema should be created");
    bootstrap_pool.close().await;

    let options = PgConnectOptions::from_str(&database_url)
        .expect("test database URL should parse")
        .options([("search_path", schema.as_str())]);

    Some(
        PgPoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("test schema database should be reachable"),
    )
}

fn unique_test_schema(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    format!("{prefix}_{nanos}")
}
