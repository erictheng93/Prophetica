CREATE TABLE member_account_audit_logs (
    id BIGSERIAL PRIMARY KEY,
    correlation_id TEXT NOT NULL UNIQUE,
    target_user_id TEXT NOT NULL,
    action TEXT NOT NULL CHECK (action IN ('create', 'update', 'disable', 'reassign')),
    actor_id TEXT NOT NULL,
    actor_role TEXT NOT NULL CHECK (actor_role = 'admin'),
    reason TEXT NOT NULL,
    authorization_basis TEXT NOT NULL,
    before_summary TEXT NOT NULL DEFAULT '',
    after_patch TEXT NOT NULL DEFAULT '',
    request_id TEXT,
    ip TEXT,
    user_agent TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_member_account_audit_logs_target
    ON member_account_audit_logs(target_user_id, created_at DESC);

CREATE INDEX idx_member_account_audit_logs_actor
    ON member_account_audit_logs(actor_id, created_at DESC);
