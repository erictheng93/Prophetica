ALTER TABLE referrals
    DROP CONSTRAINT IF EXISTS referrals_signup_bonus_txn_id_fkey;

ALTER TABLE commission_accruals
    DROP CONSTRAINT IF EXISTS commission_accruals_paid_out_txn_id_fkey;
