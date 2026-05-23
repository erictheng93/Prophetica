CREATE TABLE member_account_denied_audit_logs (
    id BIGSERIAL PRIMARY KEY,
    correlation_id TEXT NOT NULL UNIQUE,
    target_user_id TEXT,
    action TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    actor_role TEXT NOT NULL CHECK (actor_role IN ('admin', 'manager', 'affiliate', 'player')),
    reason_or_error_code TEXT NOT NULL,
    attempted_payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    request_id TEXT,
    ip TEXT,
    user_agent TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_member_account_denied_audit_logs_target
    ON member_account_denied_audit_logs(target_user_id, created_at DESC)
    WHERE target_user_id IS NOT NULL;

CREATE INDEX idx_member_account_denied_audit_logs_actor
    ON member_account_denied_audit_logs(actor_id, created_at DESC);
