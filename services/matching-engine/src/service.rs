use std::{collections::HashMap, sync::Arc};

use proto::proph::v1::{
    BookLevel, BookSnapshot, CancelOrderRequest, CancelOrderResponse, GetBookSnapshotRequest,
    MarketId, Money, Order, OrderId as ProtoOrderId, OrderStatus, OrderType as ProtoOrderType,
    PlaceOrderRequest, PlaceOrderResponse, Side as ProtoSide, TimeInForce, UserId,
    engine_service_server::EngineService,
};
use tokio::sync::{Mutex, broadcast, mpsc, oneshot};
use tonic::{Request, Response, Status};
use ulid::Ulid;

use crate::{
    order_book::{
        AddOrderResult, NewOrder, OrderBookError, OrderBookSnapshot, OrderId, OrderSide, OrderType,
        Price, Quantity, SnapshotRestingOrder, Symbol,
    },
    shard::{MarketCommand, MarketEvent, MarketShard},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineEvent {
    pub market_id: String,
    pub sequence: u64,
    pub event: MarketEvent,
}

#[derive(Debug, Clone)]
pub struct InMemoryEngineService {
    shards: Arc<Mutex<HashMap<String, mpsc::Sender<MarketCommand>>>>,
    order_routes: Arc<Mutex<HashMap<Vec<u8>, OrderRoute>>>,
    events: broadcast::Sender<EngineEvent>,
}

impl Default for InMemoryEngineService {
    fn default() -> Self {
        let (events, _event_rx) = broadcast::channel(1024);
        Self {
            shards: Arc::default(),
            order_routes: Arc::default(),
            events,
        }
    }
}

#[derive(Debug, Clone)]
struct OrderRoute {
    market_id: String,
    internal_order_id: OrderId,
}

struct PlacedOrderContext {
    order_id: Vec<u8>,
    user_id: u64,
    market_id: String,
    side: OrderSide,
    order_type: OrderType,
    price: Price,
    quantity: Quantity,
    client_order_id: String,
}

#[tonic::async_trait]
impl EngineService for InMemoryEngineService {
    async fn place_order(
        &self,
        request: Request<PlaceOrderRequest>,
    ) -> Result<Response<PlaceOrderResponse>, Status> {
        let request = request.into_inner();
        let market_id = required_market_id(request.market_id.as_ref())?;
        let user_id = required_user_id(request.user_id.as_ref())?;
        let side = map_side(request.side)?;
        let order_type = map_order_type(request.r#type, request.post_only)?;
        let price = checked_price(request.price_pts)?;
        let quantity = checked_quantity(request.quantity)?;
        let order_ulid = Ulid::new();
        let order_id_bytes = order_ulid.to_bytes().to_vec();
        let internal_order_id = OrderId::new(order_ulid.to_string());
        let command_order = NewOrder {
            order_id: internal_order_id.clone(),
            user_id,
            symbol: Symbol::new(market_id.clone()),
            side,
            order_type,
            price,
            quantity,
        };

        let sender = self.sender_for_market(&market_id).await;
        let (reply_to, reply_rx) = oneshot::channel();
        sender
            .send(MarketCommand::PlaceOrder {
                order: command_order,
                reply_to,
            })
            .await
            .map_err(|_| Status::unavailable("market shard stopped"))?;
        let result = reply_rx
            .await
            .map_err(|_| Status::unavailable("market shard dropped place order reply"))?
            .map_err(|error| map_order_book_error(&error))?;

        self.order_routes.lock().await.insert(
            order_id_bytes.clone(),
            OrderRoute {
                market_id: market_id.clone(),
                internal_order_id: internal_order_id.clone(),
            },
        );

        Ok(Response::new(place_order_response(
            &result,
            PlacedOrderContext {
                order_id: order_id_bytes,
                user_id,
                market_id,
                side,
                order_type,
                price,
                quantity,
                client_order_id: request.client_order_id,
            },
        )))
    }

    async fn cancel_order(
        &self,
        request: Request<CancelOrderRequest>,
    ) -> Result<Response<CancelOrderResponse>, Status> {
        let request = request.into_inner();
        let user_id = required_user_id(request.user_id.as_ref())?;
        let order_id = request
            .order_id
            .ok_or_else(|| Status::invalid_argument("order_id is required"))?;
        let order_route_key: &[u8] = order_id.value.as_ref();
        let route = self
            .order_routes
            .lock()
            .await
            .get(order_route_key)
            .cloned()
            .ok_or_else(|| Status::not_found("order route not found"))?;
        let sender = self.sender_for_market(&route.market_id).await;
        let (reply_to, reply_rx) = oneshot::channel();
        sender
            .send(MarketCommand::CancelOrderForUser {
                order_id: route.internal_order_id,
                user_id,
                reply_to,
            })
            .await
            .map_err(|_| Status::unavailable("market shard stopped"))?;
        let result = reply_rx
            .await
            .map_err(|_| Status::unavailable("market shard dropped cancel reply"))?
            .map_err(|error| map_order_book_error(&error))?;

        Ok(Response::new(CancelOrderResponse {
            order: Some(Order {
                id: Some(order_id),
                user_id: Some(UserId { value: user_id }),
                market_id: Some(MarketId {
                    value: route.market_id,
                }),
                status: OrderStatus::Cancelled.into(),
                qty_remaining: 0,
                qty_total: i64::try_from(result.canceled_quantity).unwrap_or(i64::MAX),
                ..Order::default()
            }),
        }))
    }

    async fn get_book_snapshot(
        &self,
        request: Request<GetBookSnapshotRequest>,
    ) -> Result<Response<BookSnapshot>, Status> {
        let request = request.into_inner();
        let market_id = required_market_id(request.market_id.as_ref())?;
        let depth = usize::try_from(request.depth.max(1))
            .unwrap_or(10)
            .clamp(1, 100);
        let sender = self.sender_for_market(&market_id).await;
        let (reply_to, reply_rx) = oneshot::channel();
        sender
            .send(MarketCommand::GetSnapshot { reply_to })
            .await
            .map_err(|_| Status::unavailable("market shard stopped"))?;
        let snapshot = reply_rx
            .await
            .map_err(|_| Status::unavailable("market shard dropped snapshot reply"))?;

        Ok(Response::new(book_snapshot_response(snapshot, depth)))
    }
}

impl InMemoryEngineService {
    #[must_use]
    pub fn subscribe_events(&self) -> broadcast::Receiver<EngineEvent> {
        self.events.subscribe()
    }

    async fn sender_for_market(&self, market_id: &str) -> mpsc::Sender<MarketCommand> {
        let mut shards = self.shards.lock().await;
        if let Some(sender) = shards.get(market_id) {
            return sender.clone();
        }

        let (command_tx, command_rx) = mpsc::channel(1024);
        let (event_tx, mut event_rx) = mpsc::channel(1024);
        let event_broadcast = self.events.clone();
        let event_market_id = market_id.to_string();
        tokio::spawn(async move {
            let mut sequence = 1;
            while let Some(event) = event_rx.recv().await {
                let _ = event_broadcast.send(EngineEvent {
                    market_id: event_market_id.clone(),
                    sequence,
                    event,
                });
                sequence += 1;
            }
        });
        let shard = MarketShard::new(Symbol::new(market_id.to_string()));
        tokio::spawn(shard.run(command_rx, event_tx));
        shards.insert(market_id.to_string(), command_tx.clone());
        command_tx
    }
}

fn place_order_response(
    result: &AddOrderResult,
    context: PlacedOrderContext,
) -> PlaceOrderResponse {
    let filled_qty = result
        .trades
        .iter()
        .map(|trade| trade.quantity)
        .sum::<u64>();
    let avg_price_pts = average_price(result);
    let status = if result.remaining_quantity == 0 {
        OrderStatus::Filled
    } else if filled_qty > 0 {
        OrderStatus::Partial
    } else if result.rested {
        OrderStatus::Active
    } else {
        OrderStatus::Cancelled
    };

    PlaceOrderResponse {
        order: Some(Order {
            id: Some(ProtoOrderId {
                value: context.order_id,
            }),
            user_id: Some(UserId {
                value: context.user_id,
            }),
            market_id: Some(MarketId {
                value: context.market_id,
            }),
            side: proto_side(context.side).into(),
            r#type: proto_order_type(context.order_type).into(),
            price_pts: i32::try_from(context.price).unwrap_or(i32::MAX),
            qty_total: i64::try_from(context.quantity).unwrap_or(i64::MAX),
            qty_remaining: i64::try_from(result.remaining_quantity).unwrap_or(i64::MAX),
            status: status.into(),
            tif: TimeInForce::Gtc.into(),
            post_only: context.order_type == OrderType::PostOnly,
            client_order_id: context.client_order_id,
            submitted_at: None,
            seq_no: result.trades.last().map_or(0, |trade| trade.sequence),
        }),
        filled_qty: i64::try_from(filled_qty).unwrap_or(i64::MAX),
        avg_price_pts,
        fee: Some(Money {
            amount: 0,
            currency: "PTS".to_string(),
        }),
    }
}

fn book_snapshot_response(snapshot: OrderBookSnapshot, depth: usize) -> BookSnapshot {
    BookSnapshot {
        seq_no: snapshot.next_sequence.saturating_sub(1),
        bids: aggregate_levels(snapshot.bids, depth, true),
        asks: aggregate_levels(snapshot.asks, depth, false),
        last_trade_pts: 0,
    }
}

fn aggregate_levels(
    orders: Vec<SnapshotRestingOrder>,
    depth: usize,
    descending: bool,
) -> Vec<BookLevel> {
    let mut by_price = std::collections::BTreeMap::<Price, Quantity>::new();
    for order in orders {
        *by_price.entry(order.price).or_default() += order.quantity;
    }

    let levels: Box<dyn Iterator<Item = (Price, Quantity)>> = if descending {
        Box::new(by_price.into_iter().rev())
    } else {
        Box::new(by_price.into_iter())
    };

    levels
        .take(depth)
        .map(|(price_pts, qty)| BookLevel {
            price_pts: i32::try_from(price_pts).unwrap_or(i32::MAX),
            qty: i64::try_from(qty).unwrap_or(i64::MAX),
        })
        .collect()
}

fn average_price(result: &AddOrderResult) -> i32 {
    let total_quantity = result
        .trades
        .iter()
        .map(|trade| trade.quantity)
        .sum::<u64>();
    if total_quantity == 0 {
        return 0;
    }

    let notional = result
        .trades
        .iter()
        .map(|trade| trade.price.saturating_mul(trade.quantity))
        .sum::<u64>();
    i32::try_from(notional / total_quantity).unwrap_or(i32::MAX)
}

fn required_market_id(market_id: Option<&MarketId>) -> Result<String, Status> {
    let market_id = market_id
        .ok_or_else(|| Status::invalid_argument("market_id is required"))?
        .value
        .trim();
    if market_id.is_empty() {
        return Err(Status::invalid_argument("market_id must not be empty"));
    }
    Ok(market_id.to_string())
}

fn required_user_id(user_id: Option<&UserId>) -> Result<u64, Status> {
    let user_id = user_id
        .ok_or_else(|| Status::invalid_argument("user_id is required"))?
        .value;
    if user_id == 0 {
        return Err(Status::invalid_argument("user_id must be non-zero"));
    }
    Ok(user_id)
}

fn checked_price(price_pts: i32) -> Result<Price, Status> {
    if !(1..100).contains(&price_pts) {
        return Err(Status::invalid_argument(
            "price_pts must satisfy 0 < price < 100",
        ));
    }
    u64::try_from(price_pts).map_err(|_| Status::invalid_argument("price_pts is invalid"))
}

fn checked_quantity(quantity: i64) -> Result<Quantity, Status> {
    if quantity <= 0 {
        return Err(Status::invalid_argument("quantity must be positive"));
    }
    u64::try_from(quantity).map_err(|_| Status::invalid_argument("quantity is invalid"))
}

fn map_side(side: i32) -> Result<OrderSide, Status> {
    match ProtoSide::try_from(side) {
        Ok(ProtoSide::Bid) => Ok(OrderSide::Buy),
        Ok(ProtoSide::Ask) => Ok(OrderSide::Sell),
        Ok(ProtoSide::Unspecified) | Err(_) => Err(Status::invalid_argument(
            "side must be SIDE_BID or SIDE_ASK",
        )),
    }
}

fn map_order_type(order_type: i32, post_only: bool) -> Result<OrderType, Status> {
    if post_only {
        return Ok(OrderType::PostOnly);
    }

    match ProtoOrderType::try_from(order_type) {
        Ok(ProtoOrderType::Limit) => Ok(OrderType::Limit),
        Ok(ProtoOrderType::Ioc) => Ok(OrderType::ImmediateOrCancel),
        Ok(ProtoOrderType::Fok) => Ok(OrderType::FillOrKill),
        Ok(ProtoOrderType::PostOnly) => Ok(OrderType::PostOnly),
        Ok(ProtoOrderType::Market | ProtoOrderType::Unspecified) | Err(_) => Err(
            Status::invalid_argument("order type must be LIMIT, IOC, FOK, or POST_ONLY"),
        ),
    }
}

fn proto_side(side: OrderSide) -> ProtoSide {
    match side {
        OrderSide::Buy => ProtoSide::Bid,
        OrderSide::Sell => ProtoSide::Ask,
    }
}

fn proto_order_type(order_type: OrderType) -> ProtoOrderType {
    match order_type {
        OrderType::Limit => ProtoOrderType::Limit,
        OrderType::ImmediateOrCancel => ProtoOrderType::Ioc,
        OrderType::FillOrKill => ProtoOrderType::Fok,
        OrderType::PostOnly => ProtoOrderType::PostOnly,
    }
}

fn map_order_book_error(error: &OrderBookError) -> Status {
    match error {
        OrderBookError::CancelOwnerMismatch => {
            Status::permission_denied("order belongs to another user")
        }
        OrderBookError::UnknownOrder(_) => Status::not_found("order not found"),
        OrderBookError::PriceOutOfRange | OrderBookError::ZeroPrice => {
            Status::invalid_argument("invalid price")
        }
        OrderBookError::ZeroQuantity => Status::invalid_argument("invalid quantity"),
        OrderBookError::DuplicateOrderId(_) => Status::already_exists("duplicate order id"),
        OrderBookError::SelfTradePrevented => Status::failed_precondition("self trade prevented"),
        OrderBookError::PostOnlyWouldCross => Status::failed_precondition("post-only would cross"),
        OrderBookError::FillOrKillWouldNotFill => {
            Status::failed_precondition("fill-or-kill would not fill")
        }
        OrderBookError::SymbolMismatch { .. } => Status::invalid_argument("market mismatch"),
    }
}
