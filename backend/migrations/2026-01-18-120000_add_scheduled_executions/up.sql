-- Add scheduled_executions table to track intended schedule and actual execution status
CREATE TABLE scheduled_executions (
    id SERIAL PRIMARY KEY,
    rule_id INTEGER NOT NULL REFERENCES automation_rules(id) ON DELETE CASCADE,
    scheduled_hour TIMESTAMP NOT NULL,
    expected_action TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    executed_at TIMESTAMP,
    execution_id INTEGER REFERENCES rule_executions(id),
    retry_count INTEGER NOT NULL DEFAULT 0,
    last_retry_at TIMESTAMP,
    next_retry_at TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    UNIQUE(rule_id, scheduled_hour)
);

CREATE INDEX idx_scheduled_executions_status ON scheduled_executions(status);
CREATE INDEX idx_scheduled_executions_scheduled_hour ON scheduled_executions(scheduled_hour);
CREATE INDEX idx_scheduled_executions_next_retry ON scheduled_executions(next_retry_at) WHERE status = 'retrying';
CREATE INDEX idx_scheduled_executions_rule_date ON scheduled_executions(rule_id, scheduled_hour);
