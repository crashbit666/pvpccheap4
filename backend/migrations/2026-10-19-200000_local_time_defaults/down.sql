-- Back to UTC values and DEFAULT now()

UPDATE users SET created_at = (created_at AT TIME ZONE 'Europe/Madrid') AT TIME ZONE 'UTC' WHERE created_at IS NOT NULL;
ALTER TABLE users ALTER COLUMN created_at SET DEFAULT now();
UPDATE user_integrations SET created_at = (created_at AT TIME ZONE 'Europe/Madrid') AT TIME ZONE 'UTC' WHERE created_at IS NOT NULL;
ALTER TABLE user_integrations ALTER COLUMN created_at SET DEFAULT now();
UPDATE automation_rules SET created_at = (created_at AT TIME ZONE 'Europe/Madrid') AT TIME ZONE 'UTC' WHERE created_at IS NOT NULL;
ALTER TABLE automation_rules ALTER COLUMN created_at SET DEFAULT now();
UPDATE automation_rules SET updated_at = (updated_at AT TIME ZONE 'Europe/Madrid') AT TIME ZONE 'UTC' WHERE updated_at IS NOT NULL;
ALTER TABLE automation_rules ALTER COLUMN updated_at SET DEFAULT now();
UPDATE automation_rules SET last_triggered_at = (last_triggered_at AT TIME ZONE 'Europe/Madrid') AT TIME ZONE 'UTC' WHERE last_triggered_at IS NOT NULL;
UPDATE rule_executions SET executed_at = (executed_at AT TIME ZONE 'Europe/Madrid') AT TIME ZONE 'UTC' WHERE executed_at IS NOT NULL;
ALTER TABLE rule_executions ALTER COLUMN executed_at SET DEFAULT now();
UPDATE scheduled_executions SET created_at = (created_at AT TIME ZONE 'Europe/Madrid') AT TIME ZONE 'UTC' WHERE created_at IS NOT NULL;
ALTER TABLE scheduled_executions ALTER COLUMN created_at SET DEFAULT now();
UPDATE schedules SET created_at = (created_at AT TIME ZONE 'Europe/Madrid') AT TIME ZONE 'UTC' WHERE created_at IS NOT NULL;
ALTER TABLE schedules ALTER COLUMN created_at SET DEFAULT now();
