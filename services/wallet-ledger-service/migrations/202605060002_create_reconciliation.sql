CREATE TABLE reconciliation_runs (
    id BIGSERIAL PRIMARY KEY,
    layer SMALLINT NOT NULL CHECK (layer IN (2, 3, 4)),
    status VARCHAR(32) NOT NULL CHECK (status IN ('started', 'completed', 'failed')),
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ,
    metadata JSONB NOT NULL DEFAULT '{}'
);

CREATE INDEX idx_reconciliation_runs_layer_started
    ON reconciliation_runs(layer, started_at DESC);

CREATE TABLE reconciliation_findings (
    id BIGSERIAL PRIMARY KEY,
    run_id BIGINT NOT NULL REFERENCES reconciliation_runs(id),
    severity VARCHAR(16) NOT NULL CHECK (severity IN ('info', 'warning', 'critical')),
    finding_type VARCHAR(64) NOT NULL,
    account_code VARCHAR(128),
    expected_amount BIGINT NOT NULL,
    actual_amount BIGINT NOT NULL,
    difference_amount BIGINT NOT NULL,
    details JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    resolved_at TIMESTAMPTZ
);

CREATE INDEX idx_reconciliation_findings_run
    ON reconciliation_findings(run_id);

CREATE INDEX idx_reconciliation_findings_unresolved
    ON reconciliation_findings(created_at DESC)
    WHERE resolved_at IS NULL;
