CREATE TABLE user_kyc_records (
    id BIGSERIAL PRIMARY KEY,
    user_id TEXT NOT NULL,
    tier SMALLINT NOT NULL,
    status VARCHAR(32) NOT NULL,
    provider VARCHAR(32) NOT NULL,
    provider_applicant_id VARCHAR(128),
    review_result JSONB,
    sanction_match JSONB,
    pep_match JSONB,
    document_id_hash CHAR(64),
    country CHAR(2),
    declared_monthly_income BIGINT,
    submitted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    reviewed_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ,
    metadata JSONB NOT NULL DEFAULT '{}',
    idempotency_key VARCHAR(128) NOT NULL UNIQUE
);

CREATE INDEX idx_kyc_user ON user_kyc_records(user_id, tier);
CREATE INDEX idx_kyc_expires ON user_kyc_records(expires_at) WHERE expires_at IS NOT NULL;
CREATE INDEX idx_kyc_provider_applicant ON user_kyc_records(provider, provider_applicant_id)
    WHERE provider_applicant_id IS NOT NULL;
CREATE UNIQUE INDEX idx_kyc_document_hash_unique ON user_kyc_records(document_id_hash)
    WHERE document_id_hash IS NOT NULL;

CREATE TABLE kyc_webhook_events (
    id BIGSERIAL PRIMARY KEY,
    provider VARCHAR(32) NOT NULL,
    provider_event_id VARCHAR(128),
    raw_payload_sha256 CHAR(64) NOT NULL UNIQUE,
    payload JSONB NOT NULL,
    processed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    record_id BIGINT REFERENCES user_kyc_records(id)
);

CREATE INDEX idx_kyc_webhook_provider_event ON kyc_webhook_events(provider, provider_event_id)
    WHERE provider_event_id IS NOT NULL;

CREATE TABLE aml_events (
    id BIGSERIAL PRIMARY KEY,
    user_id TEXT NOT NULL,
    event_type VARCHAR(64) NOT NULL,
    amount_minor_units BIGINT NOT NULL,
    ml_score INTEGER NOT NULL CHECK (ml_score BETWEEN 0 AND 100),
    rule_flags JSONB NOT NULL DEFAULT '[]',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_aml_events_user_created ON aml_events(user_id, created_at DESC);

CREATE TABLE sar_candidates (
    id BIGSERIAL PRIMARY KEY,
    candidate_key VARCHAR(128) NOT NULL UNIQUE,
    user_id TEXT NOT NULL,
    reason VARCHAR(32) NOT NULL,
    source_event_id BIGINT REFERENCES aml_events(id),
    status VARCHAR(32) NOT NULL DEFAULT 'needs_review',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    reviewed_at TIMESTAMPTZ,
    reviewed_by TEXT,
    review_notes TEXT
);

CREATE INDEX idx_sar_candidates_status_created ON sar_candidates(status, created_at DESC);

CREATE TABLE screening_rescan_events (
    id BIGSERIAL PRIMARY KEY,
    user_id TEXT NOT NULL,
    match_type VARCHAR(32) NOT NULL,
    list_name VARCHAR(128) NOT NULL,
    action VARCHAR(32) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_screening_rescan_user_created ON screening_rescan_events(user_id, created_at DESC);
