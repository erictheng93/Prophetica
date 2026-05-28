-- SPEC-024 P5.6 / SPEC-012 §11: two-stage 4-eye CS refund.
--
-- Replaces the P5.3 one-shot admin route with proposer (POST proposal) +
-- approver (POST approve) as two separate authenticated requests. Each
-- identity is bound to its own request via the x-user-id gateway header
-- (P5.5); the approver-side handler loads the prior proposal and verifies
-- `approver_user_id != stored proposer_user_id`. Single-mutex / single-UPDATE
-- semantics on the `status` field prevent racing approvals.

CREATE TABLE cs_refund_proposals (
    proposal_id        VARCHAR(64) PRIMARY KEY,
    order_id           VARCHAR(64) NOT NULL REFERENCES redemption_orders(order_id),
    proposed_by        VARCHAR(128) NOT NULL,
    reason             TEXT NOT NULL,
    status             VARCHAR(16) NOT NULL CHECK (
        status IN ('pending', 'approved', 'rejected', 'cancelled')
    ),
    proposed_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    resolved_at        TIMESTAMPTZ,
    resolved_by        VARCHAR(128),
    metadata           JSONB NOT NULL DEFAULT '{}'
);

CREATE INDEX idx_cs_proposals_order ON cs_refund_proposals(order_id);
CREATE INDEX idx_cs_proposals_status ON cs_refund_proposals(status, proposed_at);

-- One pending proposal per order at a time. Approval flips status →
-- 'approved' and frees the slot for a follow-up (rare; defensive against
-- a refund that posts ledger but fails state advance — the next attempt
-- can re-propose without bumping into a stale pending row).
CREATE UNIQUE INDEX idx_cs_proposals_one_pending_per_order
    ON cs_refund_proposals(order_id)
    WHERE status = 'pending';
