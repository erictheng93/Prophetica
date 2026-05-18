ALTER TABLE affiliate_profiles
    ADD COLUMN IF NOT EXISTS profile_uuid UUID;

UPDATE affiliate_profiles
SET profile_uuid = (
    substr(md5(affiliate_id), 1, 8) || '-' ||
    substr(md5(affiliate_id), 9, 4) || '-' ||
    substr(md5(affiliate_id), 13, 4) || '-' ||
    substr(md5(affiliate_id), 17, 4) || '-' ||
    substr(md5(affiliate_id), 21, 12)
)::UUID
WHERE profile_uuid IS NULL;

ALTER TABLE affiliate_profiles
    ALTER COLUMN profile_uuid SET NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_affiliate_profiles_profile_uuid
    ON affiliate_profiles(profile_uuid);
