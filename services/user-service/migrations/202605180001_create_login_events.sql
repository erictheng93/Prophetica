CREATE TYPE login_principal_type AS ENUM (
    'user',
    'admin',
    'manager',
    'affiliate'
);

CREATE TYPE login_event_type AS ENUM (
    'login_success',
    'login_failure',
    'session_refresh'
);

CREATE TYPE login_geo_accuracy AS ENUM (
    'approximate',
    'unknown'
);

CREATE TYPE login_device_type AS ENUM (
    'desktop',
    'mobile',
    'tablet',
    'bot',
    'unknown'
);

CREATE TABLE login_events (
    id TEXT PRIMARY KEY,
    login_surface TEXT NOT NULL,
    principal_type login_principal_type NOT NULL,
    principal_id TEXT,
    user_id TEXT,
    principal_external_id TEXT,
    account_identifier_hash TEXT,
    account_identifier_hash_key_version TEXT,
    event_type login_event_type NOT NULL,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    ip_address INET NOT NULL,
    ip_source TEXT NOT NULL,
    geo_country TEXT,
    geo_region TEXT,
    geo_city TEXT,
    geo_latitude NUMERIC(9, 6),
    geo_longitude NUMERIC(9, 6),
    geo_source TEXT,
    geo_accuracy login_geo_accuracy NOT NULL DEFAULT 'unknown',
    asn TEXT,
    isp TEXT,
    user_agent_raw TEXT NOT NULL DEFAULT '',
    browser_name TEXT,
    browser_version TEXT,
    os_name TEXT,
    os_version TEXT,
    device_type login_device_type NOT NULL DEFAULT 'unknown',
    device_vendor TEXT,
    device_model TEXT,
    session_id TEXT,
    auth_method TEXT NOT NULL,
    failure_reason TEXT,
    risk_flags JSONB NOT NULL DEFAULT '[]'::jsonb,
    request_id TEXT,
    correlation_id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT login_events_user_id_scope
        CHECK (user_id IS NULL OR principal_type = 'user'),
    CONSTRAINT login_events_success_requires_identity
        CHECK (
            event_type <> 'login_success'
            OR (
                principal_id IS NOT NULL
                AND (principal_type <> 'user' OR user_id IS NOT NULL)
            )
        ),
    CONSTRAINT login_events_failure_has_identifier
        CHECK (
            event_type <> 'login_failure'
            OR principal_id IS NOT NULL
            OR account_identifier_hash IS NOT NULL
            OR failure_reason = 'invalid_identifier'
        ),
    CONSTRAINT login_events_identifier_hash_key_version_pair
        CHECK (
            (account_identifier_hash IS NULL AND account_identifier_hash_key_version IS NULL)
            OR (account_identifier_hash IS NOT NULL AND account_identifier_hash_key_version IS NOT NULL)
        ),
    CONSTRAINT login_events_geo_source_required
        CHECK (geo_accuracy <> 'approximate' OR geo_source IS NOT NULL)
);

CREATE INDEX login_events_principal_time_idx
    ON login_events (principal_type, principal_id, occurred_at DESC);

CREATE INDEX login_events_user_time_idx
    ON login_events (user_id, occurred_at DESC)
    WHERE user_id IS NOT NULL;

CREATE INDEX login_events_ip_time_idx
    ON login_events (ip_address, occurred_at DESC);

CREATE INDEX login_events_event_type_time_idx
    ON login_events (event_type, occurred_at DESC);

CREATE INDEX login_events_identifier_hash_time_idx
    ON login_events (account_identifier_hash, occurred_at DESC)
    WHERE account_identifier_hash IS NOT NULL;
