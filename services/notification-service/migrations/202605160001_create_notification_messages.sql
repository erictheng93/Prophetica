CREATE TABLE IF NOT EXISTS notification_messages (
    id BIGSERIAL PRIMARY KEY,
    notification_id TEXT GENERATED ALWAYS AS ('notif-' || lpad(id::text, 6, '0')) STORED UNIQUE,
    dedupe_key TEXT NOT NULL,
    reminder_type TEXT NOT NULL CHECK (reminder_type IN ('pending_case', 'approval_required')),
    recipient_role TEXT NOT NULL,
    recipient_user_id TEXT NOT NULL DEFAULT '',
    channel TEXT NOT NULL CHECK (channel IN ('email', 'in_app')),
    delivery_address TEXT,
    subject TEXT NOT NULL,
    body TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'queued' CHECK (status IN ('queued', 'sent', 'failed', 'cancelled')),
    target_user_id TEXT,
    action_id TEXT,
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    delivered_at TIMESTAMPTZ,
    UNIQUE (dedupe_key, recipient_role, recipient_user_id, channel)
);

CREATE INDEX IF NOT EXISTS idx_notification_messages_recipient_status
    ON notification_messages (recipient_role, status, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_notification_messages_target_user
    ON notification_messages (target_user_id)
    WHERE target_user_id IS NOT NULL;
