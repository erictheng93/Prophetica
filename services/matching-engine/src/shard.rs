use tokio::sync::{mpsc, oneshot};

use crate::order_book::{
    AddOrderResult, CancelOrderResult, NewOrder, OrderBook, OrderBookError, OrderId, Price,
    Quantity, Sequence, Symbol,
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarketEvent {
    OrderResting {
        order_id: OrderId,
    },
    Trade {
        maker_order_id: OrderId,
        taker_order_id: OrderId,
        sequence: Sequence,
        price: Price,
        quantity: Quantity,
    },
    OrderCancelled {
        order_id: OrderId,
        canceled_quantity: Quantity,
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

    pub async fn run(
        mut self,
        mut commands: mpsc::Receiver<MarketCommand>,
        events: mpsc::Sender<MarketEvent>,
    ) {
        while let Some(command) = commands.recv().await {
            match command {
                MarketCommand::PlaceOrder { order, reply_to } => {
                    let result = self.book.add_order(order);
                    if let Ok(add_order_result) = &result {
                        publish_add_order_events(add_order_result, &events).await;
                    }
                    let _ = reply_to.send(result);
                }
                MarketCommand::CancelOrder { order_id, reply_to } => {
                    let result = self.book.cancel_order(&order_id);
                    if let Ok(cancel_order_result) = &result {
                        publish_cancel_order_event(cancel_order_result, &events).await;
                    }
                    let _ = reply_to.send(result);
                }
            }
        }
    }
}

async fn publish_add_order_events(result: &AddOrderResult, events: &mpsc::Sender<MarketEvent>) {
    for trade in &result.trades {
        let event = MarketEvent::Trade {
            maker_order_id: trade.maker_order_id.clone(),
            taker_order_id: trade.taker_order_id.clone(),
            sequence: trade.sequence,
            price: trade.price,
            quantity: trade.quantity,
        };
        if events.send(event).await.is_err() {
            return;
        }
    }

    if result.rested {
        let _ = events
            .send(MarketEvent::OrderResting {
                order_id: result.order_id.clone(),
            })
            .await;
    }
}

async fn publish_cancel_order_event(
    result: &CancelOrderResult,
    events: &mpsc::Sender<MarketEvent>,
) {
    let _ = events
        .send(MarketEvent::OrderCancelled {
            order_id: result.order_id.clone(),
            canceled_quantity: result.canceled_quantity,
        })
        .await;
}
