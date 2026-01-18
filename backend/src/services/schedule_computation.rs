use crate::db::DbPool;
use crate::models::{AutomationRule, ExecutionStatus, NewScheduledExecution, ScheduledExecution};
use crate::schema::{automation_rules, devices, scheduled_executions};
use crate::services::price_fetcher::PriceService;
use chrono::{Local, NaiveDate, NaiveDateTime, Timelike};
use diesel::prelude::*;
use log::{error, info, warn};

/// Service for computing and managing scheduled executions
pub struct ScheduleComputationService {
    pool: DbPool,
}

impl ScheduleComputationService {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Compute schedule for all enabled rules for a given date
    pub fn compute_schedule_for_date(&self, date: NaiveDate) -> Result<usize, String> {
        let mut conn = self.pool.get().map_err(|e| e.to_string())?;

        // Get all enabled rules
        let rules: Vec<AutomationRule> = automation_rules::table
            .filter(automation_rules::is_enabled.eq(true))
            .load(&mut conn)
            .map_err(|e| e.to_string())?;

        let mut total_scheduled = 0;
        for rule in rules {
            match self.compute_schedule_for_rule_internal(&mut conn, &rule, date) {
                Ok(count) => total_scheduled += count,
                Err(e) => {
                    error!("Failed to compute schedule for rule {}: {}", rule.id, e);
                }
            }
        }

        info!(
            "Computed {} scheduled executions for date {}",
            total_scheduled, date
        );
        Ok(total_scheduled)
    }

    /// Compute schedule for a specific rule
    pub fn compute_schedule_for_rule(&self, rule_id: i32, date: NaiveDate) -> Result<usize, String> {
        let mut conn = self.pool.get().map_err(|e| e.to_string())?;

        let rule: AutomationRule = automation_rules::table
            .filter(automation_rules::id.eq(rule_id))
            .first(&mut conn)
            .map_err(|e| e.to_string())?;

        self.compute_schedule_for_rule_internal(&mut conn, &rule, date)
    }

    fn compute_schedule_for_rule_internal(
        &self,
        conn: &mut diesel::PgConnection,
        rule: &AutomationRule,
        date: NaiveDate,
    ) -> Result<usize, String> {
        // Get hours to schedule based on rule type
        let hours_to_schedule = self.calculate_hours_for_rule(rule, date)?;

        let mut count = 0;
        for hour in hours_to_schedule {
            let scheduled_hour = date
                .and_hms_opt(hour as u32, 0, 0)
                .ok_or("Invalid hour")?;

            let new_execution = NewScheduledExecution {
                rule_id: rule.id,
                scheduled_hour,
                expected_action: rule.action.clone(),
                status: ExecutionStatus::Pending.as_str().to_string(),
            };

            // Upsert: insert or ignore if exists
            let result = diesel::insert_into(scheduled_executions::table)
                .values(&new_execution)
                .on_conflict((
                    scheduled_executions::rule_id,
                    scheduled_executions::scheduled_hour,
                ))
                .do_nothing()
                .execute(conn);

            match result {
                Ok(1) => count += 1,
                Ok(_) => {} // Already exists, no action
                Err(e) => {
                    warn!(
                        "Failed to insert scheduled execution for rule {} at {}: {}",
                        rule.id, scheduled_hour, e
                    );
                }
            }
        }

        Ok(count)
    }

    fn calculate_hours_for_rule(
        &self,
        rule: &AutomationRule,
        date: NaiveDate,
    ) -> Result<Vec<u32>, String> {
        let price_service = PriceService::new(self.pool.clone());

        match rule.rule_type.as_str() {
            "cheapest_hours" => {
                let hours_needed = rule
                    .config
                    .get("cheapest_hours")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(6) as usize;

                let cheapest = price_service
                    .get_cheapest_hours(date, hours_needed)
                    .map_err(|e| e.to_string())?;

                Ok(cheapest.iter().map(|p| p.timestamp.hour()).collect())
            }
            "price_threshold" => {
                let threshold = rule
                    .config
                    .get("price_threshold")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.10);

                let prices = price_service
                    .get_prices_for_date(date)
                    .map_err(|e| e.to_string())?;

                let hours: Vec<u32> = prices
                    .iter()
                    .filter(|p| p.price <= threshold)
                    .map(|p| p.timestamp.hour())
                    .collect();

                Ok(hours)
            }
            "time_schedule" => {
                let start_str = rule
                    .config
                    .get("time_range_start")
                    .and_then(|v| v.as_str())
                    .unwrap_or("00:00");
                let end_str = rule
                    .config
                    .get("time_range_end")
                    .and_then(|v| v.as_str())
                    .unwrap_or("23:59");

                let start_hour: u32 = start_str
                    .split(':')
                    .next()
                    .and_then(|h| h.parse().ok())
                    .unwrap_or(0);
                let end_hour: u32 = end_str
                    .split(':')
                    .next()
                    .and_then(|h| h.parse().ok())
                    .unwrap_or(23);

                // Handle overnight ranges
                let hours: Vec<u32> = if start_hour <= end_hour {
                    (start_hour..=end_hour).collect()
                } else {
                    (start_hour..24).chain(0..=end_hour).collect()
                };

                Ok(hours)
            }
            _ => Ok(vec![]), // Manual or unknown rule types don't schedule
        }
    }

    /// Mark pending hours that have passed as "missed"
    pub fn mark_missed_hours(&self) -> Result<usize, String> {
        let mut conn = self.pool.get().map_err(|e| e.to_string())?;

        let now = Local::now().naive_local();
        let current_hour_start = now.date().and_hms_opt(now.hour(), 0, 0).unwrap();

        // Mark all pending scheduled executions where the hour has passed as missed
        let count = diesel::update(
            scheduled_executions::table
                .filter(scheduled_executions::status.eq(ExecutionStatus::Pending.as_str()))
                .filter(scheduled_executions::scheduled_hour.lt(current_hour_start)),
        )
        .set(scheduled_executions::status.eq(ExecutionStatus::Missed.as_str()))
        .execute(&mut conn)
        .map_err(|e| e.to_string())?;

        if count > 0 {
            info!("Marked {} scheduled executions as missed", count);
        }

        Ok(count)
    }

    /// Get scheduled executions for a specific date and user
    pub fn get_schedule_for_date(
        &self,
        user_id: i32,
        date: NaiveDate,
    ) -> Result<Vec<(ScheduledExecution, String, String)>, String> {
        let mut conn = self.pool.get().map_err(|e| e.to_string())?;

        let start = date.and_hms_opt(0, 0, 0).unwrap();
        let end = date.and_hms_opt(23, 59, 59).unwrap();

        // Join with automation_rules and devices to get device_name and rule_name
        let results: Vec<(ScheduledExecution, String, String)> = scheduled_executions::table
            .inner_join(automation_rules::table.inner_join(devices::table))
            .filter(automation_rules::user_id.eq(user_id))
            .filter(scheduled_executions::scheduled_hour.ge(start))
            .filter(scheduled_executions::scheduled_hour.le(end))
            .select((
                ScheduledExecution::as_select(),
                devices::name,
                automation_rules::name,
            ))
            .order(scheduled_executions::scheduled_hour.asc())
            .load(&mut conn)
            .map_err(|e| e.to_string())?;

        Ok(results)
    }

    /// Delete scheduled executions for a rule (when rule is deleted or disabled)
    pub fn delete_schedule_for_rule(&self, rule_id: i32) -> Result<usize, String> {
        let mut conn = self.pool.get().map_err(|e| e.to_string())?;

        // Only delete pending executions
        let count = diesel::delete(
            scheduled_executions::table
                .filter(scheduled_executions::rule_id.eq(rule_id))
                .filter(scheduled_executions::status.eq(ExecutionStatus::Pending.as_str())),
        )
        .execute(&mut conn)
        .map_err(|e| e.to_string())?;

        info!(
            "Deleted {} pending scheduled executions for rule {}",
            count, rule_id
        );
        Ok(count)
    }

    /// Recompute schedule for a rule (delete pending and recompute)
    pub fn recompute_schedule_for_rule(&self, rule_id: i32) -> Result<usize, String> {
        // Delete pending schedules
        self.delete_schedule_for_rule(rule_id)?;

        // Recompute for today and tomorrow
        let today = Local::now().date_naive();
        let tomorrow = today + chrono::Duration::days(1);

        let count_today = self.compute_schedule_for_rule(rule_id, today).unwrap_or(0);
        let count_tomorrow = self.compute_schedule_for_rule(rule_id, tomorrow).unwrap_or(0);

        Ok(count_today + count_tomorrow)
    }
}
