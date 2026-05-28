//! Postgres-backed `RedemptionStore`. SPEC-024 implementation plan §1 P3.4b.
//!
//! Mirrors the schema defined by the redemption-service migrations:
//! - `202605270010_create_redemption_schema.sql` — base tables
//! - `202605280001_user_scope_idempotency.sql` — `(user_id, idempotency_key)` UNIQUE
//!
//! Notes on transactional discipline:
//! - `record_order` uses `INSERT ... ON CONFLICT DO NOTHING` against the
//!   `(user_id, idempotency_key)` unique constraint, then refetches. This
//!   makes idempotent replay safe under concurrent submits — the second
//!   inserter no-ops and reads the row the first one wrote.
//! - State / payload updates use plain `UPDATE` statements; they are NOT
//!   bundled into a single tx with the ledger write. The orchestrator's
//!   self-healing forward drive (see [`crate::service::RedemptionService`])
//!   is what guarantees consistency across the redemption + ledger DBs;
//!   wrapping anything in cross-service transactions would re-introduce the
//!   two-phase commit problem.

use std::str::FromStr;

use async_trait::async_trait;
use serde_json::Value;
use sqlx::PgPool;

use crate::{
    catalog::{
        CatalogItem, CatalogItemKind, CatalogItemStatus, FulfillmentMode, Sku, Stock, StockKind,
    },
    entitlement::{Entitlement, EntitlementState},
    order::{OrderState, RedemptionOrder},
    store::{RedemptionStore, StoreError},
};

#[derive(Debug, Clone)]
pub struct PgRedemptionStore {
    pool: PgPool,
}

impl PgRedemptionStore {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl RedemptionStore for PgRedemptionStore {
    async fn upsert_catalog_item(&self, item: CatalogItem) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO catalog_items (item_id, kind, status, category, title) \
             VALUES ($1, $2, $3, $4, $5) \
             ON CONFLICT (item_id) DO UPDATE SET \
                kind = EXCLUDED.kind, \
                status = EXCLUDED.status, \
                category = EXCLUDED.category, \
                title = EXCLUDED.title, \
                updated_at = NOW()",
        )
        .bind(&item.item_id)
        .bind(catalog_item_kind_to_str(item.kind))
        .bind(catalog_item_status_to_str(item.status))
        .bind(&item.category)
        .bind(&item.title)
        .execute(&self.pool)
        .await
        .map_err(|err| map_sqlx_error(&err))?;
        Ok(())
    }

    async fn upsert_sku(&self, sku: Sku) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO skus (sku_id, item_id, price_minor_units, fulfillment_mode, entitlement_ttl_days) \
             VALUES ($1, $2, $3, $4, $5) \
             ON CONFLICT (sku_id) DO UPDATE SET \
                item_id = EXCLUDED.item_id, \
                price_minor_units = EXCLUDED.price_minor_units, \
                fulfillment_mode = EXCLUDED.fulfillment_mode, \
                entitlement_ttl_days = EXCLUDED.entitlement_ttl_days, \
                updated_at = NOW()",
        )
        .bind(&sku.sku_id)
        .bind(&sku.item_id)
        .bind(sku.price_minor_units)
        .bind(fulfillment_mode_to_str(sku.fulfillment_mode))
        .bind(i64::from(sku.entitlement_ttl_days))
        .execute(&self.pool)
        .await
        .map_err(|err| map_sqlx_error(&err))?;
        Ok(())
    }

    async fn upsert_stock(&self, stock: Stock) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO sku_stock (sku_id, kind, available_qty, reservation_ttl_seconds) \
             VALUES ($1, $2, $3, $4) \
             ON CONFLICT (sku_id) DO UPDATE SET \
                kind = EXCLUDED.kind, \
                available_qty = EXCLUDED.available_qty, \
                reservation_ttl_seconds = EXCLUDED.reservation_ttl_seconds, \
                updated_at = NOW()",
        )
        .bind(&stock.sku_id)
        .bind(stock_kind_to_str(stock.kind))
        .bind(stock.available_qty)
        .bind(i64::from(stock.reservation_ttl_seconds))
        .execute(&self.pool)
        .await
        .map_err(|err| map_sqlx_error(&err))?;
        Ok(())
    }

    async fn get_sku(&self, sku_id: &str) -> Result<Option<Sku>, StoreError> {
        let row: Option<(String, String, i64, String, i32)> = sqlx::query_as(
            "SELECT item_id, sku_id, price_minor_units, fulfillment_mode, entitlement_ttl_days \
             FROM skus WHERE sku_id = $1",
        )
        .bind(sku_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| map_sqlx_error(&err))?;

        match row {
            None => Ok(None),
            Some((item_id, sku_id, price, mode, ttl)) => {
                let mode = fulfillment_mode_from_str(&mode)?;
                let ttl = u32::try_from(ttl).map_err(|_| StoreError::OrderNotFound {
                    order_id: format!("invalid ttl for sku {sku_id}"),
                })?;
                let sku = Sku::new(sku_id, item_id, price, mode, ttl).map_err(|err| {
                    StoreError::OrderNotFound {
                        order_id: format!("catalog invariant violated: {err}"),
                    }
                })?;
                Ok(Some(sku))
            }
        }
    }

    async fn record_order(&self, order: RedemptionOrder) -> Result<RedemptionOrder, StoreError> {
        // ON CONFLICT DO NOTHING returns affected = 0 for the dup case; we
        // then SELECT the prior row. UNIQUE is on (user_id, idempotency_key)
        // per migration 202605280001, so cross-user collisions are
        // impossible at the DB layer.
        sqlx::query(
            "INSERT INTO redemption_orders \
                (order_id, user_id, sku_id, price_minor_units, idempotency_key, state) \
             VALUES ($1, $2, $3, $4, $5, $6) \
             ON CONFLICT (user_id, idempotency_key) DO NOTHING",
        )
        .bind(&order.order_id)
        .bind(&order.user_id)
        .bind(&order.sku_id)
        .bind(order.price_minor_units)
        .bind(&order.idempotency_key)
        .bind(order_state_to_str(order.state))
        .execute(&self.pool)
        .await
        .map_err(|err| map_sqlx_error(&err))?;

        // Re-read by (user, key) to get the canonical order — whether ours
        // (first writer) or the prior row (replay).
        let row: (String, String, String, i64, String, String) = sqlx::query_as(
            "SELECT order_id, user_id, sku_id, price_minor_units, idempotency_key, state \
             FROM redemption_orders WHERE user_id = $1 AND idempotency_key = $2",
        )
        .bind(&order.user_id)
        .bind(&order.idempotency_key)
        .fetch_one(&self.pool)
        .await
        .map_err(|err| map_sqlx_error(&err))?;

        Ok(RedemptionOrder {
            order_id: row.0,
            user_id: row.1,
            sku_id: row.2,
            price_minor_units: row.3,
            idempotency_key: row.4,
            state: order_state_from_str(&row.5)?,
        })
    }

    async fn update_order_state(
        &self,
        order_id: &str,
        new_state: OrderState,
    ) -> Result<(), StoreError> {
        let result = sqlx::query(
            "UPDATE redemption_orders SET state = $1, updated_at = NOW() WHERE order_id = $2",
        )
        .bind(order_state_to_str(new_state))
        .bind(order_id)
        .execute(&self.pool)
        .await
        .map_err(|err| map_sqlx_error(&err))?;
        if result.rows_affected() == 0 {
            return Err(StoreError::OrderNotFound {
                order_id: order_id.to_owned(),
            });
        }
        Ok(())
    }

    async fn get_order(&self, order_id: &str) -> Result<Option<RedemptionOrder>, StoreError> {
        let row: Option<(String, String, String, i64, String, String)> = sqlx::query_as(
            "SELECT order_id, user_id, sku_id, price_minor_units, idempotency_key, state \
             FROM redemption_orders WHERE order_id = $1",
        )
        .bind(order_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| map_sqlx_error(&err))?;
        Ok(match row {
            None => None,
            Some(row) => Some(RedemptionOrder {
                order_id: row.0,
                user_id: row.1,
                sku_id: row.2,
                price_minor_units: row.3,
                idempotency_key: row.4,
                state: order_state_from_str(&row.5)?,
            }),
        })
    }

    async fn record_entitlement(&self, entitlement: Entitlement) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO entitlements \
                (entitlement_id, order_id, owner_user_id, sku_id, state, metadata) \
             VALUES ($1, $2, $3, $4, $5, $6) \
             ON CONFLICT (entitlement_id) DO NOTHING",
        )
        .bind(&entitlement.entitlement_id)
        .bind(&entitlement.order_id)
        .bind(&entitlement.owner_user_id)
        .bind(&entitlement.sku_id)
        .bind(entitlement_state_to_str(entitlement.state))
        .bind(
            entitlement
                .fulfillment_payload
                .clone()
                .unwrap_or(Value::Object(serde_json::Map::new())),
        )
        .execute(&self.pool)
        .await
        .map_err(|err| map_sqlx_error(&err))?;
        Ok(())
    }

    async fn update_entitlement_state(
        &self,
        entitlement_id: &str,
        new_state: EntitlementState,
    ) -> Result<(), StoreError> {
        let result = sqlx::query("UPDATE entitlements SET state = $1 WHERE entitlement_id = $2")
            .bind(entitlement_state_to_str(new_state))
            .bind(entitlement_id)
            .execute(&self.pool)
            .await
            .map_err(|err| map_sqlx_error(&err))?;
        if result.rows_affected() == 0 {
            return Err(StoreError::EntitlementNotFound {
                entitlement_id: entitlement_id.to_owned(),
            });
        }
        Ok(())
    }

    async fn get_entitlement_for_order(
        &self,
        order_id: &str,
    ) -> Result<Option<Entitlement>, StoreError> {
        let row: Option<(String, String, String, String, String, Value)> = sqlx::query_as(
            "SELECT entitlement_id, order_id, owner_user_id, sku_id, state, metadata \
             FROM entitlements WHERE order_id = $1",
        )
        .bind(order_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| map_sqlx_error(&err))?;
        Ok(match row {
            None => None,
            Some(row) => Some(Entitlement {
                entitlement_id: row.0,
                order_id: row.1,
                owner_user_id: row.2,
                sku_id: row.3,
                state: entitlement_state_from_str(&row.4)?,
                fulfillment_payload: payload_or_none(row.5),
            }),
        })
    }

    async fn list_entitlements_for_owner(
        &self,
        owner_user_id: &str,
    ) -> Result<Vec<Entitlement>, StoreError> {
        let rows: Vec<(String, String, String, String, String, Value)> = sqlx::query_as(
            "SELECT entitlement_id, order_id, owner_user_id, sku_id, state, metadata \
             FROM entitlements WHERE owner_user_id = $1 \
             ORDER BY entitlement_id",
        )
        .bind(owner_user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|err| map_sqlx_error(&err))?;
        rows.into_iter()
            .map(|row| {
                Ok(Entitlement {
                    entitlement_id: row.0,
                    order_id: row.1,
                    owner_user_id: row.2,
                    sku_id: row.3,
                    state: entitlement_state_from_str(&row.4)?,
                    fulfillment_payload: payload_or_none(row.5),
                })
            })
            .collect()
    }

    async fn list_entitlements_pending_fulfillment(
        &self,
        mode: FulfillmentMode,
        limit: usize,
    ) -> Result<Vec<Entitlement>, StoreError> {
        let limit_i64 = i64::try_from(limit).unwrap_or(i64::MAX);
        let rows: Vec<(String, String, String, String, String, Value)> = sqlx::query_as(
            "SELECT e.entitlement_id, e.order_id, e.owner_user_id, e.sku_id, e.state, e.metadata \
             FROM entitlements e \
             JOIN skus s ON s.sku_id = e.sku_id \
             WHERE e.state = 'issued' AND s.fulfillment_mode = $1 \
             ORDER BY e.entitlement_id \
             LIMIT $2",
        )
        .bind(fulfillment_mode_to_str(mode))
        .bind(limit_i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|err| map_sqlx_error(&err))?;
        rows.into_iter()
            .map(|row| {
                Ok(Entitlement {
                    entitlement_id: row.0,
                    order_id: row.1,
                    owner_user_id: row.2,
                    sku_id: row.3,
                    state: entitlement_state_from_str(&row.4)?,
                    fulfillment_payload: payload_or_none(row.5),
                })
            })
            .collect()
    }

    async fn set_entitlement_payload(
        &self,
        entitlement_id: &str,
        payload: Value,
    ) -> Result<(), StoreError> {
        let result = sqlx::query("UPDATE entitlements SET metadata = $1 WHERE entitlement_id = $2")
            .bind(payload)
            .bind(entitlement_id)
            .execute(&self.pool)
            .await
            .map_err(|err| map_sqlx_error(&err))?;
        if result.rows_affected() == 0 {
            return Err(StoreError::EntitlementNotFound {
                entitlement_id: entitlement_id.to_owned(),
            });
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Enum ↔ DB string mappings. Kept in one place so the CHECK constraints in
// the migration and the Rust enums never silently drift.
// ---------------------------------------------------------------------------

fn catalog_item_kind_to_str(kind: CatalogItemKind) -> &'static str {
    match kind {
        CatalogItemKind::Digital => "digital",
        CatalogItemKind::Service => "service",
        CatalogItemKind::Physical => "physical",
    }
}

fn catalog_item_status_to_str(status: CatalogItemStatus) -> &'static str {
    match status {
        CatalogItemStatus::Draft => "draft",
        CatalogItemStatus::Active => "active",
        CatalogItemStatus::Retired => "retired",
    }
}

fn fulfillment_mode_to_str(mode: FulfillmentMode) -> &'static str {
    match mode {
        FulfillmentMode::DigitalImmediate => "digital_immediate",
        FulfillmentMode::DigitalCode => "digital_code",
        FulfillmentMode::ServiceAsync => "service_async",
    }
}

fn fulfillment_mode_from_str(s: &str) -> Result<FulfillmentMode, StoreError> {
    match s {
        "digital_immediate" => Ok(FulfillmentMode::DigitalImmediate),
        "digital_code" => Ok(FulfillmentMode::DigitalCode),
        "service_async" => Ok(FulfillmentMode::ServiceAsync),
        other => Err(StoreError::OrderNotFound {
            order_id: format!("invalid fulfillment_mode {other:?} in DB"),
        }),
    }
}

fn stock_kind_to_str(kind: StockKind) -> &'static str {
    match kind {
        StockKind::Unlimited => "unlimited",
        StockKind::Finite => "finite",
        StockKind::PartnerManaged => "partner_managed",
    }
}

fn order_state_to_str(state: OrderState) -> &'static str {
    match state {
        OrderState::Submitted => "submitted",
        OrderState::Accepted => "accepted",
        OrderState::Committed => "committed",
        OrderState::Fulfilled => "fulfilled",
        OrderState::Refunded => "refunded",
        OrderState::Expired => "expired",
    }
}

fn order_state_from_str(s: &str) -> Result<OrderState, StoreError> {
    match s {
        "submitted" => Ok(OrderState::Submitted),
        "accepted" => Ok(OrderState::Accepted),
        "committed" => Ok(OrderState::Committed),
        "fulfilled" => Ok(OrderState::Fulfilled),
        "refunded" => Ok(OrderState::Refunded),
        "expired" => Ok(OrderState::Expired),
        other => Err(StoreError::OrderNotFound {
            order_id: format!("invalid order state {other:?} in DB"),
        }),
    }
}

fn entitlement_state_to_str(state: EntitlementState) -> &'static str {
    match state {
        EntitlementState::Issued => "issued",
        EntitlementState::Fulfilled => "fulfilled",
        EntitlementState::Consumed => "consumed",
        EntitlementState::Refunded => "refunded",
        EntitlementState::Expired => "expired",
    }
}

fn entitlement_state_from_str(s: &str) -> Result<EntitlementState, StoreError> {
    match s {
        "issued" => Ok(EntitlementState::Issued),
        "fulfilled" => Ok(EntitlementState::Fulfilled),
        "consumed" => Ok(EntitlementState::Consumed),
        "refunded" => Ok(EntitlementState::Refunded),
        "expired" => Ok(EntitlementState::Expired),
        other => Err(StoreError::EntitlementNotFound {
            entitlement_id: format!("invalid entitlement state {other:?} in DB"),
        }),
    }
}

/// `serde_json` returns `Value::Object({})` for the DB DEFAULT `'{}'::jsonb`.
/// We treat empty objects as semantically "no payload yet" so the
/// orchestrator's `payload.is_some()` check (which gates re-claim avoidance)
/// behaves the same after a row round-trip through PG.
fn payload_or_none(value: Value) -> Option<Value> {
    match value {
        Value::Object(ref map) if map.is_empty() => None,
        Value::Null => None,
        other => Some(other),
    }
}

fn map_sqlx_error(err: &sqlx::Error) -> StoreError {
    // P3.4b skeleton: collapse every transport / SQL error to Poisoned so
    // callers handle it uniformly. A richer StoreError variant set (e.g.
    // SqlxTransport, Conflict) is fair game once we have real production
    // patterns to model.
    tracing::error!(?err, "PgRedemptionStore sqlx error");
    StoreError::Poisoned
}

// Re-exported via a tiny helper so binary callers can construct a pool with
// the same options the integration tests use.
//
/// Convenience: build a `PgPool` from a `DATABASE_URL` with sensible
/// defaults for the redemption-service workload. Caller decides whether to
/// run migrations (`sqlx::migrate!`) after this.
///
/// # Errors
///
/// Propagates `sqlx::Error` from the connect step.
pub async fn connect_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    let options = sqlx::postgres::PgConnectOptions::from_str(database_url)?;
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(8)
        .connect_with(options)
        .await
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::{
        PgRedemptionStore, catalog_item_kind_to_str, entitlement_state_from_str,
        entitlement_state_to_str, fulfillment_mode_from_str, fulfillment_mode_to_str,
        order_state_from_str, order_state_to_str, payload_or_none, stock_kind_to_str,
    };
    use crate::{
        catalog::{CatalogItem, CatalogItemKind, FulfillmentMode, Sku, Stock, StockKind},
        entitlement::{Entitlement, EntitlementState},
        order::{OrderState, RedemptionOrder},
        store::RedemptionStore,
    };
    use serde_json::{Value, json};
    use sqlx::PgPool;
    use std::str::FromStr;

    // -------------------------------------------------------------------
    // Pure-unit enum round-trip tests (no DB needed).
    // -------------------------------------------------------------------

    #[test]
    fn fulfillment_mode_string_round_trip() {
        for mode in [
            FulfillmentMode::DigitalImmediate,
            FulfillmentMode::DigitalCode,
            FulfillmentMode::ServiceAsync,
        ] {
            let s = fulfillment_mode_to_str(mode);
            let parsed = fulfillment_mode_from_str(s).expect("known string");
            assert_eq!(parsed, mode);
        }
    }

    #[test]
    fn order_state_string_round_trip_matches_schema_check() {
        // Mirrors the CHECK in 202605270010_create_redemption_schema.sql.
        let allowed = [
            "submitted",
            "accepted",
            "committed",
            "fulfilled",
            "refunded",
            "expired",
        ];
        for s in allowed {
            let state = order_state_from_str(s).expect("known string");
            assert_eq!(order_state_to_str(state), s);
        }
    }

    #[test]
    fn entitlement_state_string_round_trip_matches_schema_check() {
        let allowed = ["issued", "fulfilled", "consumed", "refunded", "expired"];
        for s in allowed {
            let state = entitlement_state_from_str(s).expect("known string");
            assert_eq!(entitlement_state_to_str(state), s);
        }
    }

    #[test]
    fn stock_kind_matches_schema_check() {
        for (kind, expected) in [
            (StockKind::Unlimited, "unlimited"),
            (StockKind::Finite, "finite"),
            (StockKind::PartnerManaged, "partner_managed"),
        ] {
            assert_eq!(stock_kind_to_str(kind), expected);
        }
    }

    #[test]
    fn catalog_item_kind_matches_schema_check() {
        // The schema CHECK excludes "physical" — but the mapping must still
        // exist for the case where we ingest a legacy row.
        assert_eq!(
            catalog_item_kind_to_str(CatalogItemKind::Digital),
            "digital"
        );
        assert_eq!(
            catalog_item_kind_to_str(CatalogItemKind::Service),
            "service"
        );
        assert_eq!(
            catalog_item_kind_to_str(CatalogItemKind::Physical),
            "physical"
        );
    }

    #[test]
    fn payload_or_none_treats_empty_object_as_no_payload() {
        assert_eq!(payload_or_none(Value::Object(serde_json::Map::new())), None);
        assert_eq!(payload_or_none(Value::Null), None);
        let kept = json!({"code": "ABC"});
        assert_eq!(payload_or_none(kept.clone()), Some(kept));
    }

    // -------------------------------------------------------------------
    // PG integration tests. Skipped silently when
    // REDEMPTION_SERVICE_TEST_DATABASE_URL is not set, matching the pattern
    // in notification-service / wallet-ledger-service.
    // -------------------------------------------------------------------

    async fn postgres_test_pool() -> Option<PgPool> {
        let database_url = std::env::var("REDEMPTION_SERVICE_TEST_DATABASE_URL").ok()?;
        let bootstrap = PgPool::connect(&database_url)
            .await
            .expect("test database should be reachable");
        let schema = format!(
            "redemption_service_{}",
            ulid::Ulid::new().to_string().to_lowercase()
        );
        sqlx::query(&format!("CREATE SCHEMA IF NOT EXISTS {schema}"))
            .execute(&bootstrap)
            .await
            .expect("test schema should be created");
        bootstrap.close().await;
        let options = sqlx::postgres::PgConnectOptions::from_str(&database_url)
            .expect("database url parses")
            .options([("search_path", schema.as_str())]);
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(2)
            .connect_with(options)
            .await
            .expect("schema-scoped pool");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("redemption-service migrations should run");
        Some(pool)
    }

    #[tokio::test]
    async fn pg_round_trip_order_and_entitlement() {
        let Some(pool) = postgres_test_pool().await else {
            return;
        };
        let store = PgRedemptionStore::new(pool);

        let item = CatalogItem::new("item_a", CatalogItemKind::Digital, "cosmetic", "Badge")
            .expect("item");
        let sku = Sku::new(
            "sku_a",
            "item_a",
            500,
            FulfillmentMode::DigitalImmediate,
            30,
        )
        .expect("sku");
        let stock = Stock {
            sku_id: "sku_a".to_owned(),
            kind: StockKind::Unlimited,
            available_qty: 0,
            reservation_ttl_seconds: 300,
        };
        store.upsert_catalog_item(item).await.expect("item");
        store.upsert_sku(sku.clone()).await.expect("sku");
        store.upsert_stock(stock).await.expect("stock");

        let loaded_sku = store.get_sku("sku_a").await.expect("get").expect("present");
        assert_eq!(loaded_sku.price_minor_units, sku.price_minor_units);

        let order = RedemptionOrder::submit("user_42", "sku_a", 500, "client-key").expect("submit");
        let stored = store
            .record_order(order.clone())
            .await
            .expect("record order");
        assert_eq!(stored.order_id, order.order_id);

        // Replay returns the same order_id even when the input draft has a
        // different generated order_id (idempotency).
        let replay_draft =
            RedemptionOrder::submit("user_42", "sku_a", 500, "client-key").expect("replay");
        let replay = store
            .record_order(replay_draft)
            .await
            .expect("replay record");
        assert_eq!(replay.order_id, order.order_id);

        store
            .update_order_state(&order.order_id, OrderState::Accepted)
            .await
            .expect("advance");
        let advanced = store
            .get_order(&order.order_id)
            .await
            .expect("get")
            .expect("present");
        assert_eq!(advanced.state, OrderState::Accepted);

        let entitlement =
            Entitlement::issue(order.order_id.clone(), "user_42", "sku_a").expect("issue");
        store
            .record_entitlement(entitlement.clone())
            .await
            .expect("record");
        store
            .set_entitlement_payload(&entitlement.entitlement_id, json!({"code": "AAA-001"}))
            .await
            .expect("payload");
        let healed = store
            .get_entitlement_for_order(&order.order_id)
            .await
            .expect("get")
            .expect("present");
        assert_eq!(
            healed
                .fulfillment_payload
                .as_ref()
                .and_then(|v| v.get("code"))
                .and_then(|v| v.as_str()),
            Some("AAA-001")
        );

        // list_entitlements_pending_fulfillment is mode-aware via JOIN.
        store
            .update_entitlement_state(&entitlement.entitlement_id, EntitlementState::Issued)
            .await
            .expect("reset state");
        let pending = store
            .list_entitlements_pending_fulfillment(FulfillmentMode::DigitalImmediate, 10)
            .await
            .expect("list pending");
        assert_eq!(pending.len(), 1);
    }

    #[tokio::test]
    async fn pg_cross_user_idempotency_scoping() {
        let Some(pool) = postgres_test_pool().await else {
            return;
        };
        let store = PgRedemptionStore::new(pool);

        let item =
            CatalogItem::new("item_a", CatalogItemKind::Digital, "cat", "Badge").expect("item");
        let sku = Sku::new(
            "sku_a",
            "item_a",
            100,
            FulfillmentMode::DigitalImmediate,
            30,
        )
        .expect("sku");
        store.upsert_catalog_item(item).await.expect("item");
        store.upsert_sku(sku).await.expect("sku");

        // Same idempotency_key under two different user_ids must produce
        // two distinct order_ids — codex P3.1 P1 #3 regression guard at the
        // schema layer (UNIQUE (user_id, idempotency_key)).
        let alice =
            RedemptionOrder::submit("user_alice", "sku_a", 100, "shared-key").expect("alice");
        let bob = RedemptionOrder::submit("user_bob", "sku_a", 100, "shared-key").expect("bob");
        let alice_stored = store.record_order(alice.clone()).await.expect("alice");
        let bob_stored = store.record_order(bob.clone()).await.expect("bob");
        assert_eq!(alice_stored.order_id, alice.order_id);
        assert_eq!(bob_stored.order_id, bob.order_id);
        assert_ne!(alice_stored.order_id, bob_stored.order_id);
    }
}
