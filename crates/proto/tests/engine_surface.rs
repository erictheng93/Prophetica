use proto::proph::v1::{
    AuditMeta, BookSnapshot, CancelOrderRequest, EngineEvent, GetBookSnapshotRequest, MarketId,
    Money, Order, OrderId, PlaceOrderRequest, PlaceOrderResponse, Side, UserId,
    engine_service_server::{EngineService, EngineServiceServer},
};

#[derive(Debug, Default)]
struct DummyEngineService;

#[tonic::async_trait]
impl EngineService for DummyEngineService {
    async fn place_order(
        &self,
        _request: tonic::Request<PlaceOrderRequest>,
    ) -> Result<tonic::Response<PlaceOrderResponse>, tonic::Status> {
        Ok(tonic::Response::new(PlaceOrderResponse::default()))
    }

    async fn cancel_order(
        &self,
        _request: tonic::Request<CancelOrderRequest>,
    ) -> Result<tonic::Response<proto::proph::v1::CancelOrderResponse>, tonic::Status> {
        Ok(tonic::Response::new(
            proto::proph::v1::CancelOrderResponse::default(),
        ))
    }

    async fn get_book_snapshot(
        &self,
        _request: tonic::Request<GetBookSnapshotRequest>,
    ) -> Result<tonic::Response<BookSnapshot>, tonic::Status> {
        Ok(tonic::Response::new(BookSnapshot::default()))
    }
}

#[test]
fn engine_proto_surface_exposes_matching_engine_messages() {
    let request = PlaceOrderRequest {
        audit: Some(AuditMeta::default()),
        user_id: Some(UserId { value: 42 }),
        market_id: Some(MarketId {
            value: "mkt-election".to_string(),
        }),
        side: Side::Bid.into(),
        outcome: 1,
        r#type: 1,
        price_pts: 55,
        quantity: 2,
        tif: 1,
        post_only: false,
        client_order_id: "client-1".to_string(),
    };

    assert_eq!(request.price_pts, 55);

    let _response = PlaceOrderResponse {
        order: Some(Order::default()),
        filled_qty: 0,
        avg_price_pts: 0,
        fee: Some(Money {
            amount: 0,
            currency: "PTS".to_string(),
        }),
    };
    let _cancel = CancelOrderRequest {
        audit: None,
        order_id: Some(OrderId { value: vec![0; 16] }),
        user_id: Some(UserId { value: 42 }),
    };
    let _snapshot = BookSnapshot::default();
    let _event = EngineEvent::default();
    let _get_snapshot = GetBookSnapshotRequest {
        market_id: Some(MarketId {
            value: "mkt-election".to_string(),
        }),
        depth: 10,
    };
    let _server = EngineServiceServer::new(DummyEngineService);
}
