ALTER TABLE affiliate_profiles
    DROP CONSTRAINT IF EXISTS affiliate_profiles_status_check;

ALTER TABLE affiliate_profiles
    ADD CONSTRAINT affiliate_profiles_status_check
    CHECK (status IN ('active', 'review', 'frozen', 'onboarding', 'archived'));

CREATE TABLE affiliate_operation_audit_logs (
    id BIGSERIAL PRIMARY KEY,
    correlation_id TEXT NOT NULL UNIQUE,
    affiliate_id TEXT NOT NULL REFERENCES affiliate_profiles(affiliate_id),
    action TEXT NOT NULL CHECK (action IN ('create', 'update', 'archive')),
    actor_id TEXT NOT NULL,
    actor_role TEXT NOT NULL CHECK (actor_role IN ('admin', 'manager')),
    reason TEXT NOT NULL,
    authorization_basis TEXT NOT NULL,
    before_summary TEXT NOT NULL DEFAULT '',
    after_patch TEXT NOT NULL DEFAULT '',
    request_id TEXT,
    ip TEXT,
    user_agent TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_affiliate_operation_audit_logs_affiliate
    ON affiliate_operation_audit_logs(affiliate_id, created_at DESC);
