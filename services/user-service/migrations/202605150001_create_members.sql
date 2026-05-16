CREATE TABLE user_members (
    user_id TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    status VARCHAR(32) NOT NULL CHECK (status IN ('inactive', 'active', 'restricted', 'suspended')),
    tier TEXT NOT NULL,
    email TEXT NOT NULL,
    phone TEXT NOT NULL,
    line_id TEXT,
    country CHAR(2) NOT NULL DEFAULT 'TW',
    timezone TEXT NOT NULL DEFAULT 'Asia/Taipei',
    affiliate_id TEXT,
    kyc_status VARCHAR(32) NOT NULL CHECK (kyc_status IN ('pending', 'verified', 'review_required')),
    risk_level VARCHAR(32) NOT NULL CHECK (risk_level IN ('low', 'medium', 'high')),
    risk_score INTEGER NOT NULL CHECK (risk_score BETWEEN 0 AND 100),
    restrictions JSONB NOT NULL DEFAULT '[]',
    last_reviewed_at TIMESTAMPTZ,
    last_login_at TIMESTAMPTZ,
    device_count INTEGER NOT NULL DEFAULT 0 CHECK (device_count >= 0),
    two_factor_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    registration_audit JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at TIMESTAMPTZ
);

CREATE INDEX idx_user_members_affiliate ON user_members(affiliate_id) WHERE affiliate_id IS NOT NULL;
CREATE INDEX idx_user_members_status ON user_members(status) WHERE deleted_at IS NULL;
CREATE INDEX idx_user_members_email_lower ON user_members(LOWER(email)) WHERE deleted_at IS NULL;
