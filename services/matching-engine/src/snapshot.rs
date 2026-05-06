use std::{
    fs::File,
    io::{BufReader, BufWriter},
    path::Path,
};

use anyhow::Context;

use crate::order_book::OrderBookSnapshot;

/// Writes an order book snapshot as stable JSON.
///
/// # Errors
///
/// Returns an error when the destination cannot be created or serialized.
pub fn write_snapshot(path: &Path, snapshot: &OrderBookSnapshot) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create snapshot directory {}", parent.display()))?;
    }
    let writer = BufWriter::new(
        File::create(path)
            .with_context(|| format!("failed to create snapshot file {}", path.display()))?,
    );
    serde_json::to_writer(writer, snapshot)
        .with_context(|| format!("failed to write snapshot file {}", path.display()))
}

/// Reads an order book snapshot from stable JSON.
///
/// # Errors
///
/// Returns an error when the snapshot cannot be opened or deserialized.
pub fn read_snapshot(path: &Path) -> anyhow::Result<OrderBookSnapshot> {
    let reader = BufReader::new(
        File::open(path)
            .with_context(|| format!("failed to open snapshot file {}", path.display()))?,
    );
    serde_json::from_reader(reader)
        .with_context(|| format!("failed to read snapshot file {}", path.display()))
}
