CREATE TABLE api_gateway_affiliate_account_sagas (
    id BIGSERIAL PRIMARY KEY,
    idempotency_key TEXT NOT NULL UNIQUE,
    request_hash CHAR(64) NOT NULL,
    actor_id TEXT NOT NULL,
    actor_role TEXT NOT NULL CHECK (actor_role IN ('admin', 'manager')),
    target_user_id TEXT NOT NULL,
    affiliate_id TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN (
        'pending',
        'credential_created',
        'profile_created',
        'completed',
        'compensating',
        'failed'
    )),
    last_error TEXT,
    response_payload JSONB,
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_api_gateway_affiliate_account_sagas_status
    ON api_gateway_affiliate_account_sagas(status, updated_at DESC);

CREATE INDEX idx_api_gateway_affiliate_account_sagas_target
    ON api_gateway_affiliate_account_sagas(target_user_id, created_at DESC);
