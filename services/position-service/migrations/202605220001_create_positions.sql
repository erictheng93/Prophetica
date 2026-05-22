CREATE TABLE IF NOT EXISTS positions (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    market_id TEXT NOT NULL,
    outcome TEXT NOT NULL CHECK (outcome IN ('YES', 'NO')),
    qty BIGINT NOT NULL CHECK (qty >= 0),
    avg_price_pts INTEGER NOT NULL CHECK (avg_price_pts BETWEEN 0 AND 100),
    locked_amount BIGINT NOT NULL CHECK (locked_amount >= 0),
    opened_at TEXT NOT NULL,
    closed_at TEXT,
    settle_txn_id BIGINT,
    realized_pnl_points BIGINT
);

CREATE INDEX IF NOT EXISTS idx_positions_user_opened_at
    ON positions (user_id, opened_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_positions_user_active
    ON positions (user_id, closed_at)
    WHERE closed_at IS NULL;

CREATE TABLE IF NOT EXISTS position_fills (
    idempotency_key TEXT PRIMARY KEY,
    position_id TEXT NOT NULL REFERENCES positions(id),
    user_id TEXT NOT NULL,
    market_id TEXT NOT NULL,
    outcome TEXT NOT NULL CHECK (outcome IN ('YES', 'NO')),
    side TEXT NOT NULL CHECK (side IN ('BUY', 'SELL')),
    quantity BIGINT NOT NULL CHECK (quantity > 0),
    price_pts INTEGER NOT NULL CHECK (price_pts BETWEEN 0 AND 100),
    order_id TEXT NOT NULL,
    fill_id TEXT NOT NULL,
    ledger_txn_id BIGINT,
    projected_at TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (market_id, fill_id, user_id, side)
);

CREATE INDEX IF NOT EXISTS idx_position_fills_user_created_at
    ON position_fills (user_id, created_at DESC);
