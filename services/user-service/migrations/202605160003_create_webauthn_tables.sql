CREATE TABLE IF NOT EXISTS user_webauthn_credentials (
    credential_id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    user_webauthn_uuid UUID NOT NULL,
    display_name TEXT NOT NULL,
    public_key_cose JSONB NOT NULL,
    signature_counter BIGINT NOT NULL DEFAULT 0,
    transports JSONB NOT NULL DEFAULT '[]'::jsonb,
    user_verified BOOLEAN NOT NULL DEFAULT FALSE,
    backup_eligible BOOLEAN NOT NULL DEFAULT FALSE,
    backup_state BOOLEAN NOT NULL DEFAULT FALSE,
    registration_policy TEXT NOT NULL DEFAULT 'required',
    attestation_format TEXT,
    attestation JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_used_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb
);

CREATE INDEX IF NOT EXISTS idx_user_webauthn_credentials_user_active
    ON user_webauthn_credentials (user_id, revoked_at);

CREATE UNIQUE INDEX IF NOT EXISTS idx_user_webauthn_credentials_user_uuid
    ON user_webauthn_credentials (user_id, user_webauthn_uuid)
    WHERE revoked_at IS NULL;

CREATE TABLE IF NOT EXISTS user_webauthn_challenges (
    challenge_id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    ceremony TEXT NOT NULL CHECK (ceremony IN ('registration', 'assertion')),
    state_json JSONB NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    consumed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_user_webauthn_challenges_user_ceremony_active
    ON user_webauthn_challenges (user_id, ceremony, expires_at)
    WHERE consumed_at IS NULL;

