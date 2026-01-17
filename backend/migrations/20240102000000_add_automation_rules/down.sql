-- Remove automation rules
DROP TABLE IF EXISTS rule_executions;
DROP TABLE IF EXISTS automation_rules;

-- Remove added columns from user_integrations
ALTER TABLE user_integrations
DROP COLUMN IF EXISTS session_data,
DROP COLUMN IF EXISTS session_expires_at;
