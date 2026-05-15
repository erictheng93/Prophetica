ALTER TABLE admin_compensation_proposals
    ADD COLUMN idempotency_key TEXT,
    ADD COLUMN request_hash CHAR(64);

CREATE UNIQUE INDEX idx_admin_compensation_idempotency
    ON admin_compensation_proposals(idempotency_key)
    WHERE idempotency_key IS NOT NULL;

CREATE TABLE admin_member_actions (
    id BIGSERIAL PRIMARY KEY,
    action_id TEXT GENERATED ALWAYS AS ('mem-act-' || lpad(id::text, 6, '0')) STORED UNIQUE,
    target_user_id TEXT NOT NULL,
    action_type TEXT NOT NULL CHECK (action_type IN (
        'add_note',
        'request_kyc_review',
        'restrict_withdrawals',
        'escalate_case'
    )),
    ticket_id TEXT NOT NULL,
    reason TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('submitted', 'pending_second_approval')),
    requires_second_approval BOOLEAN NOT NULL,
    actor_user_id TEXT NOT NULL,
    actor_role TEXT NOT NULL,
    correlation_id TEXT NOT NULL UNIQUE,
    idempotency_key TEXT UNIQUE,
    request_hash CHAR(64),
    ledger_transaction_posted BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_admin_member_actions_target_created
    ON admin_member_actions(target_user_id, created_at DESC);
CREATE INDEX idx_admin_member_actions_status_created
    ON admin_member_actions(status, created_at DESC);
