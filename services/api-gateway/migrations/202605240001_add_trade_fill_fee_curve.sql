ALTER TABLE api_gateway_trade_fill_sagas
    ADD COLUMN IF NOT EXISTS fee_curve_bps INTEGER NOT NULL DEFAULT 400 -- sdk::ledger::DEFAULT_TAKER_FEE_CURVE_BPS
    CHECK (fee_curve_bps >= 0);
