ALTER TABLE affiliate_profiles
    ADD COLUMN managing_manager_id TEXT;

-- Existing affiliates remain in the admin pool until an admin explicitly assigns
-- a managing manager. This is intentionally separate from owner_sponsor_id,
-- which represents MLM referral sponsorship.
CREATE INDEX idx_affiliate_profiles_managing_manager
    ON affiliate_profiles(managing_manager_id);
