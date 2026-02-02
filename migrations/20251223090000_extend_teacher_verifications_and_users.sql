-- Add admin review metadata to teacher_verifications
ALTER TABLE teacher_verifications
    ADD COLUMN IF NOT EXISTS admin_note TEXT,
    ADD COLUMN IF NOT EXISTS reviewed_by INTEGER,
    ADD COLUMN IF NOT EXISTS updated_at TIMESTAMP DEFAULT now();

-- Index for faster status filtering
CREATE INDEX IF NOT EXISTS idx_teacher_verifications_status ON teacher_verifications(status);
CREATE INDEX IF NOT EXISTS idx_teacher_verifications_created_at ON teacher_verifications(created_at);

-- Add active flag to users for enable/disable
ALTER TABLE users
    ADD COLUMN IF NOT EXISTS active BOOLEAN DEFAULT TRUE;
