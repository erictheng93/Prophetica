use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::order_book::{NewOrder, OrderBook, OrderBookError, OrderId, Symbol, TradeEvent};

const SPEC_003_SYNC_EVENT_THRESHOLD: usize = 100;
const SPEC_003_SYNC_INTERVAL: Duration = Duration::from_millis(50);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WalRecord {
    PlaceOrder(NewOrder),
    CancelOrder(OrderId),
    Trade(TradeEvent),
}

pub trait WalSink {
    /// Writes all bytes to the WAL sink.
    ///
    /// # Errors
    ///
    /// Returns an error when the underlying sink cannot accept the full buffer.
    fn write_all(&mut self, bytes: &[u8]) -> std::io::Result<()>;

    /// Flushes buffered WAL bytes to the sink.
    ///
    /// # Errors
    ///
    /// Returns an error when the underlying sink cannot flush buffered data.
    fn flush(&mut self) -> std::io::Result<()>;

    /// Synchronizes WAL data to durable storage.
    ///
    /// # Errors
    ///
    /// Returns an error when the underlying sink cannot sync data.
    fn sync_data(&mut self) -> std::io::Result<()>;
}

impl WalSink for File {
    fn write_all(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        Write::write_all(self, bytes)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Write::flush(self)
    }

    fn sync_data(&mut self) -> std::io::Result<()> {
        File::sync_data(self)
    }
}

pub trait WalClock {
    fn now(&self) -> Duration;
}

#[derive(Debug, Clone)]
pub struct SystemWalClock {
    started_at: Instant,
}

impl Default for SystemWalClock {
    fn default() -> Self {
        Self {
            started_at: Instant::now(),
        }
    }
}

impl WalClock for SystemWalClock {
    fn now(&self) -> Duration {
        self.started_at.elapsed()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WalSyncPolicy {
    event_threshold: usize,
    interval: Duration,
}

impl WalSyncPolicy {
    #[must_use]
    pub const fn spec_003() -> Self {
        Self {
            event_threshold: SPEC_003_SYNC_EVENT_THRESHOLD,
            interval: SPEC_003_SYNC_INTERVAL,
        }
    }
}

pub struct WalWriter<S, C> {
    sink: S,
    clock: C,
    policy: WalSyncPolicy,
    pending_events: usize,
    last_sync_at: Duration,
}

impl<S, C> WalWriter<S, C>
where
    S: WalSink,
    C: WalClock,
{
    #[must_use]
    pub fn new(sink: S, clock: C, policy: WalSyncPolicy) -> Self {
        let last_sync_at = clock.now();
        Self {
            sink,
            clock,
            policy,
            pending_events: 0,
            last_sync_at,
        }
    }

    /// Appends one pre-serialized WAL line and syncs data when SPEC-003 thresholds are reached.
    ///
    /// # Errors
    ///
    /// Returns an error when writing, flushing, or syncing the WAL sink fails.
    pub fn append_json_line(&mut self, payload: &[u8]) -> std::io::Result<()> {
        self.sink.write_all(payload)?;
        self.sink.write_all(b"\n")?;
        self.sink.flush()?;
        self.pending_events += 1;

        if self.pending_events >= self.policy.event_threshold
            || self.pending_elapsed() >= self.policy.interval
        {
            self.sync_data()?;
        }

        Ok(())
    }

    #[must_use]
    pub const fn pending_events(&self) -> usize {
        self.pending_events
    }

    #[must_use]
    pub fn pending_elapsed(&self) -> Duration {
        self.clock.now().saturating_sub(self.last_sync_at)
    }

    fn sync_data(&mut self) -> std::io::Result<()> {
        self.sink.sync_data()?;
        self.pending_events = 0;
        self.last_sync_at = self.clock.now();
        Ok(())
    }
}

/// Appends a single JSONL WAL record.
///
/// # Errors
///
/// Returns an error when the file cannot be opened, the record cannot be
/// serialized, or the record cannot be flushed to disk.
pub fn append_record(path: &Path, record: &WalRecord) -> Result<()> {
    append_records(path, std::slice::from_ref(record))
}

/// Appends JSONL WAL records and syncs them as a single durable batch.
///
/// # Errors
///
/// Returns an error when the file cannot be opened, a record cannot be
/// serialized, or the records cannot be flushed to disk.
pub fn append_records(path: &Path, records: &[WalRecord]) -> Result<()> {
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("open WAL for append: {}", path.display()))?;
    let mut writer = WalWriter::new(file, SystemWalClock::default(), WalSyncPolicy::spec_003());
    for record in records {
        let payload = serde_json::to_vec(record).context("serialize WAL record")?;
        writer
            .append_json_line(&payload)
            .context("write WAL record")?;
    }
    writer.sync_data().context("sync WAL record")?;
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
            WalRecord::Trade(_) => {}
        }
    }

    Ok(book)
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, rc::Rc, time::Duration};

    use proptest::prelude::*;

    use super::{WalSyncPolicy, WalWriter};

    #[test]
    fn writer_syncs_data_after_one_hundred_events() {
        let clock = ManualClock::default();
        let sink = RecordingWalSink::default();
        let mut writer = WalWriter::new(sink.clone(), clock.clone(), WalSyncPolicy::spec_003());

        for index in 0..99 {
            if let Err(error) = writer.append_json_line(format!("event-{index}").as_bytes()) {
                panic!("append before threshold should succeed: {error}");
            }
        }
        assert_eq!(sink.sync_count(), 0);

        if let Err(error) = writer.append_json_line(b"event-99") {
            panic!("append at threshold should succeed: {error}");
        }

        assert_eq!(sink.sync_count(), 1);
    }

    #[test]
    fn writer_syncs_data_after_fifty_milliseconds() {
        let clock = ManualClock::default();
        let sink = RecordingWalSink::default();
        let mut writer = WalWriter::new(sink.clone(), clock.clone(), WalSyncPolicy::spec_003());

        if let Err(error) = writer.append_json_line(b"event-0") {
            panic!("first append should succeed: {error}");
        }
        clock.advance(Duration::from_millis(49));
        if let Err(error) = writer.append_json_line(b"event-1") {
            panic!("append before time threshold should succeed: {error}");
        }
        assert_eq!(sink.sync_count(), 0);

        clock.advance(Duration::from_millis(1));
        if let Err(error) = writer.append_json_line(b"event-2") {
            panic!("append at time threshold should succeed: {error}");
        }

        assert_eq!(sink.sync_count(), 1);
    }

    proptest! {
        #[test]
        fn sync_policy_never_allows_more_than_one_hundred_events_or_fifty_ms_between_syncs(
            steps in prop::collection::vec((0_u64..=10, 1_usize..=25), 1..200)
        ) {
            let clock = ManualClock::default();
            let sink = RecordingWalSink::default();
            let mut writer = WalWriter::new(sink.clone(), clock.clone(), WalSyncPolicy::spec_003());

            for (advance_ms, events) in steps {
                clock.advance(Duration::from_millis(advance_ms));
                for _ in 0..events {
                    if let Err(error) = writer.append_json_line(b"event") {
                        panic!("append should succeed: {error}");
                    }
                }
                assert!(
                    writer.pending_events() < 100,
                    "pending events exceeded threshold: {}",
                    writer.pending_events()
                );
                assert!(
                    writer.pending_elapsed() < Duration::from_millis(50),
                    "pending elapsed exceeded threshold: {:?}",
                    writer.pending_elapsed()
                );
            }
        }
    }

    #[derive(Clone, Default)]
    struct ManualClock {
        now: Rc<Cell<Duration>>,
    }

    impl ManualClock {
        fn advance(&self, duration: Duration) {
            self.now.set(self.now.get() + duration);
        }
    }

    impl super::WalClock for ManualClock {
        fn now(&self) -> Duration {
            self.now.get()
        }
    }

    #[derive(Clone, Default)]
    struct RecordingWalSink {
        syncs: Rc<Cell<usize>>,
    }

    impl RecordingWalSink {
        fn sync_count(&self) -> usize {
            self.syncs.get()
        }
    }

    impl super::WalSink for RecordingWalSink {
        fn write_all(&mut self, _bytes: &[u8]) -> std::io::Result<()> {
            Ok(())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }

        fn sync_data(&mut self) -> std::io::Result<()> {
            self.syncs.set(self.syncs.get() + 1);
            Ok(())
        }
    }
}
