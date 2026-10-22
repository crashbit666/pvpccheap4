DROP TABLE user_tokens;
DROP INDEX users_email_unique;
ALTER TABLE users DROP COLUMN email_verified;
ALTER TABLE users DROP COLUMN email;
