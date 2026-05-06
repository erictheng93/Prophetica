use tokio::sync::{mpsc, oneshot};

use crate::order_book::{
    AddOrderResult, CancelOrderResult, NewOrder, OrderBook, OrderBookError, OrderBookSnapshot,
    OrderId, OrderSide, Price, Quantity, Sequence, Symbol,
};

#[derive(Debug)]
pub enum MarketCommand {
    PlaceOrder {
        order: NewOrder,
        reply_to: oneshot::Sender<Result<AddOrderResult, OrderBookError>>,
    },
    CancelOrder {
        order_id: OrderId,
        reply_to: oneshot::Sender<Result<CancelOrderResult, OrderBookError>>,
    },
    CancelOrderForUser {
        order_id: OrderId,
        user_id: u64,
        reply_to: oneshot::Sender<Result<CancelOrderResult, OrderBookError>>,
    },
    GetSnapshot {
        reply_to: oneshot::Sender<OrderBookSnapshot>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarketEvent {
    OrderAccepted {
        order_id: OrderId,
        user_id: u64,
    },
    OrderResting {
        order_id: OrderId,
        user_id: u64,
        remaining: Quantity,
    },
    Trade {
        maker_order_id: OrderId,
        taker_order_id: OrderId,
        maker_user_id: u64,
        taker_user_id: u64,
        sequence: Sequence,
        price: Price,
        quantity: Quantity,
    },
    OrderCancelled {
        order_id: OrderId,
        user_id: Option<u64>,
        canceled_quantity: Quantity,
    },
    BookDelta {
        side: OrderSide,
        price: Price,
        quantity_delta: i64,
    },
    OrderFilled {
        order_id: OrderId,
        user_id: u64,
        total_filled: Quantity,
    },
    OrderPartiallyFilled {
        order_id: OrderId,
        user_id: u64,
        filled_now: Quantity,
        remaining: Quantity,
    },
}

#[derive(Debug)]
pub struct MarketShard {
    book: OrderBook,
}

impl MarketShard {
    #[must_use]
    pub fn new(symbol: Symbol) -> Self {
        Self {
            book: OrderBook::new(symbol),
        }
    }

    #[must_use]
    pub fn from_book(book: OrderBook) -> Self {
        Self { book }
    }

    pub async fn run(
        mut self,
        mut commands: mpsc::Receiver<MarketCommand>,
        events: mpsc::Sender<MarketEvent>,
    ) {
        while let Some(command) = commands.recv().await {
            match command {
                MarketCommand::PlaceOrder { order, reply_to } => {
                    let result = self.book.add_order(order.clone());
                    if let Ok(add_order_result) = &result {
                        publish_add_order_events(&order, add_order_result, &events).await;
                    }
                    let _ = reply_to.send(result);
                }
                MarketCommand::CancelOrder { order_id, reply_to } => {
                    let result = self.book.cancel_order(&order_id);
                    if let Ok(cancel_order_result) = &result {
                        publish_cancel_order_event(cancel_order_result, None, &events).await;
                    }
                    let _ = reply_to.send(result);
                }
                MarketCommand::CancelOrderForUser {
                    order_id,
                    user_id,
                    reply_to,
                } => {
                    let result = self.book.cancel_order_for_user(&order_id, user_id);
                    if let Ok(cancel_order_result) = &result {
                        publish_cancel_order_event(cancel_order_result, Some(user_id), &events)
                            .await;
                    }
                    let _ = reply_to.send(result);
                }
                MarketCommand::GetSnapshot { reply_to } => {
                    let _ = reply_to.send(self.book.snapshot());
                }
            }
        }
    }
}

async fn publish_add_order_events(
    order: &NewOrder,
    result: &AddOrderResult,
    events: &mpsc::Sender<MarketEvent>,
) {
    if events
        .send(MarketEvent::OrderAccepted {
            order_id: result.order_id.clone(),
            user_id: order.user_id,
        })
        .await
        .is_err()
    {
        return;
    }

    let filled_quantity = result
        .trades
        .iter()
        .map(|trade| trade.quantity)
        .sum::<Quantity>();

    for trade in &result.trades {
        let event = MarketEvent::Trade {
            maker_order_id: trade.maker_order_id.clone(),
            taker_order_id: trade.taker_order_id.clone(),
            maker_user_id: trade.maker_user_id,
            taker_user_id: trade.taker_user_id,
            sequence: trade.sequence,
            price: trade.price,
            quantity: trade.quantity,
        };
        if events.send(event).await.is_err() {
            return;
        }
        if publish_book_delta(
            events,
            maker_side(trade.aggressor_side),
            trade.price,
            -i64::try_from(trade.quantity).unwrap_or(i64::MAX),
        )
        .await
        .is_err()
        {
            return;
        }
    }

    if filled_quantity > 0 && result.remaining_quantity == 0 {
        let _ = events
            .send(MarketEvent::OrderFilled {
                order_id: result.order_id.clone(),
                user_id: order.user_id,
                total_filled: filled_quantity,
            })
            .await;
        return;
    }

    if filled_quantity > 0
        && events
            .send(MarketEvent::OrderPartiallyFilled {
                order_id: result.order_id.clone(),
                user_id: order.user_id,
                filled_now: filled_quantity,
                remaining: result.remaining_quantity,
            })
            .await
            .is_err()
    {
        return;
    }

    if result.rested {
        if events
            .send(MarketEvent::OrderResting {
                order_id: result.order_id.clone(),
                user_id: order.user_id,
                remaining: result.remaining_quantity,
            })
            .await
            .is_err()
        {
            return;
        }
        let _ = publish_book_delta(
            events,
            order.side,
            order.price,
            i64::try_from(result.remaining_quantity).unwrap_or(i64::MAX),
        )
        .await;
    } else if result.remaining_quantity > 0
        && events
            .send(MarketEvent::OrderCancelled {
                order_id: result.order_id.clone(),
                user_id: Some(order.user_id),
                canceled_quantity: result.remaining_quantity,
            })
            .await
            .is_err()
    {
        return;
    }
}

async fn publish_cancel_order_event(
    result: &CancelOrderResult,
    user_id: Option<u64>,
    events: &mpsc::Sender<MarketEvent>,
) {
    let _ = events
        .send(MarketEvent::OrderCancelled {
            order_id: result.order_id.clone(),
            user_id,
            canceled_quantity: result.canceled_quantity,
        })
        .await;
    let _ = publish_book_delta(
        events,
        result.side,
        result.price,
        -i64::try_from(result.canceled_quantity).unwrap_or(i64::MAX),
    )
    .await;
}

async fn publish_book_delta(
    events: &mpsc::Sender<MarketEvent>,
    side: OrderSide,
    price: Price,
    quantity_delta: i64,
) -> Result<(), mpsc::error::SendError<MarketEvent>> {
    events
        .send(MarketEvent::BookDelta {
            side,
            price,
            quantity_delta,
        })
        .await
}

fn maker_side(aggressor_side: OrderSide) -> OrderSide {
    match aggressor_side {
        OrderSide::Buy => OrderSide::Sell,
        OrderSide::Sell => OrderSide::Buy,
    }
}
