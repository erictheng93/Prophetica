-- SPEC-024 §3.4 / §7 P5.1: cancel-window enforcement + commit timestamp tracking.
--
-- Adds:
--   - skus.cancel_window_seconds : per-SKU cancel-window length. Default 300s
--     mirrors the migration-level default for sku_stock.reservation_ttl_seconds
--     and matches catalog::DEFAULT_CANCEL_WINDOW_SECONDS in the Rust types.
--   - redemption_orders.committed_at : the exact moment state moved to
--     'committed'. Cancel deadline = committed_at + cancel_window_seconds.
--     `updated_at` already exists but bumps on every UPDATE, so it can drift
--     past the commit moment as the order progresses; this column is sticky.
--
-- `redemption_orders.expires_at` was already present in
-- 202605270010_create_redemption_schema.sql but never written. The P5.1
-- service layer now populates it on commit so the P5.2 expiry sweep can
-- query `WHERE state='committed' AND expires_at < NOW()` directly.

ALTER TABLE skus
    ADD COLUMN cancel_window_seconds INTEGER NOT NULL DEFAULT 300
    CHECK (cancel_window_seconds > 0);

ALTER TABLE redemption_orders
    ADD COLUMN committed_at TIMESTAMPTZ;

-- Index for the P5.2 expiry sweep query — partial index keeps the cost on
-- writes proportional only to the (typically tiny) `committed` set.
CREATE INDEX idx_orders_committed_expires
    ON redemption_orders(expires_at)
    WHERE state = 'committed' AND expires_at IS NOT NULL;
