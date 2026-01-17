use crate::{
    db::DbPool,
    integrations::{ProviderRegistry, DeviceState},
    models::{
        AutomationRule, CheapestHoursConfig, NewRuleExecution, Price, PriceThresholdConfig,
        RuleAction, TimeScheduleConfig,
    },
    schema::{automation_rules, devices, prices, rule_executions, user_integrations},
};
use chrono::{Local, NaiveDateTime, NaiveTime, Timelike, Weekday, Datelike};
use diesel::prelude::*;
use log::{error, info, warn};
use serde_json::Value as JsonValue;
use std::sync::Arc;

/// Result of evaluating a rule
#[derive(Debug, Clone)]
pub struct RuleEvaluation {
    pub rule_id: i32,
    pub should_trigger: bool,
    pub action: RuleAction,
    pub reason: String,
}

/// Result of executing a rule action
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub rule_id: i32,
    pub success: bool,
    pub error_message: Option<String>,
    pub price_at_execution: Option<f64>,
    pub device_state_before: Option<JsonValue>,
    pub device_state_after: Option<JsonValue>,
}

/// The automation engine that evaluates and executes rules
pub struct AutomationEngine {
    pool: DbPool,
    provider_registry: Arc<ProviderRegistry>,
}

impl AutomationEngine {
    pub fn new(pool: DbPool, provider_registry: Arc<ProviderRegistry>) -> Self {
        Self {
            pool,
            provider_registry,
        }
    }

    /// Run the automation engine - evaluate all enabled rules and execute actions
    pub async fn run(&self) -> Vec<ExecutionResult> {
        let mut results = Vec::new();
        let now = Local::now().naive_local();

        // Get current price
        let current_price = self.get_current_price(&now);

        // Get all enabled rules
        let rules = match self.get_enabled_rules() {
            Ok(r) => r,
            Err(e) => {
                error!("Failed to get enabled rules: {}", e);
                return results;
            }
        };

        info!("Evaluating {} enabled rules", rules.len());

        for rule in rules {
            // Evaluate the rule
            let evaluation = self.evaluate_rule(&rule, &now, current_price);

            if evaluation.should_trigger {
                info!(
                    "Rule '{}' (id={}) triggered: {}",
                    rule.name, rule.id, evaluation.reason
                );

                // Execute the action
                let result = self.execute_rule(&rule, &evaluation, current_price).await;

                // Log the execution
                self.log_execution(&result, &evaluation);

                results.push(result);
            }
        }

        results
    }

    /// Get all enabled automation rules with their device and integration info
    fn get_enabled_rules(&self) -> Result<Vec<AutomationRule>, String> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| format!("Database connection error: {}", e))?;

        automation_rules::table
            .filter(automation_rules::is_enabled.eq(true))
            .order(automation_rules::priority.asc())
            .load::<AutomationRule>(&mut conn)
            .map_err(|e| format!("Failed to load rules: {}", e))
    }

    /// Get the current electricity price
    fn get_current_price(&self, now: &NaiveDateTime) -> Option<f64> {
        let mut conn = match self.pool.get() {
            Ok(c) => c,
            Err(_) => return None,
        };

        // Get price for current hour
        let hour_start = now
            .date()
            .and_hms_opt(now.hour(), 0, 0)
            .unwrap_or(*now);

        prices::table
            .filter(prices::timestamp.eq(hour_start))
            .select(prices::price)
            .first::<f64>(&mut conn)
            .ok()
    }

    /// Evaluate a rule to determine if it should trigger
    fn evaluate_rule(
        &self,
        rule: &AutomationRule,
        now: &NaiveDateTime,
        current_price: Option<f64>,
    ) -> RuleEvaluation {
        let action = RuleAction::from_str(&rule.action).unwrap_or(RuleAction::TurnOn);

        match rule.rule_type.as_str() {
            "price_threshold" => self.evaluate_price_threshold(rule, current_price, action),
            "cheapest_hours" => self.evaluate_cheapest_hours(rule, now, action),
            "time_schedule" => self.evaluate_time_schedule(rule, now, action),
            "manual" => RuleEvaluation {
                rule_id: rule.id,
                should_trigger: false,
                action,
                reason: "Manual rules don't auto-trigger".to_string(),
            },
            _ => RuleEvaluation {
                rule_id: rule.id,
                should_trigger: false,
                action,
                reason: format!("Unknown rule type: {}", rule.rule_type),
            },
        }
    }

    /// Evaluate a price threshold rule
    fn evaluate_price_threshold(
        &self,
        rule: &AutomationRule,
        current_price: Option<f64>,
        action: RuleAction,
    ) -> RuleEvaluation {
        let config: PriceThresholdConfig = match serde_json::from_value(rule.config.clone()) {
            Ok(c) => c,
            Err(_) => {
                return RuleEvaluation {
                    rule_id: rule.id,
                    should_trigger: false,
                    action,
                    reason: "Invalid config".to_string(),
                }
            }
        };

        let price = match current_price {
            Some(p) => p,
            None => {
                return RuleEvaluation {
                    rule_id: rule.id,
                    should_trigger: false,
                    action,
                    reason: "No current price available".to_string(),
                }
            }
        };

        let should_trigger = match config.comparison.as_str() {
            "below" => price < config.threshold,
            "above" => price > config.threshold,
            _ => false,
        };

        RuleEvaluation {
            rule_id: rule.id,
            should_trigger,
            action,
            reason: format!(
                "Price {:.4} €/kWh is {} threshold {:.4}",
                price, config.comparison, config.threshold
            ),
        }
    }

    /// Evaluate a cheapest hours rule
    fn evaluate_cheapest_hours(
        &self,
        rule: &AutomationRule,
        now: &NaiveDateTime,
        action: RuleAction,
    ) -> RuleEvaluation {
        let config: CheapestHoursConfig = match serde_json::from_value(rule.config.clone()) {
            Ok(c) => c,
            Err(_) => {
                return RuleEvaluation {
                    rule_id: rule.id,
                    should_trigger: false,
                    action,
                    reason: "Invalid config".to_string(),
                }
            }
        };

        // Parse window times
        let window_start = match NaiveTime::parse_from_str(&config.window_start, "%H:%M") {
            Ok(t) => t,
            Err(_) => {
                return RuleEvaluation {
                    rule_id: rule.id,
                    should_trigger: false,
                    action,
                    reason: "Invalid window_start format".to_string(),
                }
            }
        };

        let window_end = match NaiveTime::parse_from_str(&config.window_end, "%H:%M") {
            Ok(t) => t,
            Err(_) => {
                return RuleEvaluation {
                    rule_id: rule.id,
                    should_trigger: false,
                    action,
                    reason: "Invalid window_end format".to_string(),
                }
            }
        };

        // Check if current time is within the window
        let current_time = now.time();
        let in_window = if window_start <= window_end {
            current_time >= window_start && current_time < window_end
        } else {
            // Window crosses midnight
            current_time >= window_start || current_time < window_end
        };

        if !in_window {
            return RuleEvaluation {
                rule_id: rule.id,
                should_trigger: false,
                action,
                reason: format!("Current time {} is outside window", current_time),
            };
        }

        // Get prices within the window and find cheapest hours
        let cheapest_hours = self.find_cheapest_hours_in_window(
            now,
            window_start,
            window_end,
            config.hours_needed,
            config.contiguous,
        );

        let current_hour = now.hour();
        let is_cheap_hour = cheapest_hours.contains(&current_hour);

        RuleEvaluation {
            rule_id: rule.id,
            should_trigger: is_cheap_hour,
            action,
            reason: if is_cheap_hour {
                format!("Hour {} is one of the {} cheapest hours", current_hour, config.hours_needed)
            } else {
                format!("Hour {} is not among the {} cheapest hours", current_hour, config.hours_needed)
            },
        }
    }

    /// Find the cheapest hours within a time window
    fn find_cheapest_hours_in_window(
        &self,
        now: &NaiveDateTime,
        window_start: NaiveTime,
        window_end: NaiveTime,
        hours_needed: i32,
        contiguous: bool,
    ) -> Vec<u32> {
        let mut conn = match self.pool.get() {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        // Build the time range for the window
        let today = now.date();
        let start_dt = today.and_time(window_start);
        let end_dt = if window_end > window_start {
            today.and_time(window_end)
        } else {
            // Window crosses midnight - end is tomorrow
            (today + chrono::Duration::days(1)).and_time(window_end)
        };

        // Get prices in the window
        let prices: Vec<Price> = prices::table
            .filter(prices::timestamp.ge(start_dt))
            .filter(prices::timestamp.lt(end_dt))
            .order(prices::price.asc())
            .load(&mut conn)
            .unwrap_or_default();

        if contiguous {
            // Find contiguous block of cheapest hours
            self.find_contiguous_cheapest(&prices, hours_needed as usize)
        } else {
            // Just take the N cheapest individual hours
            prices
                .into_iter()
                .take(hours_needed as usize)
                .map(|p| p.timestamp.hour())
                .collect()
        }
    }

    /// Find a contiguous block of hours with lowest total price
    fn find_contiguous_cheapest(&self, prices: &[Price], hours_needed: usize) -> Vec<u32> {
        if prices.len() < hours_needed {
            return Vec::new();
        }

        // Sort by timestamp first
        let mut sorted_prices: Vec<_> = prices.to_vec();
        sorted_prices.sort_by_key(|p| p.timestamp);

        let mut best_start = 0;
        let mut best_sum = f64::MAX;

        for i in 0..=(sorted_prices.len() - hours_needed) {
            let sum: f64 = sorted_prices[i..i + hours_needed]
                .iter()
                .map(|p| p.price)
                .sum();
            if sum < best_sum {
                best_sum = sum;
                best_start = i;
            }
        }

        sorted_prices[best_start..best_start + hours_needed]
            .iter()
            .map(|p| p.timestamp.hour())
            .collect()
    }

    /// Evaluate a time schedule rule
    fn evaluate_time_schedule(
        &self,
        rule: &AutomationRule,
        now: &NaiveDateTime,
        action: RuleAction,
    ) -> RuleEvaluation {
        let config: TimeScheduleConfig = match serde_json::from_value(rule.config.clone()) {
            Ok(c) => c,
            Err(_) => {
                return RuleEvaluation {
                    rule_id: rule.id,
                    should_trigger: false,
                    action,
                    reason: "Invalid config".to_string(),
                }
            }
        };

        // Check if today is in the scheduled days
        let today = now.weekday();
        let day_abbrev = match today {
            Weekday::Mon => "mon",
            Weekday::Tue => "tue",
            Weekday::Wed => "wed",
            Weekday::Thu => "thu",
            Weekday::Fri => "fri",
            Weekday::Sat => "sat",
            Weekday::Sun => "sun",
        };

        if !config.days.iter().any(|d| d.to_lowercase() == day_abbrev) {
            return RuleEvaluation {
                rule_id: rule.id,
                should_trigger: false,
                action,
                reason: format!("{} is not in scheduled days", day_abbrev),
            };
        }

        // Parse scheduled time
        let scheduled_time = match NaiveTime::parse_from_str(&config.time, "%H:%M") {
            Ok(t) => t,
            Err(_) => {
                return RuleEvaluation {
                    rule_id: rule.id,
                    should_trigger: false,
                    action,
                    reason: "Invalid time format".to_string(),
                }
            }
        };

        // Check if current time matches (within the same hour)
        let current_time = now.time();
        let should_trigger = current_time.hour() == scheduled_time.hour()
            && current_time.minute() >= scheduled_time.minute()
            && current_time.minute() < scheduled_time.minute() + 5; // 5-minute window

        RuleEvaluation {
            rule_id: rule.id,
            should_trigger,
            action,
            reason: if should_trigger {
                format!("Scheduled time {} matches", config.time)
            } else {
                format!("Current time {} doesn't match scheduled {}", current_time, config.time)
            },
        }
    }

    /// Execute a rule action on the device
    async fn execute_rule(
        &self,
        rule: &AutomationRule,
        evaluation: &RuleEvaluation,
        current_price: Option<f64>,
    ) -> ExecutionResult {
        let mut conn = match self.pool.get() {
            Ok(c) => c,
            Err(e) => {
                return ExecutionResult {
                    rule_id: rule.id,
                    success: false,
                    error_message: Some(format!("Database connection error: {}", e)),
                    price_at_execution: current_price,
                    device_state_before: None,
                    device_state_after: None,
                }
            }
        };

        // Get device and integration info
        let device_info: Option<(String, String, String)> = devices::table
            .inner_join(user_integrations::table)
            .filter(devices::id.eq(rule.device_id))
            .select((
                devices::external_id,
                user_integrations::provider_name,
                user_integrations::credentials_json,
            ))
            .first(&mut conn)
            .optional()
            .unwrap_or(None);

        let (external_id, provider_name, credentials_json) = match device_info {
            Some(info) => info,
            None => {
                return ExecutionResult {
                    rule_id: rule.id,
                    success: false,
                    error_message: Some("Device or integration not found".to_string()),
                    price_at_execution: current_price,
                    device_state_before: None,
                    device_state_after: None,
                }
            }
        };

        // Get the provider
        let provider = match self.provider_registry.get(&provider_name) {
            Some(p) => p,
            None => {
                return ExecutionResult {
                    rule_id: rule.id,
                    success: false,
                    error_message: Some(format!("Provider '{}' not found", provider_name)),
                    price_at_execution: current_price,
                    device_state_before: None,
                    device_state_after: None,
                }
            }
        };

        // Parse credentials
        let credentials: JsonValue = match serde_json::from_str(&credentials_json) {
            Ok(c) => c,
            Err(e) => {
                return ExecutionResult {
                    rule_id: rule.id,
                    success: false,
                    error_message: Some(format!("Invalid credentials: {}", e)),
                    price_at_execution: current_price,
                    device_state_before: None,
                    device_state_after: None,
                }
            }
        };

        // Get device state before action
        let state_before = provider
            .get_device_state(&credentials, &external_id)
            .await
            .ok();

        // Execute the action
        let action_result = match evaluation.action {
            RuleAction::TurnOn => provider.turn_on(&credentials, &external_id).await,
            RuleAction::TurnOff => provider.turn_off(&credentials, &external_id).await,
            RuleAction::Toggle => {
                // Toggle based on current state
                if state_before.as_ref().map(|s| s.is_on).unwrap_or(false) {
                    provider.turn_off(&credentials, &external_id).await
                } else {
                    provider.turn_on(&credentials, &external_id).await
                }
            }
        };

        match action_result {
            Ok(result) => {
                // Update last_triggered_at
                let now = chrono::Utc::now().naive_utc();
                diesel::update(automation_rules::table.filter(automation_rules::id.eq(rule.id)))
                    .set(automation_rules::last_triggered_at.eq(Some(now)))
                    .execute(&mut conn)
                    .ok();

                ExecutionResult {
                    rule_id: rule.id,
                    success: result.success,
                    error_message: result.message,
                    price_at_execution: current_price,
                    device_state_before: state_before.map(|s| serde_json::to_value(s).ok()).flatten(),
                    device_state_after: result.new_state.map(|s| serde_json::to_value(s).ok()).flatten(),
                }
            }
            Err(e) => ExecutionResult {
                rule_id: rule.id,
                success: false,
                error_message: Some(e.to_string()),
                price_at_execution: current_price,
                device_state_before: state_before.map(|s| serde_json::to_value(s).ok()).flatten(),
                device_state_after: None,
            },
        }
    }

    /// Log an execution to the database
    fn log_execution(&self, result: &ExecutionResult, evaluation: &RuleEvaluation) {
        let mut conn = match self.pool.get() {
            Ok(c) => c,
            Err(e) => {
                error!("Failed to get connection for logging: {}", e);
                return;
            }
        };

        let new_execution = NewRuleExecution {
            rule_id: result.rule_id,
            action_taken: evaluation.action.as_str().to_string(),
            success: result.success,
            error_message: result.error_message.clone(),
            price_at_execution: result.price_at_execution,
            device_state_before: result.device_state_before.clone(),
            device_state_after: result.device_state_after.clone(),
        };

        if let Err(e) = diesel::insert_into(rule_executions::table)
            .values(&new_execution)
            .execute(&mut conn)
        {
            error!("Failed to log execution: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_price_threshold_config_parsing() {
        let config = json!({
            "threshold": 0.10,
            "comparison": "below"
        });
        let parsed: PriceThresholdConfig = serde_json::from_value(config).unwrap();
        assert_eq!(parsed.threshold, 0.10);
        assert_eq!(parsed.comparison, "below");
    }

    #[test]
    fn test_cheapest_hours_config_parsing() {
        let config = json!({
            "hours_needed": 3,
            "window_start": "00:00",
            "window_end": "08:00",
            "contiguous": false
        });
        let parsed: CheapestHoursConfig = serde_json::from_value(config).unwrap();
        assert_eq!(parsed.hours_needed, 3);
        assert_eq!(parsed.window_start, "00:00");
        assert_eq!(parsed.window_end, "08:00");
        assert!(!parsed.contiguous);
    }

    #[test]
    fn test_time_schedule_config_parsing() {
        let config = json!({
            "days": ["mon", "wed", "fri"],
            "time": "06:30"
        });
        let parsed: TimeScheduleConfig = serde_json::from_value(config).unwrap();
        assert_eq!(parsed.days.len(), 3);
        assert!(parsed.days.contains(&"mon".to_string()));
        assert_eq!(parsed.time, "06:30");
    }

    #[test]
    fn test_rule_evaluation_struct() {
        let eval = RuleEvaluation {
            rule_id: 1,
            should_trigger: true,
            action: RuleAction::TurnOn,
            reason: "Test reason".to_string(),
        };
        assert!(eval.should_trigger);
        assert_eq!(eval.action, RuleAction::TurnOn);
    }

    #[test]
    fn test_execution_result_struct() {
        let result = ExecutionResult {
            rule_id: 1,
            success: true,
            error_message: None,
            price_at_execution: Some(0.15),
            device_state_before: None,
            device_state_after: Some(json!({"is_on": true})),
        };
        assert!(result.success);
        assert_eq!(result.price_at_execution, Some(0.15));
    }
}
