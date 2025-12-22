-- Add migration script here (idempotent)
ALTER TABLE users ADD COLUMN IF NOT EXISTS full_name VARCHAR(255);