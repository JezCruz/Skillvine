-- Ensure teacher_verifications table exists
CREATE TABLE IF NOT EXISTS teacher_verifications (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL,
    status TEXT NOT NULL,
    id_path TEXT,
    full_name TEXT,
    dob DATE,
    gender TEXT,
    address TEXT,
    front_id_path TEXT,
    back_id_path TEXT,
    created_at TIMESTAMP DEFAULT now()
);
