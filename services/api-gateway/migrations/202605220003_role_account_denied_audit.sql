CREATE TABLE api_gateway_denied_operation_audit_logs (
    id BIGSERIAL PRIMARY KEY,
    correlation_id TEXT NOT NULL UNIQUE,
    domain TEXT NOT NULL CHECK (domain IN ('member_account', 'affiliate_account', 'affiliate_profile', 'saga')),
    target_type TEXT NOT NULL,
    target_id TEXT,
    action TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    actor_role TEXT NOT NULL CHECK (actor_role IN ('admin', 'manager', 'affiliate', 'player')),
    reason_or_error_code TEXT NOT NULL,
    attempted_payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    idempotency_key TEXT,
    request_id TEXT,
    ip TEXT,
    user_agent TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_api_gateway_denied_operation_audit_logs_target
    ON api_gateway_denied_operation_audit_logs(domain, target_type, target_id, created_at DESC)
    WHERE target_id IS NOT NULL;

CREATE INDEX idx_api_gateway_denied_operation_audit_logs_actor
    ON api_gateway_denied_operation_audit_logs(actor_id, created_at DESC);

CREATE INDEX idx_api_gateway_denied_operation_audit_logs_idempotency
    ON api_gateway_denied_operation_audit_logs(idempotency_key, created_at DESC)
    WHERE idempotency_key IS NOT NULL;
