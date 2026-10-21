-- Bumped when the password changes: tokens signed with an older version stop working
ALTER TABLE users ADD COLUMN token_version INTEGER NOT NULL DEFAULT 0;
