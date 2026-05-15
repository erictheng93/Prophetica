CREATE TABLE affiliate_commission_adjustment_reviews (
    adjustment_id TEXT PRIMARY KEY,
    affiliate_id TEXT NOT NULL REFERENCES affiliate_profiles(affiliate_id) ON DELETE CASCADE,
    adjustment_type TEXT NOT NULL CHECK (adjustment_type IN ('credit', 'debit')),
    amount_minor BIGINT NOT NULL CHECK (amount_minor > 0),
    currency TEXT NOT NULL DEFAULT 'USDT',
    reason TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'reviewing', 'approved', 'rejected')),
    requested_by TEXT NOT NULL,
    requested_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    reviewed_by TEXT,
    reviewed_at TIMESTAMPTZ,
    audit_correlation_ids TEXT[] NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT affiliate_commission_adjustment_review_state CHECK (
        (status IN ('approved', 'rejected') AND reviewed_by IS NOT NULL AND reviewed_at IS NOT NULL)
        OR status IN ('pending', 'reviewing')
    )
);

CREATE INDEX idx_affiliate_commission_adjustment_reviews_status
    ON affiliate_commission_adjustment_reviews(status, requested_at);

CREATE INDEX idx_affiliate_commission_adjustment_reviews_affiliate
    ON affiliate_commission_adjustment_reviews(affiliate_id, requested_at DESC);
