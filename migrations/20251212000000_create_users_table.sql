-- migration: create users table (minimal)
-- This migration is intentionally idempotent (IF NOT EXISTS) for local testing.

CREATE TABLE IF NOT EXISTS users (
    id SERIAL PRIMARY KEY,
    full_name TEXT NOT NULL,
    email TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    role TEXT,
    verified BOOLEAN DEFAULT FALSE,
    verification_token TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT now()
);
