pub const SERVICE_NAME: &str = "settlement-worker";
pub const DEFAULT_PORT: u16 = 8090;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Yes,
    No,
    Void,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Position {
    pub id: String,
    pub user_id: String,
    pub outcome: Outcome,
    pub qty: i64,
    pub avg_price_pts: i64,
}

impl Position {
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        user_id: impl Into<String>,
        outcome: Outcome,
        qty: i64,
        avg_price_pts: i64,
    ) -> Self {
        Self {
            id: id.into(),
            user_id: user_id.into(),
            outcome,
            qty,
            avg_price_pts,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettlementLedgerDraft {
    pub market_id: String,
    pub position_id: String,
    pub user_id: String,
    pub amount_points: i64,
    pub txn_type: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettlementBatch {
    pub batch_id: String,
    pub positions: Vec<Position>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettlementBatchStatus {
    Completed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchExecutionRecord {
    pub batch_id: String,
    pub status: SettlementBatchStatus,
    pub attempts: u8,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SettlementProgress {
    pub completed_batches: std::collections::HashSet<String>,
    pub records: Vec<BatchExecutionRecord>,
}

impl SettlementProgress {
    pub fn mark_completed(&mut self, batch_id: &str, attempts: u8) {
        if self.completed_batches.insert(batch_id.to_owned()) {
            self.records.push(BatchExecutionRecord {
                batch_id: batch_id.to_owned(),
                status: SettlementBatchStatus::Completed,
                attempts,
            });
        }
    }

    fn mark_failed(&mut self, batch_id: &str, attempts: u8) {
        self.records.push(BatchExecutionRecord {
            batch_id: batch_id.to_owned(),
            status: SettlementBatchStatus::Failed,
            attempts,
        });
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettlementExecutionError {
    Transient(String),
    ExhaustedRetries(String),
}

pub struct SettlementBatchRunner {
    batches: Vec<SettlementBatch>,
    halted: bool,
}

impl SettlementBatchRunner {
    #[must_use]
    pub fn new(batches: Vec<SettlementBatch>) -> Self {
        Self {
            batches,
            halted: false,
        }
    }

    #[must_use]
    pub fn halted(batches: Vec<SettlementBatch>) -> Self {
        Self {
            batches,
            halted: true,
        }
    }

    pub fn run_with_retry(
        self,
        progress: SettlementProgress,
        execute: impl FnMut(&SettlementBatch) -> Result<(), SettlementExecutionError>,
    ) -> SettlementProgress {
        let halted = self.halted;
        self.run_with_retry_and_halt_check(progress, move || halted, execute)
    }

    pub fn run_with_retry_and_halt_check(
        self,
        mut progress: SettlementProgress,
        mut is_halted: impl FnMut() -> bool,
        mut execute: impl FnMut(&SettlementBatch) -> Result<(), SettlementExecutionError>,
    ) -> SettlementProgress {
        for batch in self.batches {
            if is_halted() {
                return progress;
            }
            if progress.completed_batches.contains(&batch.batch_id) {
                continue;
            }

            let mut attempts = 0;
            let mut completed = false;
            while attempts < 3 {
                attempts += 1;
                if execute(&batch).is_ok() {
                    progress.mark_completed(&batch.batch_id, attempts);
                    completed = true;
                    break;
                }
            }

            if !completed {
                progress.mark_failed(&batch.batch_id, attempts);
            }
        }

        progress
    }
}

#[must_use]
pub fn build_settlement_batches(
    market_id: &str,
    positions: impl IntoIterator<Item = Position>,
) -> Vec<SettlementBatch> {
    let mut batches = Vec::new();
    let mut current = Vec::with_capacity(1_000);

    for position in positions {
        current.push(position);
        if current.len() == 1_000 {
            push_batch(market_id, &mut batches, &mut current);
        }
    }

    if !current.is_empty() {
        push_batch(market_id, &mut batches, &mut current);
    }

    batches
}

fn push_batch(market_id: &str, batches: &mut Vec<SettlementBatch>, current: &mut Vec<Position>) {
    let batch_number = batches.len() + 1;
    batches.push(SettlementBatch {
        batch_id: format!("{market_id}:batch:{batch_number:04}"),
        positions: std::mem::take(current),
    });
}

#[must_use]
pub fn settle_positions(
    market_id: &str,
    winning_outcome: Outcome,
    positions: impl IntoIterator<Item = Position>,
) -> Vec<SettlementLedgerDraft> {
    positions
        .into_iter()
        .map(|position| {
            let amount_points = if position.outcome == winning_outcome {
                position.qty * 100
            } else {
                0
            };
            ledger_draft(market_id, position, amount_points, "TXN_TYPE_MARKET_SETTLE")
        })
        .collect()
}

#[must_use]
pub fn void_refund_positions(
    market_id: &str,
    positions: impl IntoIterator<Item = Position>,
) -> Vec<SettlementLedgerDraft> {
    positions
        .into_iter()
        .map(|position| {
            let amount_points = position.avg_price_pts * position.qty;
            ledger_draft(
                market_id,
                position,
                amount_points,
                "TXN_TYPE_MARKET_VOID_REFUND",
            )
        })
        .collect()
}

fn ledger_draft(
    market_id: &str,
    position: Position,
    amount_points: i64,
    txn_type: &'static str,
) -> SettlementLedgerDraft {
    SettlementLedgerDraft {
        market_id: market_id.to_owned(),
        position_id: position.id,
        user_id: position.user_id,
        amount_points,
        txn_type,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BatchExecutionRecord, Outcome, Position, SettlementBatchRunner, SettlementBatchStatus,
        SettlementExecutionError, SettlementProgress, build_settlement_batches, settle_positions,
        void_refund_positions,
    };

    #[test]
    fn settle_positions_pays_winning_outcome_at_full_settlement_value() {
        let drafts = settle_positions(
            "mkt-election",
            Outcome::Yes,
            [
                Position::new("pos-yes", "user_01", Outcome::Yes, 100, 38),
                Position::new("pos-no", "user_02", Outcome::No, 40, 62),
            ],
        );

        assert_eq!(drafts.len(), 2);
        assert_eq!(drafts[0].user_id, "user_01");
        assert_eq!(drafts[0].amount_points, 10_000);
        assert_eq!(drafts[0].txn_type, "TXN_TYPE_MARKET_SETTLE");
        assert_eq!(drafts[1].amount_points, 0);
    }

    #[test]
    fn void_refund_positions_returns_average_cost_times_quantity() {
        let drafts = void_refund_positions(
            "mkt-election",
            [
                Position::new("pos-yes", "user_01", Outcome::Yes, 100, 38),
                Position::new("pos-no", "user_02", Outcome::No, 40, 62),
            ],
        );

        assert_eq!(drafts.len(), 2);
        assert_eq!(drafts[0].amount_points, 3_800);
        assert_eq!(drafts[0].txn_type, "TXN_TYPE_MARKET_VOID_REFUND");
        assert_eq!(drafts[1].amount_points, 2_480);
    }

    #[test]
    fn build_settlement_batches_chunks_positions_by_one_thousand() {
        let positions = (0..2_501)
            .map(|i| Position::new(format!("pos-{i}"), "user_01", Outcome::Yes, 1, 50))
            .collect::<Vec<_>>();

        let batches = build_settlement_batches("mkt-large", positions);

        assert_eq!(batches.len(), 3);
        assert_eq!(batches[0].batch_id, "mkt-large:batch:0001");
        assert_eq!(batches[0].positions.len(), 1_000);
        assert_eq!(batches[1].positions.len(), 1_000);
        assert_eq!(batches[2].positions.len(), 501);
    }

    #[test]
    fn build_settlement_batches_handles_one_hundred_thousand_positions() {
        let positions = (0..100_000)
            .map(|i| Position::new(format!("pos-{i}"), "user_01", Outcome::Yes, 1, 50))
            .collect::<Vec<_>>();

        let batches = build_settlement_batches("mkt-large", positions);

        assert_eq!(batches.len(), 100);
        assert!(batches.iter().all(|batch| batch.positions.len() == 1_000));
        assert_eq!(batches[99].batch_id, "mkt-large:batch:0100");
    }

    #[test]
    fn batch_runner_retries_failed_batch_three_times_then_succeeds() {
        let batches = build_settlement_batches(
            "mkt-election",
            [Position::new("pos-yes", "user_01", Outcome::Yes, 100, 38)],
        );
        let mut attempts = 0;
        let progress = SettlementBatchRunner::new(batches).run_with_retry(
            SettlementProgress::default(),
            |batch| {
                attempts += 1;
                if attempts < 3 {
                    return Err(SettlementExecutionError::Transient(batch.batch_id.clone()));
                }
                Ok(())
            },
        );

        assert_eq!(attempts, 3);
        assert!(
            progress
                .completed_batches
                .contains("mkt-election:batch:0001")
        );
        assert_eq!(
            progress.records,
            vec![BatchExecutionRecord {
                batch_id: "mkt-election:batch:0001".to_owned(),
                status: SettlementBatchStatus::Completed,
                attempts: 3,
            }]
        );
    }

    #[test]
    fn batch_runner_resume_skips_completed_batches_to_avoid_duplicate_settlement() {
        let batches = build_settlement_batches(
            "mkt-election",
            [
                Position::new("pos-1", "user_01", Outcome::Yes, 1, 50),
                Position::new("pos-2", "user_02", Outcome::Yes, 1, 50),
            ],
        );
        let mut progress = SettlementProgress::default();
        progress.mark_completed("mkt-election:batch:0001", 1);

        let mut executed = 0;
        let progress = SettlementBatchRunner::new(batches).run_with_retry(progress, |_batch| {
            executed += 1;
            Ok(())
        });

        assert_eq!(executed, 0);
        assert_eq!(progress.records.len(), 1);
        assert!(
            progress
                .completed_batches
                .contains("mkt-election:batch:0001")
        );
    }

    #[test]
    fn halted_batch_runner_pauses_settlement_batches() {
        let batches = build_settlement_batches(
            "mkt-election",
            [Position::new("pos-yes", "user_01", Outcome::Yes, 100, 38)],
        );
        let mut executed = 0;

        let progress = SettlementBatchRunner::halted(batches).run_with_retry(
            SettlementProgress::default(),
            |_batch| {
                executed += 1;
                Ok(())
            },
        );

        assert_eq!(executed, 0);
        assert!(progress.records.is_empty());
    }

    #[test]
    fn batch_runner_checks_runtime_halt_before_each_batch() {
        let batches = build_settlement_batches(
            "mkt-election",
            [
                Position::new("pos-1", "user_01", Outcome::Yes, 1, 50),
                Position::new("pos-2", "user_02", Outcome::Yes, 1, 50),
            ],
        );
        let halt_after_first_batch = std::cell::Cell::new(false);
        let mut executed = 0;

        let progress = SettlementBatchRunner::new(batches).run_with_retry_and_halt_check(
            SettlementProgress::default(),
            || halt_after_first_batch.get(),
            |_batch| {
                executed += 1;
                halt_after_first_batch.set(true);
                Ok(())
            },
        );

        assert_eq!(executed, 1);
        assert_eq!(progress.records.len(), 1);
        assert!(
            progress
                .completed_batches
                .contains("mkt-election:batch:0001")
        );
    }
}
