//! Reads the power of the plugs that are on, every few minutes, so the
//! savings use what the appliance really drew instead of a typed power.
//! With a rule's "stop_when_done", a plug whose power drops (the scooter is
//! charged, the water is hot) is switched off and the rule's remaining hours
//! in that window are not used. The readings also tell how long the
//! appliance runs before it finishes, to suggest the hours a rule needs.

use crate::{
    db::DbPool,
    integrations::{ProviderRegistry, SmartHomeProvider},
    models::{AutomationRule, NewRuleExecution},
    schema::{automation_rules, devices, power_readings, rule_executions, scheduled_executions, user_integrations},
    services::{credentials, schedule_computation::ScheduleComputationService},
};
use chrono::{Duration, NaiveDateTime};
use serde_json::{Value, json};
use diesel::prelude::*;
use log::{debug, info, warn};
use std::sync::Arc;

/// Readings older than this are deleted
const KEEP_DAYS: i64 = 400;

/// The appliance has finished when its power stays below this share of the
/// highest power of the last hours...
const DONE_SHARE: f32 = 0.05;
/// ...clamped to these watts (chargers draw a few watts when full)
const DONE_MIN_WATTS: f32 = 3.0;
const DONE_MAX_WATTS: f32 = 30.0;
/// Two low readings this close in a row: not a single dip
const DONE_GAP_MINUTES: i64 = 11;

/// The threshold below which the appliance is idle, given its peak power
fn idle_threshold(peak: f32) -> f32 {
    (peak * DONE_SHARE).clamp(DONE_MIN_WATTS, DONE_MAX_WATTS)
}

/// True when the last two readings (newest first) are low and close together
fn has_finished(readings: &[(NaiveDateTime, f32)]) -> bool {
    let [(last_at, last), (prev_at, prev), ..] = readings else { return false };
    let peak = readings.iter().map(|(_, w)| *w).fold(0.0, f32::max);
    let threshold = idle_threshold(peak);
    *last_at - *prev_at <= Duration::minutes(DONE_GAP_MINUTES) && *last < threshold && *prev < threshold
}

/// Minutes between two readings (the cron job)
const SAMPLE_MINUTES: i64 = 5;
/// Readings further apart than this belong to different runs: the plug was
/// switched off before the appliance finished
const RUN_GAP_HOURS: i64 = 12;
/// Drawing power again this soon after finishing: the appliance has a
/// thermostat and switches itself on and off
const RESTART_MINUTES: i64 = 60;
/// Finished runs needed before suggesting hours, and how many recent ones count
const MIN_RUNS: usize = 3;
const RECENT_RUNS: usize = 10;
/// Days of readings looked at
pub const USAGE_DAYS: i64 = 30;

/// How long the appliance runs before it finishes, learnt from its readings
#[derive(Debug, Default, PartialEq, serde::Serialize)]
pub struct Usage {
    /// Runs that ended with the power dropping (the appliance finished)
    pub finished_runs: usize,
    /// Runs cut off before finishing (the rule's hours ran out)
    pub unfinished_runs: usize,
    /// Times the appliance drew power again soon after finishing
    pub restarts: usize,
    /// Minutes of a typical run (80th percentile of the recent ones)
    pub typical_minutes: Option<i64>,
    /// Whole hours that cover a typical run
    pub suggested_hours: Option<i64>,
}

/// Splits the readings (oldest first) into runs: drawing power until two low
/// readings in a row. Counts only the minutes it drew power, so a run spread
/// over several cheap hours adds up.
pub fn learn_usage(readings: &[(NaiveDateTime, f32)]) -> Usage {
    let mut usage = Usage::default();
    let mut runs: Vec<i64> = Vec::new();
    let mut active = 0i64; // minutes drawing power in the current run
    let mut peak = 0f32;
    let mut low_at: Option<NaiveDateTime> = None; // first of the low readings
    let mut finished_at: Option<NaiveDateTime> = None;
    let mut last_at: Option<NaiveDateTime> = None;

    for &(at, watts) in readings {
        if last_at.is_some_and(|last| at - last > Duration::hours(RUN_GAP_HOURS)) {
            if active > 0 {
                usage.unfinished_runs += 1;
            }
            active = 0;
            peak = 0.0;
            low_at = None;
        }
        let gap_ok = last_at.is_some_and(|last| at - last <= Duration::minutes(DONE_GAP_MINUTES));

        if watts >= idle_threshold(peak) {
            if active == 0 && finished_at.is_some_and(|done| at - done <= Duration::minutes(RESTART_MINUTES)) {
                usage.restarts += 1;
            }
            active += SAMPLE_MINUTES;
            peak = peak.max(watts);
            low_at = None;
        } else if active > 0 {
            match low_at {
                Some(_) if gap_ok => {
                    runs.push(active);
                    usage.finished_runs += 1;
                    finished_at = Some(at);
                    active = 0;
                    peak = 0.0;
                    low_at = None;
                }
                _ => low_at = Some(at),
            }
        }
        last_at = Some(at);
    }

    // A thermostat's runs say nothing about the hours it needs
    if runs.len() >= MIN_RUNS && usage.restarts * 2 < runs.len() {
        let mut recent: Vec<i64> = runs[runs.len().saturating_sub(RECENT_RUNS)..].to_vec();
        recent.sort_unstable();
        let typical = recent[(recent.len() * 8).div_ceil(10) - 1];
        usage.typical_minutes = Some(typical);
        usage.suggested_hours = Some((typical as f64 / 60.0).ceil().max(1.0) as i64);
    }
    usage
}

/// What the device's readings of the last USAGE_DAYS say
pub fn device_usage(pool: &DbPool, device_id: i32) -> Result<Usage, String> {
    let mut conn = pool.get().map_err(|e| e.to_string())?;
    let readings: Vec<(NaiveDateTime, f32)> = power_readings::table
        .filter(power_readings::device_id.eq(device_id))
        .filter(power_readings::measured_at.ge(crate::time::now_naive() - Duration::days(USAGE_DAYS)))
        .order(power_readings::measured_at.asc())
        .select((power_readings::measured_at, power_readings::watts))
        .load(&mut conn)
        .map_err(|e| e.to_string())?;
    Ok(learn_usage(&readings))
}

/// (device id, external id, is on, measures power, provider, stored credentials)
type Candidate = (i32, String, bool, Option<bool>, String, String);

/// Asks the plugs not asked yet whether they measure power, and stores the
/// power of the ones that do and are on.
pub async fn sample_power(pool: &DbPool, registry: Arc<ProviderRegistry>) {
    let candidates: Vec<Candidate> = {
        let Ok(mut conn) = pool.get() else { return };
        match devices::table
            .inner_join(user_integrations::table)
            .filter(user_integrations::is_active.eq(true))
            .filter(
                devices::measures_power
                    .is_null()
                    .or(devices::measures_power.eq(true).and(devices::is_on.eq(true))),
            )
            .select((
                devices::id,
                devices::external_id,
                devices::is_on,
                devices::measures_power,
                user_integrations::provider_name,
                user_integrations::credentials_json,
            ))
            .load(&mut conn)
        {
            Ok(rows) => rows,
            Err(e) => {
                warn!("Failed to load devices for power readings: {}", e);
                return;
            }
        }
    };

    for (device_id, external_id, is_on, measures, provider_name, stored) in candidates {
        let Some(provider) = registry.get(&provider_name) else { continue };
        let Ok(creds) = credentials::decrypt(&stored) else { continue };

        let measures = match measures {
            Some(m) => m,
            None => match provider.supports_power_reading(&creds, &external_id).await {
                Ok(m) => {
                    info!("Device {} measures power: {}", device_id, m);
                    if let Ok(mut conn) = pool.get() {
                        let _ = diesel::update(devices::table.find(device_id))
                            .set(devices::measures_power.eq(m))
                            .execute(&mut conn);
                    }
                    m
                }
                // Unreachable now (offline, expired session): ask next time
                Err(e) => {
                    debug!("Could not ask device {} for its abilities: {}", device_id, e);
                    continue;
                }
            },
        };
        if !measures || !is_on {
            continue;
        }

        match provider.read_power(&creds, &external_id).await {
            Ok(watts) => {
                if let Ok(mut conn) = pool.get() {
                    let _ = diesel::insert_into(power_readings::table)
                        .values((
                            power_readings::device_id.eq(device_id),
                            power_readings::measured_at.eq(crate::time::now_naive()),
                            power_readings::watts.eq(watts),
                        ))
                        .execute(&mut conn);
                }
                stop_if_finished(pool, provider.as_ref(), &creds, device_id, &external_id).await;
            }
            Err(e) => debug!("Could not read the power of device {}: {}", device_id, e),
        }
    }
}

/// Rules of the device with "stop_when_done" that switched it on this hour
fn rules_stopping_when_done(pool: &DbPool, device_id: i32) -> Vec<AutomationRule> {
    let Ok(mut conn) = pool.get() else { return vec![] };
    automation_rules::table
        .inner_join(scheduled_executions::table)
        .filter(automation_rules::device_id.eq(device_id))
        .filter(automation_rules::is_enabled.eq(true))
        .filter(automation_rules::action.eq("turn_on"))
        .filter(scheduled_executions::scheduled_hour.eq(crate::time::current_hour()))
        .filter(scheduled_executions::status.eq("executed"))
        .select(AutomationRule::as_select())
        .load(&mut conn)
        .unwrap_or_default()
        .into_iter()
        .filter(|r| r.config.get("stop_when_done").and_then(Value::as_bool) == Some(true))
        .collect()
}

async fn stop_if_finished(
    pool: &DbPool,
    provider: &dyn SmartHomeProvider,
    creds: &Value,
    device_id: i32,
    external_id: &str,
) {
    let rules = rules_stopping_when_done(pool, device_id);
    if rules.is_empty() {
        return;
    }
    let readings: Vec<(NaiveDateTime, f32)> = {
        let Ok(mut conn) = pool.get() else { return };
        power_readings::table
            .filter(power_readings::device_id.eq(device_id))
            .filter(power_readings::measured_at.ge(crate::time::now_naive() - Duration::hours(3)))
            .order(power_readings::measured_at.desc())
            .select((power_readings::measured_at, power_readings::watts))
            .load(&mut conn)
            .unwrap_or_default()
    };
    if !has_finished(&readings) {
        return;
    }
    let watts = readings[0].1;

    match provider.turn_off(creds, external_id).await {
        Ok(result) if result.success => {}
        // Still on: the next reading tries again
        Ok(result) => {
            warn!("Device {} finished but could not be turned off: {:?}", device_id, result.message);
            return;
        }
        Err(e) => {
            warn!("Device {} finished but could not be turned off: {}", device_id, e);
            return;
        }
    }
    info!("Device {} finished ({} W), turned off", device_id, watts);

    let Ok(mut conn) = pool.get() else { return };
    let _ = diesel::update(devices::table.find(device_id))
        .set(devices::is_on.eq(false))
        .execute(&mut conn);
    let schedules = ScheduleComputationService::new(pool.clone());
    for rule in rules {
        let left = schedules.finish_window(rule.id, crate::time::current_hour()).unwrap_or_else(|e| {
            warn!("Failed to mark the hours of rule {} as done: {}", rule.id, e);
            0
        });
        info!("Rule {}: appliance finished, {} hours not needed", rule.id, left);
        let _ = diesel::insert_into(rule_executions::table)
            .values(&NewRuleExecution {
                rule_id: rule.id,
                action_taken: "turn_off".to_string(),
                success: true,
                error_message: None,
                price_at_execution: None,
                device_state_before: Some(json!({ "is_on": true, "watts": watts })),
                device_state_after: Some(json!({ "is_on": false, "finished": true, "hours_left": left })),
            })
            .execute(&mut conn);
    }
}

/// Deletes readings older than KEEP_DAYS
pub fn delete_old_readings(pool: &DbPool) -> Result<usize, String> {
    let mut conn = pool.get().map_err(|e| e.to_string())?;
    let limit = crate::time::now_naive() - Duration::days(KEEP_DAYS);
    diesel::delete(power_readings::table.filter(power_readings::measured_at.lt(limit)))
        .execute(&mut conn)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(minute: i64) -> NaiveDateTime {
        chrono::NaiveDate::from_ymd_opt(2026, 10, 22).unwrap().and_hms_opt(2, 0, 0).unwrap() + Duration::minutes(minute)
    }

    #[test]
    fn test_charger_that_drops_has_finished() {
        // Newest first: 130 W while charging, then 2 W when full
        let readings = [(at(52), 2.0), (at(47), 2.5), (at(42), 60.0), (at(37), 130.0)];
        assert!(has_finished(&readings));
    }

    #[test]
    fn test_one_low_reading_is_not_enough() {
        assert!(!has_finished(&[(at(52), 1.0), (at(47), 130.0)]));
        assert!(!has_finished(&[(at(52), 1.0)]));
        assert!(!has_finished(&[]));
    }

    #[test]
    fn test_readings_far_apart_are_not_enough() {
        // A gap (failed readings, or yesterday's last one) is not "in a row"
        assert!(!has_finished(&[(at(52), 1.0), (at(22), 1.0), (at(17), 130.0)]));
    }

    #[test]
    fn test_threshold_follows_the_appliance() {
        // A 2 kW heater at 40 W is still on (threshold capped at 30 W)...
        assert!(!has_finished(&[(at(52), 40.0), (at(47), 40.0), (at(42), 2000.0)]));
        // ...and at 0 W it is done
        assert!(has_finished(&[(at(52), 0.0), (at(47), 0.0), (at(42), 2000.0)]));
        // A small charger tapering at 5 W of a 60 W peak is not done (3 W)
        assert!(!has_finished(&[(at(52), 5.0), (at(47), 5.0), (at(42), 60.0)]));
    }

    /// One run per night at 02:00: `minutes` drawing 130 W, then 2 W readings
    fn nightly_runs(minutes: &[i64]) -> Vec<(NaiveDateTime, f32)> {
        let mut readings = vec![];
        for (day, &run) in minutes.iter().enumerate() {
            let start = at(0) + Duration::days(day as i64);
            for i in 0..run / SAMPLE_MINUTES + 2 {
                let watts = if i < run / SAMPLE_MINUTES { 130.0 } else { 2.0 };
                readings.push((start + Duration::minutes(i * SAMPLE_MINUTES), watts));
            }
        }
        readings
    }

    #[test]
    fn test_learns_the_hours_a_charger_needs() {
        let usage = learn_usage(&nightly_runs(&[100, 110, 95, 140, 105]));
        assert_eq!(usage.finished_runs, 5);
        assert_eq!(usage.unfinished_runs, 0);
        // 80th percentile of 95, 100, 105, 110, 140 → 110 min → 2 h
        assert_eq!(usage.typical_minutes, Some(110));
        assert_eq!(usage.suggested_hours, Some(2));
    }

    #[test]
    fn test_needs_a_few_runs_before_suggesting() {
        let usage = learn_usage(&nightly_runs(&[100, 110]));
        assert_eq!(usage.finished_runs, 2);
        assert_eq!(usage.suggested_hours, None);
    }

    #[test]
    fn test_run_spread_over_cheap_hours_adds_up() {
        // 02:00–02:55 and 05:00–05:25 drawing power, then it finishes
        let mut readings: Vec<_> = (0..12).map(|i| (at(i * 5), 130.0)).collect();
        readings.extend((0..6).map(|i| (at(180 + i * 5), 130.0)));
        readings.extend([(at(210), 2.0), (at(215), 2.0)]);
        let runs: Vec<_> = (1..4).flat_map(|d| readings.iter().map(move |&(t, w)| (t + Duration::days(d), w))).collect();
        let usage = learn_usage(&runs);
        assert_eq!(usage.finished_runs, 3);
        assert_eq!(usage.typical_minutes, Some(90));
        assert_eq!(usage.suggested_hours, Some(2));
    }

    #[test]
    fn test_run_cut_off_is_unfinished() {
        // Switched off while still drawing power, and the next night it finishes
        let mut readings: Vec<_> = (0..12).map(|i| (at(i * 5), 130.0)).collect();
        readings.extend(nightly_runs(&[0, 30]).into_iter().skip(2));
        let usage = learn_usage(&readings);
        assert_eq!(usage.unfinished_runs, 1);
        assert_eq!(usage.finished_runs, 1);
    }

    #[test]
    fn test_thermostat_gets_no_suggestion() {
        // A heater: 20 min on, 15 min off, all night
        let readings: Vec<_> = (0..60).map(|i| (at(i * 5), if i % 7 < 4 { 1500.0 } else { 0.0 })).collect();
        let usage = learn_usage(&readings);
        assert!(usage.restarts >= 2);
        assert_eq!(usage.suggested_hours, None);
    }

    #[test]
    fn test_already_full_appliance_is_done() {
        // Switched on already charged: never drew power
        assert!(has_finished(&[(at(7), 1.0), (at(2), 1.2)]));
    }
}
