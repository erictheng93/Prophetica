CREATE TABLE referral_scope_audit_logs (
    id BIGSERIAL PRIMARY KEY,
    actor_service_principal TEXT NOT NULL,
    caller_role TEXT NOT NULL,
    affiliate_id TEXT NOT NULL,
    subject_user_id TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    in_scope BOOLEAN NOT NULL,
    relationship TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_referral_scope_audit_correlation
    ON referral_scope_audit_logs(correlation_id);

CREATE INDEX idx_referral_scope_audit_affiliate_subject_created
    ON referral_scope_audit_logs(affiliate_id, subject_user_id, created_at DESC);
