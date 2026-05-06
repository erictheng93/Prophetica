#![allow(clippy::expect_used)]

use matching_engine::service::InMemoryEngineService;
use matching_engine::shard::MarketEvent;
use proto::proph::v1::{
    AuditMeta, CancelOrderRequest, GetBookSnapshotRequest, MarketId, OrderId, OrderType,
    PlaceOrderRequest, Side, TimeInForce, UserId, engine_service_client::EngineServiceClient,
    engine_service_server::EngineService,
};
use tokio::sync::oneshot;

fn market_id() -> MarketId {
    MarketId {
        value: "mkt-election".to_string(),
    }
}

fn place_order_request(
    user_id: u64,
    side: Side,
    order_type: OrderType,
    price_pts: i32,
    quantity: i64,
    client_order_id: &str,
) -> PlaceOrderRequest {
    PlaceOrderRequest {
        audit: Some(AuditMeta::default()),
        user_id: Some(UserId { value: user_id }),
        market_id: Some(market_id()),
        side: side.into(),
        outcome: 1,
        r#type: order_type.into(),
        price_pts,
        quantity,
        tif: TimeInForce::Gtc.into(),
        post_only: order_type == OrderType::PostOnly,
        client_order_id: client_order_id.to_string(),
    }
}

#[tokio::test]
async fn place_order_and_get_book_snapshot_flow_through_engine_service() {
    let service = InMemoryEngineService::default();

    let response = service
        .place_order(tonic::Request::new(place_order_request(
            10,
            Side::Ask,
            OrderType::Limit,
            55,
            3,
            "client-maker",
        )))
        .await
        .expect("place order should succeed")
        .into_inner();

    let order = response.order.expect("response should include order");
    assert_eq!(order.market_id, Some(market_id()));
    assert_eq!(order.qty_remaining, 3);
    assert_eq!(order.price_pts, 55);
    assert_eq!(order.id.expect("order id should exist").value.len(), 16);

    let snapshot = service
        .get_book_snapshot(tonic::Request::new(GetBookSnapshotRequest {
            market_id: Some(market_id()),
            depth: 10,
        }))
        .await
        .expect("snapshot should succeed")
        .into_inner();

    assert_eq!(snapshot.asks.len(), 1);
    assert_eq!(snapshot.asks[0].price_pts, 55);
    assert_eq!(snapshot.asks[0].qty, 3);
}

#[tokio::test]
async fn cancel_order_rejects_non_owner_and_allows_owner() {
    let service = InMemoryEngineService::default();
    let response = service
        .place_order(tonic::Request::new(place_order_request(
            10,
            Side::Ask,
            OrderType::Limit,
            55,
            3,
            "client-maker",
        )))
        .await
        .expect("place order should succeed")
        .into_inner();
    let order_id = response
        .order
        .expect("response should include order")
        .id
        .expect("order id should exist");

    let non_owner = service
        .cancel_order(tonic::Request::new(CancelOrderRequest {
            audit: None,
            order_id: Some(order_id.clone()),
            user_id: Some(UserId { value: 20 }),
        }))
        .await
        .expect_err("non-owner cancel should be rejected");
    assert_eq!(non_owner.code(), tonic::Code::PermissionDenied);

    let owner = service
        .cancel_order(tonic::Request::new(CancelOrderRequest {
            audit: None,
            order_id: Some(OrderId {
                value: order_id.value,
            }),
            user_id: Some(UserId { value: 10 }),
        }))
        .await
        .expect("owner cancel should succeed")
        .into_inner();
    let cancelled = owner.order.expect("response should include order");
    assert_eq!(cancelled.qty_remaining, 0);
}

#[tokio::test]
async fn grpc_server_serves_engine_service_requests() {
    let addr = free_loopback_addr();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server = tokio::spawn(async move {
        matching_engine::api::run_grpc_on_addr(addr, async {
            let _ = shutdown_rx.await;
        })
        .await
    });

    let mut client = connect_engine_client(addr).await;
    let response = client
        .place_order(place_order_request(
            10,
            Side::Ask,
            OrderType::Limit,
            55,
            3,
            "client-maker",
        ))
        .await
        .expect("place order should succeed over grpc")
        .into_inner();
    assert_eq!(
        response
            .order
            .expect("response should include order")
            .qty_remaining,
        3
    );

    let snapshot = client
        .get_book_snapshot(GetBookSnapshotRequest {
            market_id: Some(market_id()),
            depth: 10,
        })
        .await
        .expect("snapshot should succeed over grpc")
        .into_inner();
    assert_eq!(snapshot.asks.len(), 1);
    assert_eq!(snapshot.asks[0].qty, 3);

    shutdown_tx.send(()).expect("shutdown signal should send");
    server
        .await
        .expect("server task should join")
        .expect("server should stop cleanly");
}

fn free_loopback_addr() -> std::net::SocketAddr {
    let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .expect("ephemeral listener should bind");
    listener
        .local_addr()
        .expect("ephemeral listener should expose local addr")
}

async fn connect_engine_client(
    addr: std::net::SocketAddr,
) -> EngineServiceClient<tonic::transport::Channel> {
    let endpoint = format!("http://{addr}");
    let mut last_error = None;
    for _ in 0..20 {
        match EngineServiceClient::connect(endpoint.clone()).await {
            Ok(client) => return client,
            Err(error) => {
                last_error = Some(error);
                tokio::time::sleep(std::time::Duration::from_millis(25)).await;
            }
        }
    }

    panic!("grpc client should connect: {last_error:?}");
}

#[tokio::test]
async fn service_subscribers_receive_ordered_market_events() {
    let service = InMemoryEngineService::default();
    let mut events = service.subscribe_events();

    service
        .place_order(tonic::Request::new(place_order_request(
            10,
            Side::Ask,
            OrderType::Limit,
            55,
            3,
            "client-maker",
        )))
        .await
        .expect("place order should succeed");

    let accepted = events.recv().await.expect("accepted event should publish");
    assert_eq!(accepted.market_id, "mkt-election");
    assert_eq!(accepted.sequence, 1);
    assert!(matches!(accepted.event, MarketEvent::OrderAccepted { .. }));

    let resting = events.recv().await.expect("resting event should publish");
    assert_eq!(resting.market_id, "mkt-election");
    assert_eq!(resting.sequence, 2);
    assert!(matches!(resting.event, MarketEvent::OrderResting { .. }));
}

#[tokio::test]
async fn service_appends_successful_commands_to_market_wal() {
    let wal_dir = unique_wal_dir();
    std::fs::create_dir_all(&wal_dir).expect("wal dir should be created");
    let service = InMemoryEngineService::with_wal_dir(wal_dir.clone());

    let response = service
        .place_order(tonic::Request::new(place_order_request(
            10,
            Side::Ask,
            OrderType::Limit,
            55,
            3,
            "client-maker",
        )))
        .await
        .expect("place order should succeed")
        .into_inner();
    let order_id = response
        .order
        .expect("response should include order")
        .id
        .expect("order id should exist");

    service
        .cancel_order(tonic::Request::new(CancelOrderRequest {
            audit: None,
            order_id: Some(order_id),
            user_id: Some(UserId { value: 10 }),
        }))
        .await
        .expect("cancel should succeed");

    let records = matching_engine::wal::read_records(&wal_dir.join("mkt-election.wal.jsonl"))
        .expect("wal records should read");
    assert_eq!(records.len(), 2);
    assert!(matches!(
        records[0],
        matching_engine::wal::WalRecord::PlaceOrder(_)
    ));
    assert!(matches!(
        records[1],
        matching_engine::wal::WalRecord::CancelOrder(_)
    ));
}

#[tokio::test]
async fn service_recovers_book_and_cancel_routes_from_market_wal() {
    let wal_dir = unique_wal_dir();
    std::fs::create_dir_all(&wal_dir).expect("wal dir should be created");
    let service = InMemoryEngineService::with_wal_dir(wal_dir.clone());

    let response = service
        .place_order(tonic::Request::new(place_order_request(
            10,
            Side::Ask,
            OrderType::Limit,
            55,
            3,
            "client-maker",
        )))
        .await
        .expect("place order should succeed")
        .into_inner();
    let order_id = response
        .order
        .expect("response should include order")
        .id
        .expect("order id should exist");

    let recovered =
        InMemoryEngineService::recover_from_wal_dir(wal_dir, ["mkt-election".to_string()])
            .expect("service should recover from wal");
    let snapshot = recovered
        .get_book_snapshot(tonic::Request::new(GetBookSnapshotRequest {
            market_id: Some(market_id()),
            depth: 10,
        }))
        .await
        .expect("snapshot should recover")
        .into_inner();
    assert_eq!(snapshot.asks.len(), 1);
    assert_eq!(snapshot.asks[0].price_pts, 55);
    assert_eq!(snapshot.asks[0].qty, 3);

    recovered
        .cancel_order(tonic::Request::new(CancelOrderRequest {
            audit: None,
            order_id: Some(order_id),
            user_id: Some(UserId { value: 10 }),
        }))
        .await
        .expect("recovered owner route should cancel");
}

fn unique_wal_dir() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "prophetica-engine-service-wal-{}",
        ulid::Ulid::new()
    ))
}
