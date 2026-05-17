CREATE TABLE IF NOT EXISTS user_login_credentials (
    user_id TEXT PRIMARY KEY REFERENCES user_members(user_id) ON DELETE CASCADE,
    email TEXT NOT NULL UNIQUE,
    phone TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    role TEXT NOT NULL DEFAULT 'player',
    affiliate_id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    disabled_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_user_login_credentials_email_active
    ON user_login_credentials (email)
    WHERE disabled_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_user_login_credentials_phone_active
    ON user_login_credentials (phone)
    WHERE disabled_at IS NULL;
