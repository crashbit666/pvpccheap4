use crate::db::DbPool;
use crate::models::{AutomationRule, ExecutionStatus, NewScheduledExecution, Price, ScheduledExecution};
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
        // Get timestamps to schedule based on rule type
        // Returns NaiveDateTime to handle overnight windows spanning two days
        let timestamps_to_schedule = self.calculate_timestamps_for_rule(rule, date)?;

        let mut count = 0;
        for scheduled_hour in timestamps_to_schedule {
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

    /// Calculate timestamps for scheduling a rule on a given date
    /// For overnight windows (e.g., 19:00-08:00), returns timestamps from both days
    fn calculate_timestamps_for_rule(
        &self,
        rule: &AutomationRule,
        date: NaiveDate,
    ) -> Result<Vec<NaiveDateTime>, String> {
        let price_service = PriceService::new(self.pool.clone());

        match rule.rule_type.as_str() {
            "cheapest_hours" => {
                let hours_needed = rule
                    .config
                    .get("cheapest_hours")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(6) as usize;

                // Check for optional time window
                let start_hour = rule
                    .config
                    .get("time_range_start")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.split(':').next())
                    .and_then(|h| h.parse::<u32>().ok());

                let end_hour = rule
                    .config
                    .get("time_range_end")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.split(':').next())
                    .and_then(|h| h.parse::<u32>().ok());

                // Get prices based on window type
                let filtered_prices: Vec<Price> = match (start_hour, end_hour) {
                    (Some(start), Some(end)) if start > end => {
                        // Overnight window: e.g., 19:00-08:00
                        // Get prices from today (start_hour to 23:00) and tomorrow (00:00 to end_hour)
                        let today_prices = price_service
                            .get_prices_for_date(date)
                            .map_err(|e| e.to_string())?;

                        let tomorrow = date + chrono::Duration::days(1);
                        let tomorrow_prices = price_service
                            .get_prices_for_date(tomorrow)
                            .unwrap_or_else(|_| vec![]); // Tomorrow's prices may not be available yet

                        let mut combined: Vec<Price> = today_prices
                            .into_iter()
                            .filter(|p| p.timestamp.hour() >= start)
                            .collect();

                        combined.extend(
                            tomorrow_prices
                                .into_iter()
                                .filter(|p| p.timestamp.hour() <= end)
                        );

                        combined
                    }
                    (Some(start), Some(end)) => {
                        // Normal daytime window: e.g., 06:00-22:00
                        let all_prices = price_service
                            .get_prices_for_date(date)
                            .map_err(|e| e.to_string())?;

                        all_prices
                            .into_iter()
                            .filter(|p| {
                                let hour = p.timestamp.hour();
                                hour >= start && hour <= end
                            })
                            .collect()
                    }
                    _ => {
                        // No time window, use all hours of the day
                        price_service
                            .get_prices_for_date(date)
                            .map_err(|e| e.to_string())?
                    }
                };

                // Sort by price and take the cheapest N hours
                let mut sorted_prices = filtered_prices;
                sorted_prices.sort_by(|a, b| a.price.partial_cmp(&b.price).unwrap());

                let cheapest_timestamps: Vec<NaiveDateTime> = sorted_prices
                    .iter()
                    .take(hours_needed)
                    .map(|p| p.timestamp)
                    .collect();

                Ok(cheapest_timestamps)
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

                let timestamps: Vec<NaiveDateTime> = prices
                    .iter()
                    .filter(|p| p.price <= threshold)
                    .map(|p| p.timestamp)
                    .collect();

                Ok(timestamps)
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
                let timestamps: Vec<NaiveDateTime> = if start_hour <= end_hour {
                    (start_hour..=end_hour)
                        .filter_map(|h| date.and_hms_opt(h, 0, 0))
                        .collect()
                } else {
                    // Overnight: today's hours from start to 23, then tomorrow's 0 to end
                    let today_hours: Vec<NaiveDateTime> = (start_hour..24)
                        .filter_map(|h| date.and_hms_opt(h, 0, 0))
                        .collect();
                    let tomorrow = date + chrono::Duration::days(1);
                    let tomorrow_hours: Vec<NaiveDateTime> = (0..=end_hour)
                        .filter_map(|h| tomorrow.and_hms_opt(h, 0, 0))
                        .collect();
                    today_hours.into_iter().chain(tomorrow_hours).collect()
                };

                Ok(timestamps)
            }
            _ => Ok(vec![]), // Manual or unknown rule types don't schedule
        }
    }

    /// Check if a rule has an overnight time window (crosses midnight)
    pub fn rule_has_overnight_window(&self, rule: &AutomationRule) -> bool {
        if rule.rule_type != "cheapest_hours" && rule.rule_type != "time_schedule" {
            return false;
        }

        let start_hour = rule
            .config
            .get("time_range_start")
            .and_then(|v| v.as_str())
            .and_then(|s| s.split(':').next())
            .and_then(|h| h.parse::<u32>().ok());

        let end_hour = rule
            .config
            .get("time_range_end")
            .and_then(|v| v.as_str())
            .and_then(|s| s.split(':').next())
            .and_then(|h| h.parse::<u32>().ok());

        match (start_hour, end_hour) {
            (Some(start), Some(end)) => start > end,
            _ => false,
        }
    }

    /// Recompute schedules for all rules with overnight windows
    /// Called when tomorrow's prices become available (around 20:30)
    pub fn recompute_overnight_rules(&self) -> Result<usize, String> {
        let mut conn = self.pool.get().map_err(|e| e.to_string())?;

        // Get all enabled rules
        let rules: Vec<AutomationRule> = automation_rules::table
            .filter(automation_rules::is_enabled.eq(true))
            .load(&mut conn)
            .map_err(|e| e.to_string())?;

        let today = Local::now().date_naive();
        let mut total_recomputed = 0;

        for rule in rules {
            if self.rule_has_overnight_window(&rule) {
                info!(
                    "Recomputing overnight rule {} ({}) now that tomorrow's prices are available",
                    rule.id, rule.name
                );

                // Delete pending executions for this rule that are for today's overnight window
                // (we'll recalculate them with complete data)
                if let Err(e) = self.delete_pending_overnight_for_rule(&rule, today) {
                    warn!("Failed to delete pending overnight executions for rule {}: {}", rule.id, e);
                }

                // Recompute for today (which will now include tomorrow's prices)
                match self.compute_schedule_for_rule_internal(&mut conn, &rule, today) {
                    Ok(count) => {
                        total_recomputed += count;
                        info!("Recomputed {} executions for overnight rule {}", count, rule.id);
                    }
                    Err(e) => {
                        error!("Failed to recompute schedule for rule {}: {}", rule.id, e);
                    }
                }
            }
        }

        if total_recomputed > 0 {
            info!(
                "Recomputed {} scheduled executions for overnight rules",
                total_recomputed
            );
        }

        Ok(total_recomputed)
    }

    /// Delete pending overnight executions for a rule on a given date
    /// This is called before recomputing to replace with updated calculations
    fn delete_pending_overnight_for_rule(&self, rule: &AutomationRule, date: NaiveDate) -> Result<usize, String> {
        let mut conn = self.pool.get().map_err(|e| e.to_string())?;

        let start_hour = rule
            .config
            .get("time_range_start")
            .and_then(|v| v.as_str())
            .and_then(|s| s.split(':').next())
            .and_then(|h| h.parse::<u32>().ok())
            .unwrap_or(0);

        let end_hour = rule
            .config
            .get("time_range_end")
            .and_then(|v| v.as_str())
            .and_then(|s| s.split(':').next())
            .and_then(|h| h.parse::<u32>().ok())
            .unwrap_or(8);

        // For overnight rules, delete from start_hour today to end_hour tomorrow
        // Only delete the actual overnight window, not the entire next day
        let window_start = date.and_hms_opt(start_hour, 0, 0).unwrap();
        let tomorrow = date + chrono::Duration::days(1);
        let window_end = tomorrow.and_hms_opt(end_hour, 59, 59).unwrap();

        let count = diesel::delete(
            scheduled_executions::table
                .filter(scheduled_executions::rule_id.eq(rule.id))
                .filter(scheduled_executions::status.eq(ExecutionStatus::Pending.as_str()))
                .filter(scheduled_executions::scheduled_hour.ge(window_start))
                .filter(scheduled_executions::scheduled_hour.le(window_end)),
        )
        .execute(&mut conn)
        .map_err(|e| e.to_string())?;

        Ok(count)
    }

    /// Mark pending hours that have passed as "missed"
    pub fn mark_missed_hours(&self) -> Result<usize, String> {
        let mut conn = self.pool.get().map_err(|e| e.to_string())?;

        let now = Local::now().naive_local();
        // An hour is considered "missed" if we're past the start of that hour
        // e.g., if it's 14:14, then 13:00 is missed (we should have executed at 13:00)
        let current_hour_start = now.date().and_hms_opt(now.hour(), 0, 0).unwrap();

        // Mark all pending scheduled executions where the hour has passed as missed
        // Use lt (less than) because the current hour might still be executing
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
