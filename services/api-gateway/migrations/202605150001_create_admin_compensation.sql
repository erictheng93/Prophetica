CREATE TABLE admin_compensation_proposals (
    id BIGSERIAL PRIMARY KEY,
    proposal_id TEXT GENERATED ALWAYS AS ('cmp-' || lpad(id::text, 6, '0')) STORED UNIQUE,
    subject_user_id TEXT NOT NULL,
    ticket_id TEXT NOT NULL,
    reason TEXT NOT NULL,
    amount_minor_units BIGINT NOT NULL CHECK (amount_minor_units > 0),
    status TEXT NOT NULL CHECK (status IN ('pending_approval', 'approved_ready_for_ledger')),
    ledger_transaction_posted BOOLEAN NOT NULL DEFAULT FALSE,
    ledger_transaction_id BIGINT,
    correlation_id TEXT NOT NULL UNIQUE,
    actor_user_id TEXT NOT NULL,
    actor_role TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_admin_compensation_subject_created
    ON admin_compensation_proposals(subject_user_id, created_at DESC);
CREATE INDEX idx_admin_compensation_ticket
    ON admin_compensation_proposals(ticket_id);
CREATE INDEX idx_admin_compensation_status_created
    ON admin_compensation_proposals(status, created_at DESC);

CREATE TABLE admin_compensation_proposal_approvals (
    id BIGSERIAL PRIMARY KEY,
    proposal_id BIGINT NOT NULL REFERENCES admin_compensation_proposals(id) ON DELETE CASCADE,
    approver_user_id TEXT NOT NULL,
    role TEXT NOT NULL,
    ordinal SMALLINT NOT NULL CHECK (ordinal IN (1, 2)),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (proposal_id, approver_user_id),
    UNIQUE (proposal_id, ordinal)
);

CREATE TABLE admin_compensation_audit_events (
    id BIGSERIAL PRIMARY KEY,
    proposal_id BIGINT NOT NULL REFERENCES admin_compensation_proposals(id) ON DELETE CASCADE,
    action TEXT NOT NULL,
    actor_user_id TEXT NOT NULL,
    actor_role TEXT NOT NULL,
    subject_user_id TEXT NOT NULL,
    ticket_id TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_admin_compensation_audit_correlation
    ON admin_compensation_audit_events(correlation_id);
