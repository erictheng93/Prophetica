CREATE TABLE drill_logs (
    drill_id VARCHAR(128) PRIMARY KEY,
    type VARCHAR(64) NOT NULL,
    scheduled_at TIMESTAMPTZ NOT NULL,
    executed_at TIMESTAMPTZ NOT NULL,
    rto_ms BIGINT NOT NULL CHECK (rto_ms >= 0),
    rpo_lost_events BIGINT NOT NULL CHECK (rpo_lost_events >= 0),
    deviation_notes TEXT NOT NULL DEFAULT '',
    improvement_actions TEXT NOT NULL DEFAULT '',
    document JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_drill_logs_executed_at ON drill_logs(executed_at);
CREATE INDEX idx_drill_logs_type ON drill_logs(type);
