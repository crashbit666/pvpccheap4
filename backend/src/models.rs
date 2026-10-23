use chrono::{NaiveDateTime, NaiveTime};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

// ============================================================================
// Core Models
// ============================================================================

#[derive(Queryable, Selectable, Insertable, Serialize, Deserialize, Debug, Clone)]
#[diesel(table_name = crate::schema::users)]
pub struct User {
    pub id: i32,
    pub username: String,
    pub password_hash: String,
    pub created_at: NaiveDateTime,
    pub token_version: i32,
    pub is_premium: bool,
    pub email: Option<String>,
    pub email_verified: bool,
    pub zone: String,
    pub premium_until: Option<NaiveDateTime>,
    /// crate::services::tariff::Kind
    pub tariff: String,
    /// €/kWh the retailer adds to an indexed tariff
    pub tariff_margin: f64,
}

#[derive(Queryable, Selectable, Insertable, Serialize, Deserialize, Debug, Clone)]
#[diesel(table_name = crate::schema::prices)]
pub struct Price {
    pub timestamp: NaiveDateTime,
    pub price: f64,
    pub source: String,
    /// crate::services::tariff series code
    pub series: String,
    /// Length of the slot that starts at `timestamp`
    pub minutes: i16,
}

// ============================================================================
// Integration Models
// ============================================================================

#[derive(Queryable, Selectable, Serialize, Deserialize, Debug, Clone)]
#[diesel(table_name = crate::schema::user_integrations)]
pub struct UserIntegration {
    pub id: i32,
    pub user_id: i32,
    pub provider_name: String,
    // Never sent to clients: holds the provider email/password and session tokens
    #[serde(skip_serializing)]
    pub credentials_json: String,
    pub is_active: bool,
    pub created_at: NaiveDateTime,
    #[serde(skip_serializing)]
    pub session_data: Option<JsonValue>,
    pub session_expires_at: Option<NaiveDateTime>,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = crate::schema::user_integrations)]
pub struct NewUserIntegration {
    pub user_id: i32,
    pub provider_name: String,
    pub credentials_json: String,
    pub is_active: bool,
}

// ============================================================================
// Device Models
// ============================================================================

#[derive(Queryable, Selectable, Serialize, Deserialize, Debug, Clone)]
#[diesel(table_name = crate::schema::devices)]
pub struct Device {
    pub id: i32,
    pub integration_id: i32,
    pub external_id: String,
    pub name: String,
    pub device_type: String,
    pub is_managed: bool,
    pub is_on: bool,
    pub power_watts: Option<i32>,
    /// Whether the plug reports its power; None until asked
    pub measures_power: Option<bool>,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = crate::schema::devices)]
pub struct NewDevice {
    pub integration_id: i32,
    pub external_id: String,
    pub name: String,
    pub device_type: String,
    pub is_managed: bool,
}

// ============================================================================
// Automation Rule Models
// ============================================================================

/// Types of automation rules
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RuleType {
    /// Trigger when price is above/below threshold
    PriceThreshold,
    /// Find cheapest N hours within a time window
    CheapestHours,
    /// Simple time-based schedule
    TimeSchedule,
    /// Manual control (no automatic triggers)
    Manual,
}

impl RuleType {
    pub fn as_str(&self) -> &'static str {
        match self {
            RuleType::PriceThreshold => "price_threshold",
            RuleType::CheapestHours => "cheapest_hours",
            RuleType::TimeSchedule => "time_schedule",
            RuleType::Manual => "manual",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "price_threshold" => Some(RuleType::PriceThreshold),
            "cheapest_hours" => Some(RuleType::CheapestHours),
            "time_schedule" => Some(RuleType::TimeSchedule),
            "manual" => Some(RuleType::Manual),
            _ => None,
        }
    }
}

/// Actions that can be performed on a device
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RuleAction {
    TurnOn,
    TurnOff,
    Toggle,
}

impl RuleAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            RuleAction::TurnOn => "turn_on",
            RuleAction::TurnOff => "turn_off",
            RuleAction::Toggle => "toggle",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "turn_on" => Some(RuleAction::TurnOn),
            "turn_off" => Some(RuleAction::TurnOff),
            "toggle" => Some(RuleAction::Toggle),
            _ => None,
        }
    }
}

#[derive(Queryable, Selectable, Serialize, Deserialize, Debug, Clone)]
#[diesel(table_name = crate::schema::automation_rules)]
pub struct AutomationRule {
    pub id: i32,
    pub user_id: i32,
    pub device_id: i32,
    pub name: String,
    pub rule_type: String,
    pub action: String,
    pub config: JsonValue,
    pub is_enabled: bool,
    pub priority: i32,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub last_triggered_at: Option<NaiveDateTime>,
}

impl AutomationRule {
    pub fn get_rule_type(&self) -> Option<RuleType> {
        RuleType::parse(&self.rule_type)
    }

    pub fn get_action(&self) -> Option<RuleAction> {
        RuleAction::parse(&self.action)
    }
}

#[derive(Insertable, Debug)]
#[diesel(table_name = crate::schema::automation_rules)]
pub struct NewAutomationRule {
    pub user_id: i32,
    pub device_id: i32,
    pub name: String,
    pub rule_type: String,
    pub action: String,
    pub config: JsonValue,
    pub is_enabled: bool,
    pub priority: i32,
}

// ============================================================================
// Rule Execution Log
// ============================================================================

#[derive(Queryable, Selectable, Serialize, Deserialize, Debug, Clone)]
#[diesel(table_name = crate::schema::rule_executions)]
pub struct RuleExecution {
    pub id: i32,
    pub rule_id: i32,
    pub executed_at: NaiveDateTime,
    pub action_taken: String,
    pub success: bool,
    pub error_message: Option<String>,
    pub price_at_execution: Option<f64>,
    pub device_state_before: Option<JsonValue>,
    pub device_state_after: Option<JsonValue>,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = crate::schema::rule_executions)]
pub struct NewRuleExecution {
    pub rule_id: i32,
    pub action_taken: String,
    pub success: bool,
    pub error_message: Option<String>,
    pub price_at_execution: Option<f64>,
    pub device_state_before: Option<JsonValue>,
    pub device_state_after: Option<JsonValue>,
}

// ============================================================================
// Legacy Schedule Model (kept for backwards compatibility)
// ============================================================================

#[derive(Queryable, Selectable, Insertable, Serialize, Deserialize, Debug, Clone)]
#[diesel(table_name = crate::schema::schedules)]
pub struct Schedule {
    pub id: i32,
    pub device_id: i32,
    pub user_id: i32,
    pub duration_minutes: i32,
    pub window_start: NaiveTime,
    pub window_end: NaiveTime,
    pub created_at: NaiveDateTime,
}

// ============================================================================
// Scheduled Execution Models
// ============================================================================

/// Execution status for scheduled actions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    Pending,
    /// Claimed by a runner that is sending the command right now
    Executing,
    Executed,
    Failed,
    Retrying,
    Missed,
    /// Not needed: the appliance had already finished (its power dropped)
    Done,
}

impl ExecutionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ExecutionStatus::Pending => "pending",
            ExecutionStatus::Executing => "executing",
            ExecutionStatus::Executed => "executed",
            ExecutionStatus::Failed => "failed",
            ExecutionStatus::Retrying => "retrying",
            ExecutionStatus::Missed => "missed",
            ExecutionStatus::Done => "done",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(ExecutionStatus::Pending),
            "executing" => Some(ExecutionStatus::Executing),
            "executed" => Some(ExecutionStatus::Executed),
            "failed" => Some(ExecutionStatus::Failed),
            "retrying" => Some(ExecutionStatus::Retrying),
            "missed" => Some(ExecutionStatus::Missed),
            "done" => Some(ExecutionStatus::Done),
            _ => None,
        }
    }
}

#[derive(Queryable, Selectable, Serialize, Deserialize, Debug, Clone)]
#[diesel(table_name = crate::schema::scheduled_executions)]
pub struct ScheduledExecution {
    pub id: i32,
    pub rule_id: i32,
    pub scheduled_hour: NaiveDateTime,
    pub expected_action: String,
    pub status: String,
    pub executed_at: Option<NaiveDateTime>,
    pub execution_id: Option<i32>,
    pub retry_count: i32,
    pub last_retry_at: Option<NaiveDateTime>,
    pub next_retry_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
}

impl ScheduledExecution {
    pub fn get_status(&self) -> Option<ExecutionStatus> {
        ExecutionStatus::parse(&self.status)
    }
}

#[derive(Insertable, Debug)]
#[diesel(table_name = crate::schema::scheduled_executions)]
pub struct NewScheduledExecution {
    pub rule_id: i32,
    pub scheduled_hour: NaiveDateTime,
    pub expected_action: String,
    pub status: String,
}

#[derive(AsChangeset, Debug)]
#[diesel(table_name = crate::schema::scheduled_executions)]
pub struct UpdateScheduledExecution {
    pub status: Option<String>,
    pub executed_at: Option<NaiveDateTime>,
    pub execution_id: Option<i32>,
    pub retry_count: Option<i32>,
    pub last_retry_at: Option<NaiveDateTime>,
    pub next_retry_at: Option<Option<NaiveDateTime>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rule_type_conversion() {
        assert_eq!(RuleType::PriceThreshold.as_str(), "price_threshold");
        assert_eq!(
            RuleType::parse("cheapest_hours"),
            Some(RuleType::CheapestHours)
        );
        assert_eq!(RuleType::parse("invalid"), None);
    }

    #[test]
    fn test_rule_action_conversion() {
        assert_eq!(RuleAction::TurnOn.as_str(), "turn_on");
        assert_eq!(RuleAction::parse("turn_off"), Some(RuleAction::TurnOff));
        assert_eq!(RuleAction::parse("invalid"), None);
    }
}
