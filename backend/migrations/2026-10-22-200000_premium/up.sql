-- Paid plan: unlocks the extra features (rules written in plain words)
ALTER TABLE users ADD COLUMN is_premium BOOLEAN NOT NULL DEFAULT FALSE;
