ALTER TABLE affiliate_commission_adjustment_reviews
    ADD COLUMN IF NOT EXISTS review_reason TEXT;

ALTER TABLE affiliate_payouts
    ADD COLUMN IF NOT EXISTS reviewed_by TEXT,
    ADD COLUMN IF NOT EXISTS reviewed_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS review_reason TEXT,
    ADD COLUMN IF NOT EXISTS audit_correlation_ids TEXT[] NOT NULL DEFAULT '{}';

ALTER TABLE affiliate_payouts
    DROP CONSTRAINT IF EXISTS affiliate_payouts_status_check;

ALTER TABLE affiliate_payouts
    ADD CONSTRAINT affiliate_payouts_status_check
    CHECK (status IN ('settled', 'pending', 'blocked', 'approved', 'rejected'));

ALTER TABLE affiliate_payouts
    ADD CONSTRAINT affiliate_payout_review_state
    CHECK (
        (status IN ('approved', 'rejected') AND reviewed_by IS NOT NULL AND reviewed_at IS NOT NULL)
        OR status IN ('settled', 'pending', 'blocked')
    );
