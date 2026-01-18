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
}

#[derive(Queryable, Selectable, Insertable, Serialize, Deserialize, Debug, Clone)]
#[diesel(table_name = crate::schema::prices)]
pub struct Price {
    pub timestamp: NaiveDateTime,
    pub price: f64,
    pub source: String,
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
    pub credentials_json: String,
    pub is_active: bool,
    pub created_at: NaiveDateTime,
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

    pub fn from_str(s: &str) -> Option<Self> {
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

    pub fn from_str(s: &str) -> Option<Self> {
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
        RuleType::from_str(&self.rule_type)
    }

    pub fn get_action(&self) -> Option<RuleAction> {
        RuleAction::from_str(&self.action)
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
// Configuration Structs for Rules
// ============================================================================

/// Configuration for price threshold rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceThresholdConfig {
    /// Price threshold in €/kWh
    pub threshold: f64,
    /// "below" or "above"
    pub comparison: String,
}

/// Configuration for cheapest hours rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheapestHoursConfig {
    /// Number of hours needed
    pub hours_needed: i32,
    /// Start of the time window (e.g., "00:00")
    pub window_start: String,
    /// End of the time window (e.g., "08:00")
    pub window_end: String,
    /// Whether hours must be contiguous
    #[serde(default)]
    pub contiguous: bool,
}

/// Configuration for time schedule rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeScheduleConfig {
    /// Days of week: ["mon", "tue", "wed", "thu", "fri", "sat", "sun"]
    pub days: Vec<String>,
    /// Time to trigger (e.g., "06:00")
    pub time: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rule_type_conversion() {
        assert_eq!(RuleType::PriceThreshold.as_str(), "price_threshold");
        assert_eq!(
            RuleType::from_str("cheapest_hours"),
            Some(RuleType::CheapestHours)
        );
        assert_eq!(RuleType::from_str("invalid"), None);
    }

    #[test]
    fn test_rule_action_conversion() {
        assert_eq!(RuleAction::TurnOn.as_str(), "turn_on");
        assert_eq!(RuleAction::from_str("turn_off"), Some(RuleAction::TurnOff));
        assert_eq!(RuleAction::from_str("invalid"), None);
    }

    #[test]
    fn test_price_threshold_config_serialization() {
        let config = PriceThresholdConfig {
            threshold: 0.10,
            comparison: "below".to_string(),
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("0.1"));
        assert!(json.contains("below"));
    }

    #[test]
    fn test_cheapest_hours_config_serialization() {
        let config = CheapestHoursConfig {
            hours_needed: 3,
            window_start: "00:00".to_string(),
            window_end: "08:00".to_string(),
            contiguous: false,
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: CheapestHoursConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.hours_needed, 3);
    }

    #[test]
    fn test_time_schedule_config_serialization() {
        let config = TimeScheduleConfig {
            days: vec!["mon".to_string(), "wed".to_string(), "fri".to_string()],
            time: "06:30".to_string(),
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("mon"));
        assert!(json.contains("06:30"));
    }
}
