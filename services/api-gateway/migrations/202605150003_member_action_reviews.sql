ALTER TABLE admin_member_actions
    DROP CONSTRAINT admin_member_actions_status_check,
    ADD CONSTRAINT admin_member_actions_status_check CHECK (
        status IN ('submitted', 'pending_second_approval', 'approved', 'rejected')
    ),
    ADD COLUMN reviewer_user_id TEXT,
    ADD COLUMN reviewer_role TEXT,
    ADD COLUMN review_decision TEXT CHECK (review_decision IN ('approve', 'reject')),
    ADD COLUMN review_reason TEXT,
    ADD COLUMN reviewed_at TIMESTAMPTZ,
    ADD COLUMN side_effects JSONB NOT NULL DEFAULT '[]';

CREATE INDEX idx_admin_member_actions_reviewer_created
    ON admin_member_actions(reviewer_user_id, reviewed_at DESC)
    WHERE reviewer_user_id IS NOT NULL;
