-- Drop unique constraint on full_name if present
ALTER TABLE users DROP CONSTRAINT IF EXISTS users_username_key;
DROP INDEX IF EXISTS users_username_key;
