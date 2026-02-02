-- Add avatar_path and birthday columns if missing
ALTER TABLE users
    ADD COLUMN IF NOT EXISTS avatar_path TEXT,
    ADD COLUMN IF NOT EXISTS birthday DATE;
