-- Add automation rules table (more flexible than schedules)
CREATE TABLE automation_rules (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    device_id INTEGER NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    name TEXT NOT NULL,

    -- Rule type: "price_threshold", "cheapest_hours", "time_schedule", "manual"
    rule_type TEXT NOT NULL,

    -- Action to perform: "turn_on", "turn_off", "toggle"
    action TEXT NOT NULL,

    -- Configuration as JSON for flexibility
    -- For price_threshold: {"threshold": 0.10, "comparison": "below"}
    -- For cheapest_hours: {"hours_needed": 3, "window_start": "00:00", "window_end": "08:00"}
    -- For time_schedule: {"days": ["mon", "tue"], "time": "06:00"}
    config JSONB NOT NULL DEFAULT '{}',

    -- Rule state
    is_enabled BOOLEAN NOT NULL DEFAULT true,

    -- Priority (lower = higher priority, for conflict resolution)
    priority INTEGER NOT NULL DEFAULT 100,

    -- Timestamps
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    last_triggered_at TIMESTAMP
);

-- Index for quick lookups
CREATE INDEX idx_automation_rules_user_id ON automation_rules(user_id);
CREATE INDEX idx_automation_rules_device_id ON automation_rules(device_id);
CREATE INDEX idx_automation_rules_enabled ON automation_rules(is_enabled) WHERE is_enabled = true;

-- Execution log for debugging and history
CREATE TABLE rule_executions (
    id SERIAL PRIMARY KEY,
    rule_id INTEGER NOT NULL REFERENCES automation_rules(id) ON DELETE CASCADE,
    executed_at TIMESTAMP NOT NULL DEFAULT NOW(),
    action_taken TEXT NOT NULL,
    success BOOLEAN NOT NULL,
    error_message TEXT,

    -- Context at execution time
    price_at_execution DOUBLE PRECISION,
    device_state_before JSONB,
    device_state_after JSONB
);

CREATE INDEX idx_rule_executions_rule_id ON rule_executions(rule_id);
CREATE INDEX idx_rule_executions_executed_at ON rule_executions(executed_at);

-- Add token/session storage for provider integrations
-- This allows caching authenticated sessions to avoid repeated logins
ALTER TABLE user_integrations
ADD COLUMN IF NOT EXISTS session_data JSONB,
ADD COLUMN IF NOT EXISTS session_expires_at TIMESTAMP;
