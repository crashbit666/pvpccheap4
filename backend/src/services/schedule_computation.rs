use crate::db::DbPool;
use crate::models::{AutomationRule, ExecutionStatus, NewScheduledExecution, Price};
use crate::schema::{automation_rules, prices, scheduled_executions};
use crate::services::price_fetcher::PriceService;
use chrono::{Duration, NaiveDate, NaiveDateTime, Timelike};
use diesel::prelude::*;
use log::{error, info, warn};
use std::collections::HashSet;

/// Statuses meaning the device was (or is being) switched for that hour.
/// They count towards a cheapest_hours rule's N hours.
const ACTED_STATUSES: [&str; 3] = ["executed", "executing", "retrying"];

/// Service for computing and managing scheduled executions.
///
/// A rule's schedule is organised in *instances*: the window that starts on a
/// given date (e.g. 19:00 on D to 07:00 on D+1). Recomputing an instance only
/// replaces its pending hours: hours already executed are kept and count
/// towards the rule's N hours, and only hours from now on are picked.
pub struct ScheduleComputationService {
    pool: DbPool,
}

impl ScheduleComputationService {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Recompute yesterday's, today's and tomorrow's instances of every rule.
    /// Disabled rules lose their pending hours. Idempotent: safe to run every hour.
    pub fn refresh_all(&self) -> Result<usize, String> {
        self.mark_missed_hours()?;

        let mut conn = self.pool.get().map_err(|e| e.to_string())?;
        let rules: Vec<AutomationRule> = automation_rules::table
            .load(&mut conn)
            .map_err(|e| e.to_string())?;

        let mut total = 0;
        for rule in &rules {
            total += self.refresh_rule_instances(&mut conn, rule);
        }
        info!("Schedules refreshed: {} pending hours for {} rules", total, rules.len());
        Ok(total)
    }

    /// Recompute the schedule of one rule (after it is created, edited or toggled).
    pub fn refresh_rule(&self, rule_id: i32) -> Result<usize, String> {
        self.mark_missed_hours()?;

        let mut conn = self.pool.get().map_err(|e| e.to_string())?;
        let rule: AutomationRule = automation_rules::table
            .find(rule_id)
            .first(&mut conn)
            .map_err(|e| e.to_string())?;

        Ok(self.refresh_rule_instances(&mut conn, &rule))
    }

    fn refresh_rule_instances(&self, conn: &mut PgConnection, rule: &AutomationRule) -> usize {
        let today = crate::time::today();
        let mut total = 0;
        // Yesterday's instance still matters for windows that cross midnight
        for date in [today - Duration::days(1), today, today + Duration::days(1)] {
            match self.recompute_rule_instance(conn, rule, date) {
                Ok(n) => total += n,
                Err(e) => error!("Failed to compute schedule for rule {} on {}: {}", rule.id, date, e),
            }
        }
        total
    }

    /// Replace the pending hours of the rule's instance starting on `date`.
    /// Returns how many pending hours it now has.
    fn recompute_rule_instance(
        &self,
        conn: &mut PgConnection,
        rule: &AutomationRule,
        date: NaiveDate,
    ) -> Result<usize, String> {
        let mut total = 0;
        for group in instance_groups(rule, date) {
            total += self.recompute_group(conn, rule, &group)?;
        }
        Ok(total)
    }

    /// Replace the pending hours of one group of windows (they share an hour budget).
    fn recompute_group(
        &self,
        conn: &mut PgConnection,
        rule: &AutomationRule,
        group: &WindowGroup,
    ) -> Result<usize, String> {
        let (Some(span_start), Some(span_end)) = (
            group.windows.iter().map(|w| w.0).min(),
            group.windows.iter().map(|w| w.1).max(),
        ) else {
            return Ok(0);
        };

        // Rows of this group: the rule's rows inside any of its windows
        let rows: Vec<(i32, NaiveDateTime, String)> = scheduled_executions::table
            .filter(scheduled_executions::rule_id.eq(rule.id))
            .filter(scheduled_executions::scheduled_hour.ge(span_start))
            .filter(scheduled_executions::scheduled_hour.lt(span_end))
            .select((
                scheduled_executions::id,
                scheduled_executions::scheduled_hour,
                scheduled_executions::status,
            ))
            .load(conn)
            .map_err(|e| e.to_string())?;
        let (pending, handled): (Vec<_>, Vec<_>) = rows
            .into_iter()
            .filter(|(_, hour, _)| group.contains(hour))
            .partition(|(_, _, status)| status == ExecutionStatus::Pending.as_str());

        let pending_ids: Vec<i32> = pending.iter().map(|(id, _, _)| *id).collect();
        diesel::delete(scheduled_executions::table.filter(scheduled_executions::id.eq_any(pending_ids)))
            .execute(conn)
            .map_err(|e| e.to_string())?;

        if !rule.is_enabled {
            return Ok(0);
        }

        // Hours already past the pending stage stay as they are
        let handled_hours: HashSet<NaiveDateTime> = handled.iter().map(|(_, h, _)| *h).collect();
        let acted = handled
            .iter()
            .filter(|(_, _, status)| ACTED_STATUSES.contains(&status.as_str()))
            .count();
        let is_free = |h: &NaiveDateTime| !handled_hours.contains(h);
        let current_hour = crate::time::current_hour();

        let picks: Vec<NaiveDateTime> = match rule.rule_type.as_str() {
            "cheapest_hours" => {
                if let Some(&(start, end)) = group
                    .windows
                    .iter()
                    .find(|(start, end)| !self.prices_complete(*start, *end))
                {
                    // Picking from a partial window would choose expensive hours
                    // (e.g. only the evening of an overnight window)
                    info!(
                        "Rule {}: prices for {}..{} not complete yet, not scheduling",
                        rule.id, start, end
                    );
                    return Ok(0);
                }
                let hours_needed = group.hours.unwrap_or(0).saturating_sub(acted);
                let mut candidates: Vec<Price> = Vec::new();
                for &(start, end) in &group.windows {
                    candidates.extend(
                        load_prices(conn, start.max(current_hour), end)?
                            .into_iter()
                            .filter(|p| is_free(&p.timestamp)),
                    );
                }
                candidates.sort_by(|a, b| {
                    a.price
                        .total_cmp(&b.price)
                        .then(a.timestamp.cmp(&b.timestamp))
                });
                candidates
                    .into_iter()
                    .take(hours_needed)
                    .map(|p| p.timestamp)
                    .collect()
            }
            "price_threshold" => {
                let threshold = rule
                    .config
                    .get("price_threshold")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.10);
                let mut hours = Vec::new();
                for &(start, end) in &group.windows {
                    hours.extend(
                        load_prices(conn, start.max(current_hour), end)?
                            .into_iter()
                            .filter(|p| p.price <= threshold && is_free(&p.timestamp))
                            .map(|p| p.timestamp),
                    );
                }
                hours
            }
            "time_schedule" => {
                let mut hours = Vec::new();
                for &(start, end) in &group.windows {
                    let mut h = start.max(current_hour);
                    while h < end {
                        if is_free(&h) {
                            hours.push(h);
                        }
                        h += Duration::hours(1);
                    }
                }
                hours
            }
            _ => vec![],
        };

        for scheduled_hour in &picks {
            let new_execution = NewScheduledExecution {
                rule_id: rule.id,
                scheduled_hour: *scheduled_hour,
                expected_action: rule.action.clone(),
                status: ExecutionStatus::Pending.as_str().to_string(),
            };
            if let Err(e) = diesel::insert_into(scheduled_executions::table)
                .values(&new_execution)
                .on_conflict((scheduled_executions::rule_id, scheduled_executions::scheduled_hour))
                .do_nothing()
                .execute(conn)
            {
                warn!(
                    "Failed to insert scheduled execution for rule {} at {}: {}",
                    rule.id, scheduled_hour, e
                );
            }
        }

        Ok(picks.len())
    }

    /// True when every day touched by [start, end) has its full set of prices.
    fn prices_complete(&self, start: NaiveDateTime, end: NaiveDateTime) -> bool {
        let price_service = PriceService::new(self.pool.clone());
        let last_day = (end - Duration::seconds(1)).date();
        let mut day = start.date();
        while day <= last_day {
            if !price_service.has_prices_for_date(day).unwrap_or(false) {
                return false;
            }
            day += Duration::days(1);
        }
        true
    }

    /// Put rows left in "executing" by a crashed runner back in the retry queue.
    /// Rows of past hours are marked missed by `mark_missed_hours` instead.
    pub fn reset_interrupted_executions(&self) -> Result<usize, String> {
        let mut conn = self.pool.get().map_err(|e| e.to_string())?;
        diesel::update(
            scheduled_executions::table
                .filter(scheduled_executions::status.eq(ExecutionStatus::Executing.as_str()))
                .filter(scheduled_executions::scheduled_hour.ge(crate::time::current_hour())),
        )
        .set((
            scheduled_executions::status.eq(ExecutionStatus::Retrying.as_str()),
            scheduled_executions::next_retry_at.eq(Some(crate::time::now_naive())),
        ))
        .execute(&mut conn)
        .map_err(|e| e.to_string())
    }

    /// Mark pending (or abandoned executing) hours that have passed as "missed"
    pub fn mark_missed_hours(&self) -> Result<usize, String> {
        let mut conn = self.pool.get().map_err(|e| e.to_string())?;

        let now = crate::time::now_naive();
        // An hour is considered "missed" if we're past the start of that hour
        // e.g., if it's 14:14, then 13:00 is missed (we should have executed at 13:00)
        let current_hour_start = now.date().and_hms_opt(now.hour(), 0, 0).unwrap();

        // Use lt (less than) because the current hour might still be executing
        let count = diesel::update(
            scheduled_executions::table
                .filter(scheduled_executions::status.eq_any([
                    ExecutionStatus::Pending.as_str(),
                    ExecutionStatus::Executing.as_str(),
                ]))
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
}

/// Number of hours a cheapest_hours rule wants per window.
fn cheapest_hours_needed(rule: &AutomationRule) -> usize {
    rule.config
        .get("cheapest_hours")
        .and_then(|v| v.as_i64())
        .unwrap_or(6)
        .clamp(0, 24) as usize
}

/// Prices in [start, end), in time order.
fn load_prices(
    conn: &mut PgConnection,
    start: NaiveDateTime,
    end: NaiveDateTime,
) -> Result<Vec<Price>, String> {
    prices::table
        .filter(prices::timestamp.ge(start))
        .filter(prices::timestamp.lt(end))
        .order(prices::timestamp.asc())
        .load(conn)
        .map_err(|e| e.to_string())
}

/// A time window of a rule, in whole hours: [start, end), crossing midnight
/// when end <= start. `hours` is its own budget in "per_window" mode.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowSpec {
    pub start: u32,
    pub end: u32,
    pub hours: Option<usize>,
}

impl WindowSpec {
    /// Hours of the day (0-23) the window covers.
    pub fn hours_of_day(&self) -> Vec<u32> {
        if self.start < self.end {
            (self.start..self.end).collect()
        } else {
            (self.start..24).chain(0..self.end).collect()
        }
    }
}

/// The windows of a rule: the "windows" list, or the legacy single
/// time_range_start/time_range_end pair. Empty when the rule has none.
pub fn window_specs(config: &serde_json::Value) -> Vec<WindowSpec> {
    if let Some(list) = config.get("windows").and_then(|w| w.as_array()) {
        return list
            .iter()
            .filter_map(|w| {
                let (start, _) = parse_time_of_day(w.get("start")?.as_str()?)?;
                let (end_h, end_m) = parse_time_of_day(w.get("end")?.as_str()?)?;
                Some(WindowSpec {
                    start,
                    end: if end_m == 0 { end_h } else { end_h + 1 },
                    hours: w.get("hours").and_then(|h| h.as_u64()).map(|h| h as usize),
                })
            })
            .collect();
    }
    match (window_start_hour(config), window_end_hour(config)) {
        (Some(start), Some(end)) => vec![WindowSpec { start, end, hours: None }],
        _ => vec![],
    }
}

/// True when a cheapest_hours rule picks its hours separately in each window.
fn is_per_window(config: &serde_json::Value) -> bool {
    config.get("distribution").and_then(|d| d.as_str()) == Some("per_window")
}

/// Windows of one rule instance that share an hour budget.
#[derive(Debug, PartialEq)]
struct WindowGroup {
    /// Concrete [start, end) spans
    windows: Vec<(NaiveDateTime, NaiveDateTime)>,
    /// cheapest_hours: how many hours to pick; unused by the other rule types
    hours: Option<usize>,
}

impl WindowGroup {
    fn contains(&self, hour: &NaiveDateTime) -> bool {
        self.windows.iter().any(|(start, end)| start <= hour && hour < end)
    }
}

/// Window groups of the rule's instance for `date`: the windows that end on
/// `date`. A window crossing midnight belongs to the day it ends, so with
/// 14:00-16:00 and 20:00-06:00 the day is "last night + this afternoon": all of
/// it priced by ~20:15 the evening before, in time to choose before it starts.
/// (Anchoring on the start day would put this afternoon with tonight, whose
/// prices only arrive after the afternoon is over.)
fn instance_groups(rule: &AutomationRule, date: NaiveDate) -> Vec<WindowGroup> {
    let at = |d: NaiveDate, hour: u32| d.and_hms_opt(0, 0, 0).unwrap() + Duration::hours(hour as i64);
    let span = |w: &WindowSpec| {
        if w.start < w.end {
            (at(date, w.start), at(date, w.end))
        } else {
            (at(date - Duration::days(1), w.start), at(date, w.end))
        }
    };
    let whole_day = WindowSpec { start: 0, end: 24, hours: None };

    let mut specs = window_specs(&rule.config);
    match rule.rule_type.as_str() {
        "cheapest_hours" => {
            if specs.is_empty() {
                specs.push(whole_day);
            }
            let total = cheapest_hours_needed(rule);
            if is_per_window(&rule.config) {
                specs
                    .iter()
                    .map(|w| WindowGroup { windows: vec![span(w)], hours: Some(w.hours.unwrap_or(total)) })
                    .collect()
            } else {
                vec![WindowGroup { windows: specs.iter().map(span).collect(), hours: Some(total) }]
            }
        }
        "time_schedule" => {
            if specs.is_empty() {
                // Legacy: a missing bound means the start or end of the day
                specs.push(WindowSpec {
                    start: window_start_hour(&rule.config).unwrap_or(0),
                    end: window_end_hour(&rule.config).unwrap_or(24),
                    hours: None,
                });
            }
            specs
                .iter()
                .map(|w| WindowGroup { windows: vec![span(w)], hours: None })
                .collect()
        }
        "price_threshold" => vec![WindowGroup { windows: vec![span(&whole_day)], hours: None }],
        _ => vec![],
    }
}

/// Parse a rule config time ("HH:MM") into (hour, minute), rejecting out-of-range values.
pub fn parse_time_of_day(s: &str) -> Option<(u32, u32)> {
    let (h, m) = s.trim().split_once(':')?;
    let (h, m) = (h.parse::<u32>().ok()?, m.parse::<u32>().ok()?);
    (h < 24 && m < 60).then_some((h, m))
}

/// First hour of a rule's time window (`time_range_start`).
fn window_start_hour(config: &serde_json::Value) -> Option<u32> {
    let (h, _) = parse_time_of_day(config.get("time_range_start")?.as_str()?)?;
    Some(h)
}

/// Exclusive end hour of a rule's time window (`time_range_end`):
/// "22:00" stops before 22:00, "22:30" and "23:59" include that hour.
fn window_end_hour(config: &serde_json::Value) -> Option<u32> {
    let (h, m) = parse_time_of_day(config.get("time_range_end")?.as_str()?)?;
    Some(if m == 0 { h } else { h + 1 })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_time_of_day() {
        assert_eq!(parse_time_of_day("07:00"), Some((7, 0)));
        assert_eq!(parse_time_of_day(" 23:59 "), Some((23, 59)));
        assert_eq!(parse_time_of_day("7h30"), None);
        assert_eq!(parse_time_of_day("24:00"), None);
        assert_eq!(parse_time_of_day("12:60"), None);
        assert_eq!(parse_time_of_day("7"), None);
    }

    #[test]
    fn test_window_end_hour_is_exclusive() {
        let end = |t: &str| window_end_hour(&json!({ "time_range_end": t }));
        assert_eq!(end("22:00"), Some(22));
        assert_eq!(end("22:30"), Some(23));
        assert_eq!(end("23:59"), Some(24));
        assert_eq!(end("00:00"), Some(0));
        assert_eq!(end("bad"), None);
        assert_eq!(window_end_hour(&json!({})), None);
    }

    fn rule(rule_type: &str, config: serde_json::Value) -> AutomationRule {
        AutomationRule {
            id: 1,
            user_id: 1,
            device_id: 1,
            name: "test".to_string(),
            rule_type: rule_type.to_string(),
            action: "turn_on".to_string(),
            config,
            is_enabled: true,
            priority: 100,
            created_at: NaiveDateTime::default(),
            updated_at: NaiveDateTime::default(),
            last_triggered_at: None,
        }
    }

    fn dt(d: u32, h: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, d).unwrap().and_hms_opt(h, 0, 0).unwrap()
    }

    fn groups(r: AutomationRule) -> Vec<(Vec<(NaiveDateTime, NaiveDateTime)>, Option<usize>)> {
        let date = NaiveDate::from_ymd_opt(2026, 10, 19).unwrap();
        instance_groups(&r, date).into_iter().map(|g| (g.windows, g.hours)).collect()
    }

    #[test]
    fn test_instance_groups_legacy() {
        assert_eq!(
            groups(rule("cheapest_hours", json!({"cheapest_hours": 6}))),
            vec![(vec![(dt(19, 0), dt(20, 0))], Some(6))]
        );
        assert_eq!(
            groups(rule("cheapest_hours", json!({"cheapest_hours": 6, "time_range_start": "19:00", "time_range_end": "07:00"}))),
            vec![(vec![(dt(18, 19), dt(19, 7))], Some(6))]
        );
        assert_eq!(
            groups(rule("time_schedule", json!({"time_range_start": "22:00", "time_range_end": "23:59"}))),
            vec![(vec![(dt(19, 22), dt(20, 0))], None)]
        );
        assert_eq!(
            groups(rule("price_threshold", json!({"price_threshold": 0.1}))),
            vec![(vec![(dt(19, 0), dt(20, 0))], None)]
        );
        assert!(groups(rule("manual", json!({}))).is_empty());
    }

    #[test]
    fn test_instance_groups_several_windows() {
        let windows = json!([
            {"start": "14:00", "end": "16:00", "hours": 1},
            {"start": "20:00", "end": "06:00", "hours": 4}
        ]);
        // Total: one budget across both windows
        assert_eq!(
            groups(rule("cheapest_hours", json!({"cheapest_hours": 3, "windows": windows, "distribution": "total"}))),
            vec![(vec![(dt(19, 14), dt(19, 16)), (dt(18, 20), dt(19, 6))], Some(3))]
        );
        // Per window: each window with its own budget
        assert_eq!(
            groups(rule("cheapest_hours", json!({"cheapest_hours": 3, "windows": windows, "distribution": "per_window"}))),
            vec![
                (vec![(dt(19, 14), dt(19, 16))], Some(1)),
                (vec![(dt(18, 20), dt(19, 6))], Some(4))
            ]
        );
        // Time schedule: every hour of every window
        assert_eq!(
            groups(rule("time_schedule", json!({"windows": windows}))),
            vec![
                (vec![(dt(19, 14), dt(19, 16))], None),
                (vec![(dt(18, 20), dt(19, 6))], None)
            ]
        );
    }

    #[test]
    fn test_window_hours_of_day() {
        let w = |start, end| WindowSpec { start, end, hours: None };
        assert_eq!(w(14, 16).hours_of_day(), vec![14, 15]);
        assert_eq!(w(22, 2).hours_of_day(), vec![22, 23, 0, 1]);
        assert_eq!(w(0, 0).hours_of_day().len(), 24);
    }

    #[test]
    fn test_window_start_hour() {
        assert_eq!(window_start_hour(&json!({ "time_range_start": "19:00" })), Some(19));
        assert_eq!(window_start_hour(&json!({ "time_range_start": "19:45" })), Some(19));
    }
}
