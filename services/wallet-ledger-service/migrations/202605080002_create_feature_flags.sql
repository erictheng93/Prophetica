CREATE TABLE IF NOT EXISTS feature_flags (
    key TEXT PRIMARY KEY,
    enabled BOOLEAN NOT NULL DEFAULT FALSE,
    subject_id TEXT,
    reason TEXT,
    updated_by TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    CONSTRAINT feature_flags_engine_halted_subject
        CHECK (key <> 'engine.halted' OR enabled = FALSE OR subject_id IS NOT NULL)
);

INSERT INTO feature_flags (key, enabled, updated_by)
VALUES ('engine.halted', FALSE, 'system:migration')
ON CONFLICT (key) DO NOTHING;

INSERT INTO feature_flags (key, enabled, updated_by)
VALUES ('payments.real_money_enabled', FALSE, 'system:migration')
ON CONFLICT (key) DO NOTHING;
