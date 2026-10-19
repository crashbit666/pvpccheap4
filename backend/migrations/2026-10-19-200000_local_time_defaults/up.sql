-- All naive timestamps are Europe/Madrid wall time, like prices.timestamp and
-- scheduled_executions.scheduled_hour. Until now these columns were filled with
-- DEFAULT now() (the server runs in UTC) or Utc::now(), so they were stored in UTC.

UPDATE users SET created_at = (created_at AT TIME ZONE 'UTC') AT TIME ZONE 'Europe/Madrid' WHERE created_at IS NOT NULL;
ALTER TABLE users ALTER COLUMN created_at SET DEFAULT (now() AT TIME ZONE 'Europe/Madrid');
UPDATE user_integrations SET created_at = (created_at AT TIME ZONE 'UTC') AT TIME ZONE 'Europe/Madrid' WHERE created_at IS NOT NULL;
ALTER TABLE user_integrations ALTER COLUMN created_at SET DEFAULT (now() AT TIME ZONE 'Europe/Madrid');
UPDATE automation_rules SET created_at = (created_at AT TIME ZONE 'UTC') AT TIME ZONE 'Europe/Madrid' WHERE created_at IS NOT NULL;
ALTER TABLE automation_rules ALTER COLUMN created_at SET DEFAULT (now() AT TIME ZONE 'Europe/Madrid');
UPDATE automation_rules SET updated_at = (updated_at AT TIME ZONE 'UTC') AT TIME ZONE 'Europe/Madrid' WHERE updated_at IS NOT NULL;
ALTER TABLE automation_rules ALTER COLUMN updated_at SET DEFAULT (now() AT TIME ZONE 'Europe/Madrid');
UPDATE automation_rules SET last_triggered_at = (last_triggered_at AT TIME ZONE 'UTC') AT TIME ZONE 'Europe/Madrid' WHERE last_triggered_at IS NOT NULL;
UPDATE rule_executions SET executed_at = (executed_at AT TIME ZONE 'UTC') AT TIME ZONE 'Europe/Madrid' WHERE executed_at IS NOT NULL;
ALTER TABLE rule_executions ALTER COLUMN executed_at SET DEFAULT (now() AT TIME ZONE 'Europe/Madrid');
UPDATE scheduled_executions SET created_at = (created_at AT TIME ZONE 'UTC') AT TIME ZONE 'Europe/Madrid' WHERE created_at IS NOT NULL;
ALTER TABLE scheduled_executions ALTER COLUMN created_at SET DEFAULT (now() AT TIME ZONE 'Europe/Madrid');
UPDATE schedules SET created_at = (created_at AT TIME ZONE 'UTC') AT TIME ZONE 'Europe/Madrid' WHERE created_at IS NOT NULL;
ALTER TABLE schedules ALTER COLUMN created_at SET DEFAULT (now() AT TIME ZONE 'Europe/Madrid');
