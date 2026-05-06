use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::order_book::{NewOrder, OrderBook, OrderBookError, OrderId, Symbol};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WalRecord {
    PlaceOrder(NewOrder),
    CancelOrder(OrderId),
}

/// Appends a single JSONL WAL record.
///
/// # Errors
///
/// Returns an error when the file cannot be opened, the record cannot be
/// serialized, or the record cannot be flushed to disk.
pub fn append_record(path: &Path, record: &WalRecord) -> Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("open WAL for append: {}", path.display()))?;
    serde_json::to_writer(&mut file, record).context("serialize WAL record")?;
    file.write_all(b"\n").context("write WAL record newline")?;
    file.flush().context("flush WAL record")?;
    Ok(())
}

/// Reads all JSONL WAL records from a file.
///
/// # Errors
///
/// Returns an error when the file cannot be opened, a line cannot be read, or a
/// line cannot be decoded as a WAL record.
pub fn read_records(path: &Path) -> Result<Vec<WalRecord>> {
    let file =
        File::open(path).with_context(|| format!("open WAL for read: {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut records = Vec::new();

    for line in reader.lines() {
        let line = line.context("read WAL line")?;
        if line.trim().is_empty() {
            continue;
        }
        records.push(serde_json::from_str(&line).context("deserialize WAL record")?);
    }

    Ok(records)
}

/// Replays WAL records into a fresh order book for the given market symbol.
///
/// # Errors
///
/// Returns an [`OrderBookError`] when a recorded command is invalid for the
/// reconstructed book state.
pub fn replay_records(
    symbol: Symbol,
    records: impl IntoIterator<Item = WalRecord>,
) -> Result<OrderBook, OrderBookError> {
    replay_records_into(OrderBook::new(symbol), records)
}

/// Replays WAL records into an existing order book, typically restored from a snapshot.
///
/// # Errors
///
/// Returns an [`OrderBookError`] when a recorded command is invalid for the
/// reconstructed book state.
pub fn replay_records_into(
    mut book: OrderBook,
    records: impl IntoIterator<Item = WalRecord>,
) -> Result<OrderBook, OrderBookError> {
    for record in records {
        match record {
            WalRecord::PlaceOrder(order) => {
                book.add_order(order)?;
            }
            WalRecord::CancelOrder(order_id) => {
                book.cancel_order(&order_id)?;
            }
        }
    }

    Ok(book)
}
