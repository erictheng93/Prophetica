ALTER TABLE commission_accruals
    ADD COLUMN referrer_group_id TEXT;

CREATE INDEX idx_commission_accruals_group_month
    ON commission_accruals(referrer_group_id, accrued_at)
    WHERE referrer_group_id IS NOT NULL;
