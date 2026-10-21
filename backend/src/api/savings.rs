//! How much the rules saved: the price paid in the hours a rule switched a
//! device on, against the average price of those days (what running at any
//! hour would have cost). Euros only for devices whose power the user set.

use crate::{
    db::DbPool,
    schema::{automation_rules, devices, prices, scheduled_executions},
    services::auth::Claims,
};
use actix_web::{HttpResponse, Responder, get, web};
use chrono::{Datelike, NaiveDate, NaiveDateTime, Timelike};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};

#[derive(Deserialize)]
pub struct SavingsQuery {
    /// First day (YYYY-MM-DD); default the first day of this month
    pub from: Option<NaiveDate>,
    /// Last day, included; default today
    pub to: Option<NaiveDate>,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct DeviceSavings {
    pub device_id: i32,
    pub device_name: String,
    pub hours_on: usize,
    /// Average €/kWh paid in those hours
    pub avg_price_paid: f64,
    /// Average €/kWh of the same days
    pub avg_price_reference: f64,
    /// Share cheaper than the reference, 0.25 = 25 %
    pub saving_ratio: f64,
    pub power_watts: Option<i32>,
    pub cost_eur: Option<f64>,
    pub saved_eur: Option<f64>,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct SavingsResponse {
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub hours_on: usize,
    pub saving_ratio: f64,
    /// Sum over the devices with a known power
    pub cost_eur: Option<f64>,
    pub saved_eur: Option<f64>,
    /// Devices whose power is not set, so they have no euros
    pub devices_without_power: usize,
    pub devices: Vec<DeviceSavings>,
}

/// One hour a device was on: (device id, device name, watts, hour)
type OnHour = (i32, String, Option<i32>, NaiveDateTime);

#[get("")]
pub async fn get_savings(
    pool: web::Data<DbPool>,
    claims: Claims,
    query: web::Query<SavingsQuery>,
) -> impl Responder {
    let Ok(user_id) = claims.sub.parse::<i32>() else {
        return HttpResponse::BadRequest().body("Invalid user ID");
    };
    let today = crate::time::today();
    let to = query.to.unwrap_or(today).min(today);
    let from = query.from.unwrap_or_else(|| today.with_day(1).unwrap_or(today));
    if from > to {
        return HttpResponse::BadRequest().body("from must not be after to");
    }
    let Ok(mut conn) = pool.get() else {
        return HttpResponse::InternalServerError().body("Database connection error");
    };

    let start = from.and_hms_opt(0, 0, 0).unwrap();
    let end = (to + chrono::Duration::days(1)).and_hms_opt(0, 0, 0).unwrap();
    let now = crate::time::now_naive();

    let on_hours: Vec<OnHour> = match scheduled_executions::table
        .inner_join(automation_rules::table.inner_join(devices::table))
        .filter(automation_rules::user_id.eq(user_id))
        .filter(scheduled_executions::expected_action.eq("turn_on"))
        .filter(scheduled_executions::status.eq("executed"))
        .filter(scheduled_executions::scheduled_hour.ge(start))
        .filter(scheduled_executions::scheduled_hour.lt(end.min(now)))
        .select((devices::id, devices::name, devices::power_watts, scheduled_executions::scheduled_hour))
        .load(&mut conn)
    {
        Ok(rows) => rows,
        Err(_) => return HttpResponse::InternalServerError().body("Error loading executions"),
    };

    let day_prices: Vec<(NaiveDateTime, f64)> = match prices::table
        .filter(prices::timestamp.ge(start))
        .filter(prices::timestamp.lt(end))
        .select((prices::timestamp, prices::price))
        .load(&mut conn)
    {
        Ok(rows) => rows,
        Err(_) => return HttpResponse::InternalServerError().body("Error loading prices"),
    };

    HttpResponse::Ok().json(compute_savings(from, to, on_hours, &day_prices))
}

fn round(value: f64, decimals: i32) -> f64 {
    let factor = 10f64.powi(decimals);
    (value * factor).round() / factor
}

fn compute_savings(
    from: NaiveDate,
    to: NaiveDate,
    on_hours: Vec<OnHour>,
    hourly_prices: &[(NaiveDateTime, f64)],
) -> SavingsResponse {
    let price_at: HashMap<NaiveDateTime, f64> = hourly_prices
        .iter()
        .map(|(t, p)| (t.with_minute(0).unwrap().with_second(0).unwrap(), *p))
        .collect();
    let mut per_day: HashMap<NaiveDate, (f64, usize)> = HashMap::new();
    for (t, p) in hourly_prices {
        let e = per_day.entry(t.date()).or_default();
        e.0 += p;
        e.1 += 1;
    }
    let day_avg = |d: NaiveDate| per_day.get(&d).map(|(sum, n)| sum / *n as f64);

    // Two rules on the same device and hour count once
    let mut by_device: BTreeMap<i32, (String, Option<i32>, BTreeSet<NaiveDateTime>)> = BTreeMap::new();
    for (id, name, watts, hour) in on_hours {
        by_device.entry(id).or_insert_with(|| (name, watts, BTreeSet::new())).2.insert(hour);
    }

    let mut devices = Vec::new();
    let (mut all_paid, mut all_reference, mut all_hours) = (0.0, 0.0, 0);
    let (mut cost, mut saved, mut with_power, mut without_power) = (0.0, 0.0, 0, 0);
    for (device_id, (device_name, watts, hours)) in by_device {
        // Hours without a price (should not happen) are left out
        let priced: Vec<(f64, f64)> = hours
            .iter()
            .filter_map(|h| Some((*price_at.get(h)?, day_avg(h.date())?)))
            .collect();
        if priced.is_empty() {
            continue;
        }
        let paid: f64 = priced.iter().map(|(p, _)| p).sum();
        let reference: f64 = priced.iter().map(|(_, r)| r).sum();
        let n = priced.len();
        let ratio = if reference > 0.0 { (reference - paid) / reference } else { 0.0 };
        let kw = watts.map(|w| w as f64 / 1000.0);
        let (device_cost, device_saved) = match kw {
            Some(kw) => {
                with_power += 1;
                cost += kw * paid;
                saved += kw * (reference - paid);
                (Some(round(kw * paid, 2)), Some(round(kw * (reference - paid), 2)))
            }
            None => {
                without_power += 1;
                (None, None)
            }
        };
        all_paid += paid;
        all_reference += reference;
        all_hours += n;
        devices.push(DeviceSavings {
            device_id,
            device_name,
            hours_on: n,
            avg_price_paid: round(paid / n as f64, 4),
            avg_price_reference: round(reference / n as f64, 4),
            saving_ratio: round(ratio, 3),
            power_watts: watts,
            cost_eur: device_cost,
            saved_eur: device_saved,
        });
    }

    SavingsResponse {
        from,
        to,
        hours_on: all_hours,
        saving_ratio: if all_reference > 0.0 { round((all_reference - all_paid) / all_reference, 3) } else { 0.0 },
        cost_eur: (with_power > 0).then(|| round(cost, 2)),
        saved_eur: (with_power > 0).then(|| round(saved, 2)),
        devices_without_power: without_power,
        devices,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(day: u32, hour: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, day).unwrap().and_hms_opt(hour, 0, 0).unwrap()
    }

    /// Day 1: hours 0-3 at 0.10, 0.20, 0.30, 0.40 (average 0.25)
    fn prices() -> Vec<(NaiveDateTime, f64)> {
        vec![(h(1, 0), 0.10), (h(1, 1), 0.20), (h(1, 2), 0.30), (h(1, 3), 0.40)]
    }

    fn day() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 1).unwrap()
    }

    #[test]
    fn test_cheap_hours_against_day_average() {
        let on = vec![(1, "Termo".to_string(), Some(2000), h(1, 0)), (1, "Termo".to_string(), Some(2000), h(1, 1))];
        let s = compute_savings(day(), day(), on, &prices());
        // Paid 0.10 + 0.20 = 0.30 for 2 h; the average would be 0.50
        assert_eq!(s.hours_on, 2);
        assert_eq!(s.saving_ratio, 0.4);
        assert_eq!(s.cost_eur, Some(0.6)); // 2 kW × 0.30
        assert_eq!(s.saved_eur, Some(0.4)); // 2 kW × 0.20
        assert_eq!(s.devices[0].avg_price_paid, 0.15);
        assert_eq!(s.devices[0].avg_price_reference, 0.25);
    }

    #[test]
    fn test_device_without_power_has_ratio_but_no_euros() {
        let on = vec![(5, "Any name".to_string(), None, h(1, 0))];
        let s = compute_savings(day(), day(), on, &prices());
        assert_eq!(s.saving_ratio, 0.6);
        assert_eq!(s.cost_eur, None);
        assert_eq!(s.saved_eur, None);
        assert_eq!(s.devices_without_power, 1);
    }

    #[test]
    fn test_same_hour_from_two_rules_counts_once() {
        let on = vec![(1, "A".to_string(), Some(1000), h(1, 0)), (1, "A".to_string(), Some(1000), h(1, 0))];
        assert_eq!(compute_savings(day(), day(), on, &prices()).hours_on, 1);
    }

    #[test]
    fn test_expensive_hours_give_negative_saving() {
        let on = vec![(1, "A".to_string(), Some(1000), h(1, 3))];
        let s = compute_savings(day(), day(), on, &prices());
        assert_eq!(s.saving_ratio, -0.6);
        assert_eq!(s.saved_eur, Some(-0.15));
    }

    #[test]
    fn test_no_hours() {
        let s = compute_savings(day(), day(), vec![], &prices());
        assert_eq!(s.hours_on, 0);
        assert_eq!(s.saving_ratio, 0.0);
        assert!(s.devices.is_empty());
    }
}
