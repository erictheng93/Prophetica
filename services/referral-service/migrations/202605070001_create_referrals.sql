CREATE TABLE referrals (
    id BIGSERIAL PRIMARY KEY,
    referrer_id TEXT NOT NULL,
    referred_id TEXT NOT NULL UNIQUE,
    referral_code VARCHAR(32) NOT NULL,
    referred_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    observation_window_ends_at TIMESTAMPTZ NOT NULL,
    qualification_evaluated_at TIMESTAMPTZ,
    qualified_at TIMESTAMPTZ,
    status VARCHAR(32) NOT NULL CHECK (status IN (
        'pending',
        'qualified',
        'disqualified',
        'pending_review',
        'pending_eddreview'
    )),
    signup_bonus_txn_id BIGINT,
    metadata JSONB NOT NULL DEFAULT '{}',
    CONSTRAINT referrals_no_self_referral CHECK (referrer_id <> referred_id),
    CONSTRAINT referrals_qualified_timestamp_required CHECK (
        (status = 'qualified' AND qualified_at IS NOT NULL)
        OR (status <> 'qualified')
    ),
    CONSTRAINT referrals_observation_window_after_signup CHECK (
        observation_window_ends_at >= referred_at + INTERVAL '30 days'
    )
);

CREATE INDEX idx_referrals_referrer ON referrals(referrer_id);
CREATE INDEX idx_referrals_pending_eval
    ON referrals(observation_window_ends_at)
    WHERE status = 'pending';
CREATE UNIQUE INDEX idx_referrals_code ON referrals(referral_code);

CREATE TABLE commission_accruals (
    id BIGSERIAL PRIMARY KEY,
    referrer_id TEXT NOT NULL,
    referred_id TEXT NOT NULL,
    trade_id TEXT NOT NULL UNIQUE,
    fee_amount BIGINT NOT NULL CHECK (fee_amount > 0),
    commission_amount BIGINT NOT NULL CHECK (commission_amount > 0),
    accrued_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    paid_out_txn_id BIGINT,
    paid_out_at TIMESTAMPTZ
);

CREATE INDEX idx_commission_pending
    ON commission_accruals(referrer_id)
    WHERE paid_out_txn_id IS NULL;
