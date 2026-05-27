//! HTTP surface for the redemption-service.
//!
//! P2 ships stubs only. The actual order, fulfillment, refund, and entitlement
//! routes return `501 Not Implemented` with a JSON body pointing at the
//! SPEC-024 implementation plan phase that owns the work. Once P3 / P4 / P5
//! land, swap each stub for a real handler in the same router so deployed
//! binaries don't need to change wiring.

use axum::{
    Json, Router,
    http::StatusCode,
    routing::{get, post},
};
use serde_json::{Value, json};

/// Builds the redemption-service HTTP router. Mounted by the
/// `redemption-api` binary alongside `sdk::service_stub::router`.
/// (`Router` is itself `#[must_use]` so no attribute is needed here.)
pub fn router() -> Router {
    Router::new()
        .route("/v1/redemption/orders", post(stub_submit_order))
        .route("/v1/redemption/orders/:order_id", get(stub_get_order))
        .route(
            "/v1/redemption/orders/:order_id/cancel",
            post(stub_cancel_order),
        )
        .route(
            "/v1/redemption/entitlements/:user_id",
            get(stub_list_entitlements),
        )
}

async fn stub_submit_order() -> (StatusCode, Json<Value>) {
    not_implemented(
        "redemption submit",
        "SPEC-024 implementation plan §1 P2 ↔ P3: order acceptance + ledger \
         commit lands with the catalog/order phase; until then this endpoint \
         is a placeholder so route shape is stable for client integration.",
    )
}

async fn stub_get_order() -> (StatusCode, Json<Value>) {
    not_implemented(
        "redemption get-order",
        "SPEC-024 implementation plan §1 P2: order lookup ships with the \
         catalog/order phase store implementation.",
    )
}

async fn stub_cancel_order() -> (StatusCode, Json<Value>) {
    not_implemented(
        "redemption cancel-order",
        "SPEC-024 implementation plan §1 P4: cancel-window refund + ledger \
         redemption_refund txn live in the expiry/breakage/refund phase.",
    )
}

async fn stub_list_entitlements() -> (StatusCode, Json<Value>) {
    not_implemented(
        "redemption list-entitlements",
        "SPEC-024 implementation plan §1 P2 ↔ P6: per-user entitlement \
         listing ships with the admin/audit phase; SPEC-024 §3.3 v1 keeps \
         entitlements non-transferable, so this route filters strictly by the \
         requesting user.",
    )
}

fn not_implemented(handler: &'static str, plan_ref: &'static str) -> (StatusCode, Json<Value>) {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({
            "error": "not_implemented",
            "handler": handler,
            "plan_ref": plan_ref,
        })),
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::router;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    async fn read_body_to_string(response: axum::response::Response) -> String {
        let body = response.into_body().collect().await.expect("collect body");
        String::from_utf8(body.to_bytes().to_vec()).expect("utf-8 body")
    }

    #[tokio::test]
    async fn submit_order_route_returns_501_with_plan_pointer() {
        let response = router()
            .oneshot(
                Request::builder()
                    .uri("/v1/redemption/orders")
                    .method("POST")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("oneshot");

        assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
        let body = read_body_to_string(response).await;
        assert!(body.contains("not_implemented"));
        assert!(body.contains("SPEC-024"));
    }

    #[tokio::test]
    async fn get_order_route_returns_501() {
        let response = router()
            .oneshot(
                Request::builder()
                    .uri("/v1/redemption/orders/01HX")
                    .method("GET")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("oneshot");
        assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
    }

    #[tokio::test]
    async fn list_entitlements_route_returns_501() {
        let response = router()
            .oneshot(
                Request::builder()
                    .uri("/v1/redemption/entitlements/user_42")
                    .method("GET")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("oneshot");
        assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
    }
}
