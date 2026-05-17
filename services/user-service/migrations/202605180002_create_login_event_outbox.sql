CREATE TABLE login_event_outbox (
    id TEXT PRIMARY KEY,
    login_event_id TEXT NOT NULL,
    payload JSONB NOT NULL,
    failure_reason TEXT NOT NULL,
    enqueued_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    processed_at TIMESTAMPTZ
);

CREATE INDEX login_event_outbox_unprocessed_idx
    ON login_event_outbox (enqueued_at ASC)
    WHERE processed_at IS NULL;
