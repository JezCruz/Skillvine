-- Add KYC verification tracking to users
ALTER TABLE users
    ADD COLUMN IF NOT EXISTS kyc_verified BOOLEAN DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS kyc_verified_at TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS idx_users_kyc_verified ON users(kyc_verified);
