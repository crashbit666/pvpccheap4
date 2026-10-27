use crate::{
    db::DbPool,
    integrations::{DeviceState, ProviderError, ProviderRegistry},
    models::{
        AutomationRule, ExecutionStatus, NewRuleExecution, RuleAction, ScheduledExecution,
        UpdateScheduledExecution,
    },
    schema::{automation_rules, devices, rule_executions, scheduled_executions, user_integrations},
    services::tariff::Tariff,
};
use chrono::{NaiveDateTime, Timelike};
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

    /// What the user pays in the hour of `now`
    fn get_current_price(&self, now: &NaiveDateTime, user_id: i32) -> Option<f64> {
        let mut conn = self.pool.get().ok()?;
        Tariff::of_user(&mut conn, user_id)
            .at(&mut conn, *now)
            .ok()
            .flatten()
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
                };
            }
        };

        // Get device and integration info
        let device_info: Option<(i32, String, String, String)> = devices::table
            .inner_join(user_integrations::table)
            .filter(devices::id.eq(rule.device_id))
            .select((
                user_integrations::id,
                devices::external_id,
                user_integrations::provider_name,
                user_integrations::credentials_json,
            ))
            .first(&mut conn)
            .optional()
            .unwrap_or(None);

        let (integration_id, external_id, provider_name, credentials_json) = match device_info {
            Some(info) => info,
            None => {
                return ExecutionResult {
                    rule_id: rule.id,
                    success: false,
                    error_message: Some("Device or integration not found".to_string()),
                    price_at_execution: current_price,
                    device_state_before: None,
                    device_state_after: None,
                };
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
                };
            }
        };

        // Parse credentials
        let mut credentials: JsonValue =
            match crate::services::credentials::decrypt(&credentials_json) {
                Ok(c) => c,
                Err(e) => {
                    return ExecutionResult {
                        rule_id: rule.id,
                        success: false,
                        error_message: Some(format!("Invalid credentials: {}", e)),
                        price_at_execution: current_price,
                        device_state_before: None,
                        device_state_after: None,
                    };
                }
            };

        // Get device state before action
        let state_before = provider
            .get_device_state(&credentials, &external_id)
            .await
            .ok();

        // Execute the action (with automatic token refresh on auth failure)
        let action_result = self
            .execute_action_with_retry(
                &provider,
                &mut credentials,
                &external_id,
                &evaluation.action,
                &state_before,
                integration_id,
            )
            .await;

        match action_result {
            Ok(result) => {
                // Update last_triggered_at
                let now = crate::time::now_naive();
                diesel::update(automation_rules::table.filter(automation_rules::id.eq(rule.id)))
                    .set(automation_rules::last_triggered_at.eq(Some(now)))
                    .execute(&mut conn)
                    .ok();

                // Keep the cached device state in step with what we just did
                let is_on =
                    result
                        .new_state
                        .as_ref()
                        .map(|s| s.is_on)
                        .or(match evaluation.action {
                            RuleAction::TurnOn => Some(true),
                            RuleAction::TurnOff => Some(false),
                            RuleAction::Toggle => None,
                        });
                if let (true, Some(is_on)) = (result.success, is_on) {
                    diesel::update(devices::table.filter(devices::id.eq(rule.device_id)))
                        .set(devices::is_on.eq(is_on))
                        .execute(&mut conn)
                        .ok();
                }

                ExecutionResult {
                    rule_id: rule.id,
                    success: result.success,
                    error_message: result.message,
                    price_at_execution: current_price,
                    device_state_before: state_before.and_then(|s| serde_json::to_value(s).ok()),
                    device_state_after: result.new_state.and_then(|s| serde_json::to_value(s).ok()),
                }
            }
            Err(e) => ExecutionResult {
                rule_id: rule.id,
                success: false,
                error_message: Some(e.to_string()),
                price_at_execution: current_price,
                device_state_before: state_before.and_then(|s| serde_json::to_value(s).ok()),
                device_state_after: None,
            },
        }
    }

    /// Execute an action with automatic token refresh on authentication failure
    async fn execute_action_with_retry(
        &self,
        provider: &std::sync::Arc<dyn crate::integrations::SmartHomeProvider>,
        credentials: &mut JsonValue,
        external_id: &str,
        action: &RuleAction,
        state_before: &Option<DeviceState>,
        integration_id: i32,
    ) -> Result<crate::integrations::DeviceActionResult, ProviderError> {
        // First attempt
        let first_result = self
            .execute_single_action(provider, credentials, external_id, action, state_before)
            .await;

        match &first_result {
            Err(ProviderError::AuthenticationFailed(_))
            | Err(ProviderError::InvalidCredentials) => {
                // Token might be expired - try to refresh
                warn!(
                    "Authentication failed, attempting to refresh credentials for integration {}",
                    integration_id
                );

                match provider.refresh_credentials(credentials).await {
                    Ok(new_credentials) => {
                        info!("Credentials refreshed successfully, updating database");

                        // Update credentials in database
                        if let Err(e) =
                            self.update_integration_credentials(integration_id, &new_credentials)
                        {
                            error!("Failed to update credentials in database: {}", e);
                        }

                        // Update local credentials and retry
                        *credentials = new_credentials;
                        self.execute_single_action(
                            provider,
                            credentials,
                            external_id,
                            action,
                            state_before,
                        )
                        .await
                    }
                    Err(e) => {
                        error!("Failed to refresh credentials: {}", e);
                        Err(ProviderError::AuthenticationFailed(format!(
                            "Token expired and refresh failed: {}",
                            e
                        )))
                    }
                }
            }
            _ => first_result,
        }
    }

    /// Execute a single action without retry
    async fn execute_single_action(
        &self,
        provider: &std::sync::Arc<dyn crate::integrations::SmartHomeProvider>,
        credentials: &JsonValue,
        external_id: &str,
        action: &RuleAction,
        state_before: &Option<DeviceState>,
    ) -> Result<crate::integrations::DeviceActionResult, ProviderError> {
        match action {
            RuleAction::TurnOn => provider.turn_on(credentials, external_id).await,
            RuleAction::TurnOff => provider.turn_off(credentials, external_id).await,
            RuleAction::Toggle => {
                if state_before.as_ref().map(|s| s.is_on).unwrap_or(false) {
                    provider.turn_off(credentials, external_id).await
                } else {
                    provider.turn_on(credentials, external_id).await
                }
            }
        }
    }

    /// Update integration credentials in the database
    fn update_integration_credentials(
        &self,
        integration_id: i32,
        new_credentials: &JsonValue,
    ) -> Result<(), String> {
        let mut conn = self.pool.get().map_err(|e| e.to_string())?;

        let credentials_str = crate::services::credentials::encrypt(new_credentials);

        diesel::update(user_integrations::table.filter(user_integrations::id.eq(integration_id)))
            .set(user_integrations::credentials_json.eq(credentials_str))
            .execute(&mut conn)
            .map_err(|e| format!("Failed to update credentials: {}", e))?;

        info!("Updated credentials for integration {}", integration_id);
        Ok(())
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

    // =========================================================================
    // Scheduled Execution Methods
    // =========================================================================

    /// Execute all scheduled actions for the current hour
    /// Also turn off devices that are NOT scheduled for the current hour (inverse action)
    pub async fn execute_current_hour(&self) -> Vec<ExecutionResult> {
        let mut results = Vec::new();
        let now = crate::time::now_naive();
        let current_hour_start = now.date().and_hms_opt(now.hour(), 0, 0).unwrap();

        let mut conn = match self.pool.get() {
            Ok(c) => c,
            Err(e) => {
                error!("Failed to get connection: {}", e);
                return results;
            }
        };

        // Get pending scheduled executions for current hour
        let pending_executions: Vec<(ScheduledExecution, AutomationRule)> =
            scheduled_executions::table
                .inner_join(automation_rules::table)
                .filter(scheduled_executions::scheduled_hour.eq(current_hour_start))
                .filter(scheduled_executions::status.eq(ExecutionStatus::Pending.as_str()))
                .filter(automation_rules::is_enabled.eq(true))
                .select((ScheduledExecution::as_select(), AutomationRule::as_select()))
                .load(&mut conn)
                .unwrap_or_default();

        info!(
            "Found {} pending scheduled executions for hour {}",
            pending_executions.len(),
            current_hour_start
        );

        // Rules and devices scheduled for this hour, whatever the status: an execution
        // already done (e.g. before a cron restart) or being retried must not be undone
        // by the inverse turn-off below.
        let (scheduled_rule_ids, scheduled_device_ids): (Vec<i32>, Vec<i32>) =
            scheduled_executions::table
                .inner_join(automation_rules::table)
                .filter(scheduled_executions::scheduled_hour.eq(current_hour_start))
                .select((automation_rules::id, automation_rules::device_id))
                .load::<(i32, i32)>(&mut conn)
                .unwrap_or_default()
                .into_iter()
                .unzip();

        // Execute scheduled actions (turn on)
        for (scheduled, rule) in pending_executions {
            if let Some(result) = self.claim_and_execute(&scheduled, &rule).await {
                results.push(result);
            }
        }

        // Find rules that should turn OFF (not scheduled for current hour but have "turn_on" action).
        // Skip devices that another rule keeps on this hour.
        let rules_to_turn_off: Vec<AutomationRule> = automation_rules::table
            .filter(automation_rules::is_enabled.eq(true))
            .filter(automation_rules::action.eq("turn_on"))
            .filter(automation_rules::id.ne_all(&scheduled_rule_ids))
            .filter(automation_rules::device_id.ne_all(&scheduled_device_ids))
            .load(&mut conn)
            .unwrap_or_default();

        // For each rule not scheduled this hour, turn off the device (once per device)
        let mut turned_off_devices: Vec<i32> = Vec::new();
        for rule in rules_to_turn_off {
            if turned_off_devices.contains(&rule.device_id) {
                continue;
            }
            // Check if this rule has any scheduled execution for today (to know if it's an automated rule)
            let today_start = now.date().and_hms_opt(0, 0, 0).unwrap();
            let today_end = now.date().and_hms_opt(23, 59, 59).unwrap();

            let has_schedule_today: bool = scheduled_executions::table
                .filter(scheduled_executions::rule_id.eq(rule.id))
                .filter(scheduled_executions::scheduled_hour.ge(today_start))
                .filter(scheduled_executions::scheduled_hour.le(today_end))
                .select(scheduled_executions::id)
                .first::<i32>(&mut conn)
                .is_ok();

            // Only turn off if this rule has scheduled executions (it's an automated rule)
            if has_schedule_today {
                info!(
                    "Rule {} not scheduled for hour {}, turning off device {}",
                    rule.id, current_hour_start, rule.device_id
                );

                let current_price = self.get_current_price(&now, rule.user_id);
                let evaluation = RuleEvaluation {
                    rule_id: rule.id,
                    should_trigger: true,
                    action: RuleAction::TurnOff,
                    reason: format!("Not scheduled for hour {} - auto turn off", now.hour()),
                };

                turned_off_devices.push(rule.device_id);
                let result = self.execute_rule(&rule, &evaluation, current_price).await;
                self.log_execution(&result, &evaluation);
                results.push(result);
            }
        }

        results
    }

    /// Execute a specific scheduled execution
    async fn execute_scheduled_execution(
        &self,
        scheduled: &ScheduledExecution,
        rule: &AutomationRule,
    ) -> ExecutionResult {
        let current_price = self.get_current_price(&crate::time::now_naive(), rule.user_id);
        let action = RuleAction::parse(&scheduled.expected_action).unwrap_or(RuleAction::TurnOn);

        let evaluation = RuleEvaluation {
            rule_id: rule.id,
            should_trigger: true,
            action: action.clone(),
            reason: format!("Scheduled execution for hour {}", scheduled.scheduled_hour),
        };

        // Execute the action
        let result = self.execute_rule(rule, &evaluation, current_price).await;

        // Log the execution to rule_executions
        self.log_execution(&result, &evaluation);

        // Update the scheduled_execution status
        self.update_scheduled_status(scheduled.id, &result).await;

        result
    }

    /// Update the status of a scheduled execution based on result
    async fn update_scheduled_status(&self, scheduled_id: i32, result: &ExecutionResult) {
        let mut conn = match self.pool.get() {
            Ok(c) => c,
            Err(e) => {
                error!("Failed to get connection for status update: {}", e);
                return;
            }
        };

        let now = crate::time::now_naive();

        if result.success {
            // Mark as executed
            let update = UpdateScheduledExecution {
                status: Some(ExecutionStatus::Executed.as_str().to_string()),
                executed_at: Some(now),
                execution_id: None, // TODO: get the execution_id from log
                retry_count: None,
                last_retry_at: None,
                next_retry_at: Some(None),
            };

            diesel::update(
                scheduled_executions::table.filter(scheduled_executions::id.eq(scheduled_id)),
            )
            .set(&update)
            .execute(&mut conn)
            .ok();
        } else {
            // Get current retry count
            let current: Option<ScheduledExecution> = scheduled_executions::table
                .filter(scheduled_executions::id.eq(scheduled_id))
                .first(&mut conn)
                .optional()
                .unwrap_or(None);

            if let Some(sched) = current {
                let new_retry_count = sched.retry_count + 1;

                if new_retry_count >= 5 {
                    // Max retries reached - mark as failed
                    let update = UpdateScheduledExecution {
                        status: Some(ExecutionStatus::Failed.as_str().to_string()),
                        executed_at: None,
                        execution_id: None,
                        retry_count: Some(new_retry_count),
                        last_retry_at: Some(now),
                        next_retry_at: Some(None),
                    };

                    diesel::update(
                        scheduled_executions::table
                            .filter(scheduled_executions::id.eq(scheduled_id)),
                    )
                    .set(&update)
                    .execute(&mut conn)
                    .ok();
                } else {
                    // Schedule retry in 1 minute
                    let next_retry = now + chrono::Duration::minutes(1);
                    let update = UpdateScheduledExecution {
                        status: Some(ExecutionStatus::Retrying.as_str().to_string()),
                        executed_at: None,
                        execution_id: None,
                        retry_count: Some(new_retry_count),
                        last_retry_at: Some(now),
                        next_retry_at: Some(Some(next_retry)),
                    };

                    diesel::update(
                        scheduled_executions::table
                            .filter(scheduled_executions::id.eq(scheduled_id)),
                    )
                    .set(&update)
                    .execute(&mut conn)
                    .ok();
                }
            }
        }
    }

    /// Retry failed scheduled executions that are due
    pub async fn retry_failed_executions(&self) -> Vec<ExecutionResult> {
        let mut results = Vec::new();
        let now = crate::time::now_naive();

        let mut conn = match self.pool.get() {
            Ok(c) => c,
            Err(e) => {
                error!("Failed to get connection: {}", e);
                return results;
            }
        };

        // Executions due now: retries whose time has come, and pending hours of
        // the current hour (e.g. a rule created or enabled in the middle of it)
        let current_hour = crate::time::current_hour();
        let retrying_executions: Vec<(ScheduledExecution, AutomationRule)> =
            scheduled_executions::table
                .inner_join(automation_rules::table)
                .filter(automation_rules::is_enabled.eq(true))
                .filter(
                    scheduled_executions::status
                        .eq(ExecutionStatus::Retrying.as_str())
                        .and(scheduled_executions::next_retry_at.le(now))
                        .or(scheduled_executions::status
                            .eq(ExecutionStatus::Pending.as_str())
                            .and(scheduled_executions::scheduled_hour.eq(current_hour))),
                )
                .select((ScheduledExecution::as_select(), AutomationRule::as_select()))
                .load(&mut conn)
                .unwrap_or_default();

        if !retrying_executions.is_empty() {
            info!("Found {} executions to retry", retrying_executions.len());
        }

        for (scheduled, rule) in retrying_executions {
            // Check if the hour has passed - if so, mark as missed
            let current_hour = now.date().and_hms_opt(now.hour(), 0, 0).unwrap();
            if scheduled.scheduled_hour < current_hour {
                // Hour has passed, mark as missed
                let update = UpdateScheduledExecution {
                    status: Some(ExecutionStatus::Missed.as_str().to_string()),
                    executed_at: None,
                    execution_id: None,
                    retry_count: None,
                    last_retry_at: Some(now),
                    next_retry_at: Some(None),
                };

                diesel::update(
                    scheduled_executions::table.filter(scheduled_executions::id.eq(scheduled.id)),
                )
                .set(&update)
                .execute(&mut conn)
                .ok();

                warn!(
                    "Scheduled execution {} for rule {} marked as missed - hour passed",
                    scheduled.id, rule.id
                );
            } else if let Some(result) = self.claim_and_execute(&scheduled, &rule).await {
                results.push(result);
            }
        }

        results.extend(self.retry_failed_turn_offs().await);
        results
    }

    /// Execute a scheduled hour unless another runner already took it. The row is
    /// moved to "executing" atomically, so the hourly and per-minute jobs (or two
    /// overlapping per-minute passes) never send the same command twice.
    async fn claim_and_execute(
        &self,
        scheduled: &ScheduledExecution,
        rule: &AutomationRule,
    ) -> Option<ExecutionResult> {
        let mut conn = self.pool.get().ok()?;
        let claimed = diesel::update(
            scheduled_executions::table
                .filter(scheduled_executions::id.eq(scheduled.id))
                .filter(scheduled_executions::status.eq_any([
                    ExecutionStatus::Pending.as_str(),
                    ExecutionStatus::Retrying.as_str(),
                ])),
        )
        .set(scheduled_executions::status.eq(ExecutionStatus::Executing.as_str()))
        .execute(&mut conn)
        .unwrap_or(0);

        if claimed == 0 {
            return None;
        }
        Some(self.execute_scheduled_execution(scheduled, rule).await)
    }

    /// Turn a rule's device off if the rule switched it on for the current hour
    /// and no other enabled rule keeps it on. Used when a rule is disabled or
    /// about to be deleted. Returns None when nothing had to be done.
    pub async fn release_device_for_rule(&self, rule_id: i32) -> Option<ExecutionResult> {
        let mut conn = self.pool.get().ok()?;
        let current_hour = crate::time::current_hour();

        let rule: AutomationRule = automation_rules::table
            .find(rule_id)
            .first(&mut conn)
            .ok()?;
        if rule.action != "turn_on" {
            return None;
        }

        let switched_on_now = scheduled_executions::table
            .filter(scheduled_executions::rule_id.eq(rule.id))
            .filter(scheduled_executions::scheduled_hour.eq(current_hour))
            .filter(scheduled_executions::status.eq_any(["executed", "executing", "retrying"]))
            .select(scheduled_executions::id)
            .first::<i32>(&mut conn)
            .is_ok();
        if !switched_on_now {
            return None;
        }

        let kept_on_by_other_rule = scheduled_executions::table
            .inner_join(automation_rules::table)
            .filter(automation_rules::device_id.eq(rule.device_id))
            .filter(automation_rules::id.ne(rule.id))
            .filter(automation_rules::is_enabled.eq(true))
            .filter(scheduled_executions::scheduled_hour.eq(current_hour))
            .filter(scheduled_executions::status.ne(ExecutionStatus::Missed.as_str()))
            .select(scheduled_executions::id)
            .first::<i32>(&mut conn)
            .is_ok();
        if kept_on_by_other_rule {
            return None;
        }

        info!(
            "Rule {} no longer applies, turning off device {}",
            rule.id, rule.device_id
        );
        let evaluation = RuleEvaluation {
            rule_id: rule.id,
            should_trigger: true,
            action: RuleAction::TurnOff,
            reason: "Rule disabled or deleted - turn off".to_string(),
        };
        let current_price = self.get_current_price(&crate::time::now_naive(), rule.user_id);
        let result = self.execute_rule(&rule, &evaluation, current_price).await;
        self.log_execution(&result, &evaluation);
        Some(result)
    }

    /// Retry the hourly inverse turn-off when it failed, up to 5 attempts per hour.
    async fn retry_failed_turn_offs(&self) -> Vec<ExecutionResult> {
        const MAX_ATTEMPTS: usize = 5;
        let mut results = Vec::new();
        let Ok(mut conn) = self.pool.get() else {
            return results;
        };
        let current_hour = crate::time::current_hour();

        // Turn-offs logged this hour, newest first
        let attempts: Vec<(i32, bool)> = rule_executions::table
            .filter(rule_executions::executed_at.ge(current_hour))
            .filter(rule_executions::action_taken.eq(RuleAction::TurnOff.as_str()))
            .order(rule_executions::executed_at.desc())
            .select((rule_executions::rule_id, rule_executions::success))
            .load(&mut conn)
            .unwrap_or_default();

        let mut seen: Vec<i32> = Vec::new();
        for (rule_id, success) in &attempts {
            if seen.contains(rule_id) {
                continue;
            }
            seen.push(*rule_id);
            let tries = attempts.iter().filter(|(id, _)| id == rule_id).count();
            if *success || tries >= MAX_ATTEMPTS {
                continue;
            }

            // Only while the rule is still enabled and not scheduled for this hour
            let Ok(rule) = automation_rules::table
                .find(rule_id)
                .filter(automation_rules::is_enabled.eq(true))
                .first::<AutomationRule>(&mut conn)
            else {
                continue;
            };
            let scheduled_now = scheduled_executions::table
                .inner_join(automation_rules::table)
                .filter(automation_rules::device_id.eq(rule.device_id))
                .filter(scheduled_executions::scheduled_hour.eq(current_hour))
                .filter(scheduled_executions::status.ne(ExecutionStatus::Missed.as_str()))
                .select(scheduled_executions::id)
                .first::<i32>(&mut conn)
                .is_ok();
            if scheduled_now {
                continue;
            }

            warn!(
                "Retrying turn-off of device {} (attempt {})",
                rule.device_id,
                tries + 1
            );
            let evaluation = RuleEvaluation {
                rule_id: rule.id,
                should_trigger: true,
                action: RuleAction::TurnOff,
                reason: format!(
                    "Retry of failed auto turn off ({}/{})",
                    tries + 1,
                    MAX_ATTEMPTS
                ),
            };
            let current_price = self.get_current_price(&crate::time::now_naive(), rule.user_id);
            let result = self.execute_rule(&rule, &evaluation, current_price).await;
            self.log_execution(&result, &evaluation);
            results.push(result);
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
