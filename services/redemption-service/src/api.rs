//! HTTP surface for the redemption-service.
//!
//! Routes:
//! - **Public:** `submit_order` / `get_order` / `list_entitlements` /
//!   `cancel_order` (under `/v1/redemption/...`). Gated by the API gateway's
//!   normal authenticated-user role check (SPEC-014 §6.1.2).
//! - **Admin (P5.6 two-stage CS refund):** POST
//!   `/v1/admin/redemption/orders/:order_id/refund-proposals` (proposer
//!   creates a proposal) and POST
//!   `/v1/admin/redemption/refund-proposals/:proposal_id/approve` (a
//!   distinct approver flips it to Approved + posts the refund). Each
//!   endpoint reads `x-user-id` from a gateway-asserted header — the
//!   service does NOT re-authenticate. The 4-eye invariant is enforced
//!   by loading the stored proposer and rejecting any approver where
//!   `approver_user_id == proposed_by`.
//! - **Operational (binary-only):** the expiry sweep
//!   (`run_expiry_sweep`) ships as `redemption-expiry-sweeper-binary`, not
//!   as an HTTP route.

use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, HeaderName, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::warn;

use crate::{
    entitlement::Entitlement,
    service::{RedemptionError, RedemptionService, SubmitOrderRequest},
};

/// Trusted authenticated-user header injected by the API gateway after
/// bearer-token verification. SPEC-014 §6.1.2; matches the
/// `api-gateway::lib::USER_ID_HEADER` constant. Any handler that needs to
/// know "who is calling" MUST read this header — never trust a user-id
/// field in the request body.
const AUTH_USER_ID_HEADER: HeaderName = HeaderName::from_static("x-user-id");

/// Builds the redemption-service HTTP router. The `service` argument is
/// shared across all handlers as axum `State`.
pub fn router(service: Arc<RedemptionService>) -> Router {
    Router::new()
        .route("/v1/redemption/orders", post(submit_order))
        .route("/v1/redemption/orders/:order_id", get(get_order))
        .route("/v1/redemption/orders/:order_id/cancel", post(cancel_order))
        .route(
            "/v1/redemption/entitlements/:user_id",
            get(list_entitlements),
        )
        // P5.6 two-stage CS refund: proposer creates proposal, approver
        // approves it. Each endpoint independently authenticates its
        // caller via x-user-id (P5.5).
        .route(
            "/v1/admin/redemption/orders/:order_id/refund-proposals",
            post(admin_propose_cs_refund),
        )
        .route(
            "/v1/admin/redemption/refund-proposals/:proposal_id/approve",
            post(admin_approve_cs_refund),
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

async fn cancel_order(
    State(service): State<Arc<RedemptionService>>,
    Path(order_id): Path<String>,
) -> Response {
    let now = crate::service::current_unix_secs();
    match service.cancel_order(&order_id, now).await {
        Ok(view) => (StatusCode::OK, Json(view)).into_response(),
        Err(error) => render_redemption_error(&error),
    }
}

/// Inbound body for the propose step of the SPEC-012 §11 two-stage CS
/// refund. The proposer identity is taken from the `x-user-id` header (P5.5),
/// not the body.
#[derive(Debug, Deserialize)]
struct ProposeCsRefundRequest {
    reason: String,
}

async fn admin_propose_cs_refund(
    State(service): State<Arc<RedemptionService>>,
    Path(order_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<ProposeCsRefundRequest>,
) -> Response {
    let Some(proposer_user_id) = extract_authenticated_user_id(&headers) else {
        return missing_auth_response();
    };
    if request.reason.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "empty_reason"})),
        )
            .into_response();
    }
    let now = crate::service::current_unix_secs();
    match service
        .propose_cs_refund(&order_id, &proposer_user_id, &request.reason, now)
        .await
    {
        Ok(proposal) => (StatusCode::CREATED, Json(proposal)).into_response(),
        Err(error) => render_redemption_error(&error),
    }
}

async fn admin_approve_cs_refund(
    State(service): State<Arc<RedemptionService>>,
    Path(proposal_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    // No body — approval is a pure verb on the proposal id. Approver
    // identity comes from the gateway-injected header.
    let Some(approver_user_id) = extract_authenticated_user_id(&headers) else {
        return missing_auth_response();
    };
    let now = crate::service::current_unix_secs();
    match service
        .approve_cs_refund(&proposal_id, &approver_user_id, now)
        .await
    {
        Ok(view) => (StatusCode::OK, Json(view)).into_response(),
        Err(error) => render_redemption_error(&error),
    }
}

fn extract_authenticated_user_id(headers: &HeaderMap) -> Option<String> {
    headers
        .get(&AUTH_USER_ID_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
}

fn missing_auth_response() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({
            "error": "missing_auth_context",
            "message": "x-user-id header must be set by the API gateway"
        })),
    )
        .into_response()
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
        RedemptionError::OrderNotFound { .. } => (StatusCode::NOT_FOUND, "order_not_found"),
        RedemptionError::OrderStateNotAllowed { .. } => {
            (StatusCode::CONFLICT, "order_state_not_allowed")
        }
        RedemptionError::CancelWindowExpired { .. } => (StatusCode::GONE, "cancel_window_expired"),
        RedemptionError::FourEyeViolation { .. } | RedemptionError::ApproverIsProposer { .. } => {
            (StatusCode::UNPROCESSABLE_ENTITY, "four_eye_violation")
        }
        RedemptionError::ProposalAlreadyResolved { .. } => {
            (StatusCode::CONFLICT, "proposal_already_resolved")
        }
        RedemptionError::ProposalBuild(_) => (StatusCode::BAD_REQUEST, "invalid_proposal"),
        RedemptionError::MissingCommitTimestamp { .. } => {
            warn!(?error, "redemption-service missing commit timestamp");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal_error")
        }
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
    async fn cancel_route_404s_for_unknown_order() {
        let app = build_router_with_seeded_sku().await;
        let request = Request::builder()
            .uri("/v1/redemption/orders/01HX-NOT-A-REAL-ORDER/cancel")
            .method("POST")
            .body(Body::empty())
            .expect("request");
        let response = app.oneshot(request).await.expect("oneshot");
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert!(
            read_body_to_string(response)
                .await
                .contains("order_not_found")
        );
    }

    async fn submit_then_propose_helper(
        app: &axum::Router,
        idempotency_key: &str,
        proposer: &str,
    ) -> String {
        // submit → Fulfilled → propose → return proposal_id.
        let submit = Request::builder()
            .uri("/v1/redemption/orders")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_vec(&serde_json::json!({
                    "user_id": "user_42",
                    "sku_id": "sku_a",
                    "idempotency_key": idempotency_key,
                }))
                .expect("encode"),
            ))
            .expect("request");
        let submit_response = app.clone().oneshot(submit).await.expect("submit");
        assert_eq!(submit_response.status(), StatusCode::CREATED);
        let body = read_body_to_string(submit_response).await;
        let parsed: serde_json::Value = serde_json::from_str(&body).expect("parse");
        let order_id = parsed["order_id"].as_str().expect("order_id").to_owned();

        let propose = Request::builder()
            .uri(format!(
                "/v1/admin/redemption/orders/{order_id}/refund-proposals"
            ))
            .method("POST")
            .header("content-type", "application/json")
            .header("x-user-id", proposer)
            .body(Body::from(
                serde_json::to_vec(&serde_json::json!({"reason": "wrong item"})).expect("encode"),
            ))
            .expect("request");
        let propose_response = app.clone().oneshot(propose).await.expect("propose");
        assert_eq!(propose_response.status(), StatusCode::CREATED);
        let body = read_body_to_string(propose_response).await;
        let parsed: serde_json::Value = serde_json::from_str(&body).expect("parse");
        parsed["proposal_id"]
            .as_str()
            .expect("proposal_id")
            .to_owned()
    }

    #[tokio::test]
    async fn admin_propose_201_on_valid_proposal() {
        let app = build_router_with_seeded_sku().await;
        let proposal_id = submit_then_propose_helper(&app, "prop-201", "ops_alice").await;
        assert_eq!(proposal_id.len(), 26, "ULID proposal_id");
    }

    #[tokio::test]
    async fn admin_propose_401_when_x_user_id_missing() {
        let app = build_router_with_seeded_sku().await;
        let request = Request::builder()
            .uri("/v1/admin/redemption/orders/anything/refund-proposals")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_vec(&serde_json::json!({"reason": "test"})).expect("encode"),
            ))
            .expect("request");
        let response = app.oneshot(request).await.expect("oneshot");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn admin_approve_200_for_distinct_approver() {
        let app = build_router_with_seeded_sku().await;
        let proposal_id = submit_then_propose_helper(&app, "prop-app", "ops_alice").await;

        let approve = Request::builder()
            .uri(format!(
                "/v1/admin/redemption/refund-proposals/{proposal_id}/approve"
            ))
            .method("POST")
            .header("x-user-id", "ops_bob")
            .body(Body::empty())
            .expect("request");
        let response = app.oneshot(approve).await.expect("oneshot");
        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            read_body_to_string(response)
                .await
                .contains("\"state\":\"refunded\"")
        );
    }

    #[tokio::test]
    async fn admin_approve_422_when_approver_matches_proposer() {
        let app = build_router_with_seeded_sku().await;
        let proposal_id = submit_then_propose_helper(&app, "prop-422", "ops_alice").await;

        let approve = Request::builder()
            .uri(format!(
                "/v1/admin/redemption/refund-proposals/{proposal_id}/approve"
            ))
            .method("POST")
            .header("x-user-id", "ops_alice") // same as proposer
            .body(Body::empty())
            .expect("request");
        let response = app.oneshot(approve).await.expect("oneshot");
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert!(
            read_body_to_string(response)
                .await
                .contains("four_eye_violation")
        );
    }

    #[tokio::test]
    async fn admin_approve_409_when_proposal_already_approved() {
        let app = build_router_with_seeded_sku().await;
        let proposal_id = submit_then_propose_helper(&app, "prop-409", "ops_alice").await;

        let approve_first = Request::builder()
            .uri(format!(
                "/v1/admin/redemption/refund-proposals/{proposal_id}/approve"
            ))
            .method("POST")
            .header("x-user-id", "ops_bob")
            .body(Body::empty())
            .expect("request");
        let resp1 = app.clone().oneshot(approve_first).await.expect("first");
        assert_eq!(resp1.status(), StatusCode::OK);

        let approve_second = Request::builder()
            .uri(format!(
                "/v1/admin/redemption/refund-proposals/{proposal_id}/approve"
            ))
            .method("POST")
            .header("x-user-id", "ops_carol")
            .body(Body::empty())
            .expect("request");
        let resp2 = app.oneshot(approve_second).await.expect("second");
        assert_eq!(resp2.status(), StatusCode::CONFLICT);
        assert!(
            read_body_to_string(resp2)
                .await
                .contains("proposal_already_resolved")
        );
    }

    #[tokio::test]
    async fn cancel_route_409s_for_terminal_order() {
        // Fulfilled order can't be cancelled via this path (CS path only).
        let app = build_router_with_seeded_sku().await;

        // First submit + commit a DigitalImmediate order so it's Fulfilled.
        let submit = Request::builder()
            .uri("/v1/redemption/orders")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_vec(&serde_json::json!({
                    "user_id": "user_42",
                    "sku_id": "sku_a",
                    "idempotency_key": "cancel-409",
                }))
                .expect("encode"),
            ))
            .expect("request");
        let submit_response = app.clone().oneshot(submit).await.expect("submit");
        assert_eq!(submit_response.status(), StatusCode::CREATED);
        let body = read_body_to_string(submit_response).await;
        let parsed: serde_json::Value = serde_json::from_str(&body).expect("parse");
        let order_id = parsed["order_id"].as_str().expect("order_id").to_owned();

        let cancel = Request::builder()
            .uri(format!("/v1/redemption/orders/{order_id}/cancel"))
            .method("POST")
            .body(Body::empty())
            .expect("request");
        let response = app.oneshot(cancel).await.expect("oneshot");
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert!(
            read_body_to_string(response)
                .await
                .contains("order_state_not_allowed")
        );
    }
}
