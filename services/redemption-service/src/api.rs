//! HTTP surface for the redemption-service.
//!
//! SPEC-024 implementation plan §1 P3.1: `submit_order` / `get_order` /
//! `list_entitlements` are real handlers backed by [`crate::service::RedemptionService`].
//! The `cancel_order` route still returns 501 — cancel-window refund lands
//! with P4 (`expiry / breakage / refund`).

use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Serialize;
use serde_json::{Value, json};
use tracing::warn;

use crate::{
    entitlement::Entitlement,
    service::{RedemptionError, RedemptionService, SubmitOrderRequest},
};

/// Builds the redemption-service HTTP router. The `service` argument is
/// shared across all handlers as axum `State`.
pub fn router(service: Arc<RedemptionService>) -> Router {
    Router::new()
        .route("/v1/redemption/orders", post(submit_order))
        .route("/v1/redemption/orders/:order_id", get(get_order))
        .route(
            "/v1/redemption/orders/:order_id/cancel",
            post(stub_cancel_order),
        )
        .route(
            "/v1/redemption/entitlements/:user_id",
            get(list_entitlements),
        )
        .with_state(service)
}

async fn submit_order(
    State(service): State<Arc<RedemptionService>>,
    Json(request): Json<SubmitOrderRequest>,
) -> Response {
    match service.submit_order(request).await {
        Ok(view) => {
            let status = if view.replayed {
                StatusCode::OK
            } else {
                StatusCode::CREATED
            };
            (status, Json(view)).into_response()
        }
        Err(error) => render_redemption_error(&error),
    }
}

async fn get_order(
    State(service): State<Arc<RedemptionService>>,
    Path(order_id): Path<String>,
) -> Response {
    match service.get_order(&order_id).await {
        Ok(Some(view)) => (StatusCode::OK, Json(view)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "order_not_found", "order_id": order_id})),
        )
            .into_response(),
        Err(error) => render_redemption_error(&error),
    }
}

async fn list_entitlements(
    State(service): State<Arc<RedemptionService>>,
    Path(user_id): Path<String>,
) -> Response {
    match service.list_entitlements(&user_id).await {
        Ok(entitlements) => (
            StatusCode::OK,
            Json(EntitlementListResponse { entitlements }),
        )
            .into_response(),
        Err(error) => render_redemption_error(&error),
    }
}

async fn stub_cancel_order() -> (StatusCode, Json<Value>) {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({
            "error": "not_implemented",
            "handler": "redemption cancel-order",
            "plan_ref": "SPEC-024 implementation plan §1 P4: cancel-window refund + ledger redemption_refund txn live in the expiry/breakage/refund phase.",
        })),
    )
}

#[derive(Debug, Serialize)]
struct EntitlementListResponse {
    entitlements: Vec<Entitlement>,
}

fn render_redemption_error(error: &RedemptionError) -> Response {
    let (status, code) = match error {
        RedemptionError::EmptyField { .. } => (StatusCode::BAD_REQUEST, "empty_field"),
        RedemptionError::OrderBuild(_) => (StatusCode::BAD_REQUEST, "invalid_order"),
        RedemptionError::EntitlementBuild(_) => (StatusCode::BAD_REQUEST, "invalid_entitlement"),
        RedemptionError::SkuNotFound { .. } => (StatusCode::NOT_FOUND, "sku_not_found"),
        RedemptionError::InsufficientBalance { .. } => {
            (StatusCode::PAYMENT_REQUIRED, "insufficient_balance")
        }
        RedemptionError::LedgerRejected(_) => (StatusCode::CONFLICT, "ledger_rejected"),
        // Vault errors should normally be confined to worker passes (not
        // HTTP-facing). If we ever surface one here, treat as 503 — the
        // operational fix is re-seeding the vault, not retrying the request.
        RedemptionError::VaultRejected(_) => (StatusCode::SERVICE_UNAVAILABLE, "vault_exhausted"),
        // Partner errors likewise should stay in worker scope. If surfaced
        // here, treat as 503 — the operational fix is partner-side, not
        // client-retry.
        RedemptionError::PartnerRejected(_) => {
            (StatusCode::SERVICE_UNAVAILABLE, "partner_unavailable")
        }
        RedemptionError::SkuVanished { .. }
        | RedemptionError::StoreUnavailable(_)
        | RedemptionError::Internal { .. } => {
            warn!(?error, "redemption-service internal error");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal_error")
        }
    };
    (
        status,
        Json(json!({
            "error": code,
            "message": error.to_string(),
        })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::router;
    use crate::{
        catalog::{CatalogItem, CatalogItemKind, FulfillmentMode, Sku, Stock, StockKind},
        ledger::InMemoryLedgerClient,
        service::RedemptionService,
        store::{InMemoryRedemptionStore, RedemptionStore},
    };
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use std::sync::Arc;
    use tower::ServiceExt;

    async fn build_router_with_seeded_sku() -> axum::Router {
        let store = Arc::new(InMemoryRedemptionStore::new());
        let ledger = Arc::new(InMemoryLedgerClient::new());
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
        store.upsert_sku(sku).await.expect("sku");
        store.upsert_stock(stock).await.expect("stock");
        ledger
            .seed_deposit("user_42", 10_000)
            .await
            .expect("seed deposit");
        let service = RedemptionService::new(store as Arc<_>, ledger as Arc<_>);
        router(Arc::new(service))
    }

    async fn read_body_to_string(response: axum::response::Response) -> String {
        let body = response.into_body().collect().await.expect("collect body");
        String::from_utf8(body.to_bytes().to_vec()).expect("utf-8 body")
    }

    #[tokio::test]
    async fn submit_order_201_on_first_post() {
        let app = build_router_with_seeded_sku().await;
        let request = Request::builder()
            .uri("/v1/redemption/orders")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_vec(&serde_json::json!({
                    "user_id": "user_42",
                    "sku_id": "sku_a",
                    "idempotency_key": "k1",
                }))
                .expect("encode"),
            ))
            .expect("request");
        let response = app.oneshot(request).await.expect("oneshot");
        assert_eq!(response.status(), StatusCode::CREATED);
        let body = read_body_to_string(response).await;
        assert!(body.contains("\"state\":\"fulfilled\""));
        assert!(body.contains("\"replayed\":false"));
    }

    #[tokio::test]
    async fn submit_order_404_on_unknown_sku() {
        let app = build_router_with_seeded_sku().await;
        let request = Request::builder()
            .uri("/v1/redemption/orders")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_vec(&serde_json::json!({
                    "user_id": "user_42",
                    "sku_id": "ghost",
                    "idempotency_key": "k2",
                }))
                .expect("encode"),
            ))
            .expect("request");
        let response = app.oneshot(request).await.expect("oneshot");
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert!(
            read_body_to_string(response)
                .await
                .contains("sku_not_found")
        );
    }

    #[tokio::test]
    async fn submit_order_402_on_insufficient_balance() {
        // No deposit; balance = 0.
        let store = Arc::new(InMemoryRedemptionStore::new());
        let ledger = Arc::new(InMemoryLedgerClient::new());
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
        store.upsert_catalog_item(item).await.expect("item");
        store.upsert_sku(sku).await.expect("sku");
        let service = RedemptionService::new(store as Arc<_>, ledger as Arc<_>);
        let app = router(Arc::new(service));

        let request = Request::builder()
            .uri("/v1/redemption/orders")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_vec(&serde_json::json!({
                    "user_id": "user_42",
                    "sku_id": "sku_a",
                    "idempotency_key": "k3",
                }))
                .expect("encode"),
            ))
            .expect("request");
        let response = app.oneshot(request).await.expect("oneshot");
        assert_eq!(response.status(), StatusCode::PAYMENT_REQUIRED);
        assert!(
            read_body_to_string(response)
                .await
                .contains("insufficient_balance")
        );
    }

    #[tokio::test]
    async fn cancel_route_still_returns_501() {
        let app = build_router_with_seeded_sku().await;
        let request = Request::builder()
            .uri("/v1/redemption/orders/some-id/cancel")
            .method("POST")
            .body(Body::empty())
            .expect("request");
        let response = app.oneshot(request).await.expect("oneshot");
        assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
    }
}
