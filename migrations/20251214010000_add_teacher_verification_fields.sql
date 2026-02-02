-- Extend teacher_verifications with more KYC fields
ALTER TABLE teacher_verifications
    ADD COLUMN IF NOT EXISTS full_name TEXT,
    ADD COLUMN IF NOT EXISTS dob DATE,
    ADD COLUMN IF NOT EXISTS gender TEXT,
    ADD COLUMN IF NOT EXISTS address TEXT,
    ADD COLUMN IF NOT EXISTS front_id_path TEXT,
    ADD COLUMN IF NOT EXISTS back_id_path TEXT;
