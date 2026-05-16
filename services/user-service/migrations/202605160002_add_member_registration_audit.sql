ALTER TABLE user_members
    ADD COLUMN IF NOT EXISTS registration_audit JSONB;
