CREATE TABLE affiliate_profiles (
    affiliate_id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    email_masked TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('active', 'review', 'frozen', 'onboarding')),
    owner_sponsor_id TEXT,
    commission_rate_bps INTEGER NOT NULL CHECK (commission_rate_bps >= 0),
    tier SMALLINT NOT NULL CHECK (tier IN (1, 2, 3)),
    last_activity_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    risk_flags TEXT[] NOT NULL DEFAULT '{}',
    credit_rating TEXT NOT NULL DEFAULT 'restricted',
    credit_status TEXT NOT NULL DEFAULT 'pending_review',
    max_active_credit_accounts INTEGER NOT NULL DEFAULT 0,
    total_credit_limit_minor_units BIGINT NOT NULL DEFAULT 0,
    per_account_limit_minor_units BIGINT NOT NULL DEFAULT 0,
    credit_review_expires_at DATE,
    credit_last_changed_by TEXT NOT NULL DEFAULT 'system',
    credit_last_changed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    credit_audit_correlation_ids TEXT[] NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_affiliate_profiles_owner ON affiliate_profiles(owner_sponsor_id);
CREATE INDEX idx_affiliate_profiles_status ON affiliate_profiles(status);

CREATE TABLE affiliate_sub_tree_stats (
    affiliate_id TEXT NOT NULL REFERENCES affiliate_profiles(affiliate_id) ON DELETE CASCADE,
    tier SMALLINT NOT NULL CHECK (tier IN (1, 2, 3)),
    count INTEGER NOT NULL DEFAULT 0 CHECK (count >= 0),
    active_count INTEGER NOT NULL DEFAULT 0 CHECK (active_count >= 0),
    mtd_contribution_minor BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (affiliate_id, tier)
);

CREATE TABLE affiliate_payouts (
    payout_id TEXT PRIMARY KEY,
    affiliate_id TEXT NOT NULL REFERENCES affiliate_profiles(affiliate_id) ON DELETE CASCADE,
    amount_minor BIGINT NOT NULL CHECK (amount_minor >= 0),
    currency TEXT NOT NULL DEFAULT 'USDT',
    settled_at DATE NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('settled', 'pending', 'blocked')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_affiliate_payouts_affiliate_settled ON affiliate_payouts(affiliate_id, settled_at DESC);

CREATE TABLE affiliate_rate_changes (
    id BIGSERIAL PRIMARY KEY,
    affiliate_id TEXT NOT NULL REFERENCES affiliate_profiles(affiliate_id) ON DELETE CASCADE,
    effective_at DATE NOT NULL,
    previous_bps INTEGER NOT NULL CHECK (previous_bps >= 0),
    next_bps INTEGER NOT NULL CHECK (next_bps >= 0),
    changed_by TEXT NOT NULL,
    reason TEXT NOT NULL
);

CREATE INDEX idx_affiliate_rate_changes_affiliate_effective ON affiliate_rate_changes(affiliate_id, effective_at DESC);

CREATE TABLE affiliate_credit_accounts (
    account_id TEXT PRIMARY KEY,
    affiliate_id TEXT NOT NULL REFERENCES affiliate_profiles(affiliate_id) ON DELETE CASCADE,
    status TEXT NOT NULL CHECK (status IN ('pending_review', 'active', 'limited', 'frozen', 'closed')),
    credit_limit_minor_units BIGINT NOT NULL DEFAULT 0,
    used_minor_units BIGINT NOT NULL DEFAULT 0,
    expires_at DATE
);

CREATE INDEX idx_affiliate_credit_accounts_affiliate ON affiliate_credit_accounts(affiliate_id);
