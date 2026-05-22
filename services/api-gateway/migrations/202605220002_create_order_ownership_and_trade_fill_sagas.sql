CREATE TABLE IF NOT EXISTS api_gateway_order_owners (
    order_id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    market_id TEXT NOT NULL,
    matching_user_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_api_gateway_order_owners_user_market
    ON api_gateway_order_owners(user_id, market_id);

CREATE TABLE IF NOT EXISTS api_gateway_trade_fill_sagas (
    fill_key TEXT PRIMARY KEY,
    market_id TEXT NOT NULL,
    fill_id TEXT NOT NULL,
    taker_user_id TEXT NOT NULL,
    maker_user_id TEXT NOT NULL,
    buyer_user_id TEXT NOT NULL,
    seller_user_id TEXT NOT NULL,
    taker_order_id TEXT NOT NULL,
    maker_order_id TEXT NOT NULL,
    taker_side TEXT NOT NULL,
    maker_side TEXT NOT NULL,
    outcome TEXT NOT NULL,
    price_pts INTEGER NOT NULL CHECK (price_pts >= 0),
    quantity BIGINT NOT NULL CHECK (quantity >= 0),
    reservation_txn_id BIGINT,
    ledger_txn_id BIGINT,
    settlement_idempotency_key TEXT NOT NULL,
    taker_projection_idempotency_key TEXT NOT NULL,
    maker_projection_idempotency_key TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'running', 'retryable', 'completed')),
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    last_error TEXT,
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    settlement_posted_at TIMESTAMPTZ,
    taker_projected_at TIMESTAMPTZ,
    maker_projected_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (market_id, fill_id)
);

CREATE INDEX IF NOT EXISTS idx_api_gateway_trade_fill_sagas_due
    ON api_gateway_trade_fill_sagas(status, next_attempt_at, created_at)
    WHERE status IN ('pending', 'retryable', 'running');
