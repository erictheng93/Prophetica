-- Compensation v1 state machine migration.
--
-- `cancelled` is intentionally reserved in v1. No v1 handler, UI, worker, or
-- audit path may transition a proposal into `cancelled`; it exists only for a
-- future proposer-withdrawal plan.

ALTER TABLE admin_compensation_proposals
    DROP CONSTRAINT IF EXISTS admin_compensation_proposals_status_check;

ALTER TABLE admin_compensation_proposals
    ADD COLUMN proposer_kind TEXT,
    ADD COLUMN ledger_idempotency_key TEXT UNIQUE,
    ADD COLUMN posting_attempt_count INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN last_posting_error TEXT,
    ADD COLUMN approved_at TIMESTAMPTZ,
    ADD COLUMN posted_at TIMESTAMPTZ,
    ADD COLUMN cancelled_at TIMESTAMPTZ;

UPDATE admin_compensation_proposals
SET status = CASE status
        WHEN 'pending_approval' THEN 'proposed'
        WHEN 'approved_ready_for_ledger' THEN 'approved'
        ELSE status
    END,
    proposer_kind = CASE lower(actor_role)
        WHEN 'affiliate' THEN 'affiliate'
        WHEN 'manager' THEN 'manager'
        WHEN 'admin' THEN 'admin'
        WHEN 'cs_system' THEN 'cs_system'
        ELSE 'admin'
    END;

ALTER TABLE admin_compensation_proposals
    ALTER COLUMN proposer_kind SET NOT NULL,
    ADD CONSTRAINT admin_compensation_proposals_status_check
        CHECK (status IN (
            'proposed',
            'approved',
            'posting',
            'posted',
            'posting_failed',
            'posting_unknown',
            'rejected',
            'cancelled'
        )),
    ADD CONSTRAINT admin_compensation_proposals_proposer_kind_check
        CHECK (proposer_kind IN ('affiliate', 'manager', 'admin', 'cs_system'));

COMMENT ON COLUMN admin_compensation_proposals.proposer_kind IS
    'Proposal actor class. Backfilled from actor_role for legacy rows.';
COMMENT ON COLUMN admin_compensation_proposals.ledger_idempotency_key IS
    'Wallet-ledger idempotency key generated once before posting, using cs-cmp-{proposal_id}. Retries must reuse this value.';
COMMENT ON COLUMN admin_compensation_proposals.cancelled_at IS
    'Reserved for future proposer-withdrawal support. Always NULL in compensation v1.';

CREATE INDEX IF NOT EXISTS idx_admin_compensation_stuck_queue
    ON admin_compensation_proposals(status, approved_at DESC)
    WHERE status IN ('posting', 'posting_failed', 'posting_unknown');

ALTER TABLE admin_compensation_proposal_approvals
    DROP CONSTRAINT IF EXISTS admin_compensation_proposal_approvals_ordinal_check,
    DROP CONSTRAINT IF EXISTS admin_compensation_proposal_approvals_proposal_id_ordinal_key;

ALTER TABLE admin_compensation_proposal_approvals
    ALTER COLUMN ordinal DROP NOT NULL,
    ADD COLUMN decision TEXT,
    ADD COLUMN review_note TEXT,
    ADD COLUMN reviewed_at TIMESTAMPTZ NOT NULL DEFAULT now();

UPDATE admin_compensation_proposal_approvals
SET decision = 'approved';

ALTER TABLE admin_compensation_proposal_approvals
    ALTER COLUMN decision SET NOT NULL,
    ADD CONSTRAINT admin_compensation_proposal_approvals_decision_check
        CHECK (decision IN ('approved', 'rejected'));

COMMENT ON COLUMN admin_compensation_proposal_approvals.ordinal IS
    'Legacy two-approver ordinal. Nullable in compensation v1 because review is a single manager/admin decision.';
COMMENT ON TABLE admin_compensation_proposal_approvals IS
    'Compensation review decisions. proposer != approver is enforced in application handlers and tests because PostgreSQL CHECK constraints cannot safely reference admin_compensation_proposals.';
