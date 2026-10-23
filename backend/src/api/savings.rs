//! How much the rules saved: the price paid in the hours a rule switched a
//! device on, against the average price of those days (what running at any
//! hour would have cost). The energy of each hour is the power the plug
//! measured in it, or else the power the user set; without either, no euros.

use crate::{
    db::DbPool,
    schema::{automation_rules, devices, power_readings, scheduled_executions},
    services::{auth::Claims, tariff::Tariff},
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
    /// Hours whose energy comes from the plug's own readings
    pub measured_hours: usize,
    /// kWh used in the hours on; None without power
    pub energy_kwh: Option<f64>,
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
    /// Devices with no power set or measured, so they have no euros
    pub devices_without_power: usize,
    /// Hours whose energy comes from the plugs' own readings
    pub measured_hours: usize,
    pub devices: Vec<DeviceSavings>,
}

/// One hour a device was on: (device id, device name, watts, hour)
type OnHour = (i32, String, Option<i32>, NaiveDateTime);

/// A power reading: (device id, when, watts)
type Reading = (i32, NaiveDateTime, f32);

#[get("")]
pub async fn get_savings(
    pool: web::Data<DbPool>,
    claims: Claims,
    query: web::Query<SavingsQuery>,
) -> impl Responder {
    let Ok(user_id) = claims.sub.parse::<i32>() else {
        return HttpResponse::BadRequest().body("Invalid user ID");
    };
    let Ok(mut conn) = pool.get() else {
        return HttpResponse::InternalServerError().body("Database connection error");
    };
    // Days are the user's local days; the database is in engine time
    let tariff = Tariff::of_user(&mut conn, user_id);
    let zone = tariff.zone;
    let today = zone.now().date();
    let to = query.to.unwrap_or(today).min(today);
    let from = query.from.unwrap_or_else(|| today.with_day(1).unwrap_or(today));
    if from > to {
        return HttpResponse::BadRequest().body("from must not be after to");
    }

    let (start, _) = tariff.local_day(from);
    let (_, end) = tariff.local_day(to);
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

    let day_prices: Vec<(NaiveDateTime, f64)> = match tariff.hourly(&mut conn, start, end) {
        Ok(rows) => rows.into_iter().map(|p| (zone.to_local(p.hour), p.price)).collect(),
        Err(_) => return HttpResponse::InternalServerError().body("Error loading prices"),
    };
    let on_hours: Vec<OnHour> =
        on_hours.into_iter().map(|(id, name, watts, hour)| (id, name, watts, zone.to_local(hour))).collect();

    let device_ids: Vec<i32> = on_hours.iter().map(|(id, ..)| *id).collect::<BTreeSet<_>>().into_iter().collect();
    let readings: Vec<Reading> = match power_readings::table
        .filter(power_readings::device_id.eq_any(&device_ids))
        .filter(power_readings::measured_at.ge(start))
        .filter(power_readings::measured_at.lt(end))
        .select((power_readings::device_id, power_readings::measured_at, power_readings::watts))
        .load(&mut conn)
    {
        Ok(rows) => rows.into_iter().map(|(id, at, watts)| (id, zone.to_local(at), watts)).collect(),
        Err(_) => return HttpResponse::InternalServerError().body("Error loading power readings"),
    };

    HttpResponse::Ok().json(compute_savings(from, to, on_hours, &day_prices, &readings))
}

fn start_of_hour(t: NaiveDateTime) -> NaiveDateTime {
    t.with_minute(0).and_then(|t| t.with_second(0)).and_then(|t| t.with_nanosecond(0)).unwrap_or(t)
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
    readings: &[Reading],
) -> SavingsResponse {
    // Average measured kW of each device and hour
    let mut per_hour: HashMap<(i32, NaiveDateTime), (f64, usize)> = HashMap::new();
    for (id, at, watts) in readings {
        let e = per_hour.entry((*id, start_of_hour(*at))).or_default();
        e.0 += *watts as f64;
        e.1 += 1;
    }
    let measured_kw = |id: i32, h: NaiveDateTime| per_hour.get(&(id, h)).map(|(sum, n)| sum / *n as f64 / 1000.0);

    let price_at: HashMap<NaiveDateTime, f64> = hourly_prices
        .iter()
        .map(|(t, p)| (start_of_hour(*t), *p))
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
    let (mut all_paid, mut all_reference, mut all_hours, mut all_measured) = (0.0, 0.0, 0, 0);
    let (mut cost, mut saved, mut with_power, mut without_power) = (0.0, 0.0, 0, 0);
    for (device_id, (device_name, watts, hours)) in by_device {
        // (price, day average, measured kW); hours without a price (should not happen) are left out
        let priced: Vec<(f64, f64, Option<f64>)> = hours
            .iter()
            .filter_map(|h| Some((*price_at.get(h)?, day_avg(h.date())?, measured_kw(device_id, *h))))
            .collect();
        if priced.is_empty() {
            continue;
        }
        let paid: f64 = priced.iter().map(|(p, ..)| p).sum();
        let reference: f64 = priced.iter().map(|(_, r, _)| r).sum();
        let n = priced.len();
        let ratio = if reference > 0.0 { (reference - paid) / reference } else { 0.0 };

        // Hours not measured use the power the user set, or else the
        // device's average measured power
        let measured: Vec<f64> = priced.iter().filter_map(|(.., kw)| *kw).collect();
        let fallback_kw = watts
            .map(|w| w as f64 / 1000.0)
            .or_else(|| (!measured.is_empty()).then(|| measured.iter().sum::<f64>() / measured.len() as f64));
        let (energy, device_cost, device_saved) = match fallback_kw {
            Some(fallback) => {
                with_power += 1;
                let (mut kwh, mut c, mut sv) = (0.0, 0.0, 0.0);
                for (price, avg, kw) in &priced {
                    let kw = kw.unwrap_or(fallback);
                    kwh += kw;
                    c += kw * price;
                    sv += kw * (avg - price);
                }
                cost += c;
                saved += sv;
                (Some(round(kwh, 2)), Some(round(c, 2)), Some(round(sv, 2)))
            }
            None => {
                without_power += 1;
                (None, None, None)
            }
        };
        all_paid += paid;
        all_reference += reference;
        all_hours += n;
        all_measured += measured.len();
        devices.push(DeviceSavings {
            device_id,
            device_name,
            hours_on: n,
            avg_price_paid: round(paid / n as f64, 4),
            avg_price_reference: round(reference / n as f64, 4),
            saving_ratio: round(ratio, 3),
            power_watts: watts,
            measured_hours: measured.len(),
            energy_kwh: energy,
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
        measured_hours: all_measured,
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
        let s = compute_savings(day(), day(), on, &prices(), &[]);
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
        let s = compute_savings(day(), day(), on, &prices(), &[]);
        assert_eq!(s.saving_ratio, 0.6);
        assert_eq!(s.cost_eur, None);
        assert_eq!(s.saved_eur, None);
        assert_eq!(s.devices_without_power, 1);
    }

    #[test]
    fn test_same_hour_from_two_rules_counts_once() {
        let on = vec![(1, "A".to_string(), Some(1000), h(1, 0)), (1, "A".to_string(), Some(1000), h(1, 0))];
        assert_eq!(compute_savings(day(), day(), on, &prices(), &[]).hours_on, 1);
    }

    #[test]
    fn test_expensive_hours_give_negative_saving() {
        let on = vec![(1, "A".to_string(), Some(1000), h(1, 3))];
        let s = compute_savings(day(), day(), on, &prices(), &[]);
        assert_eq!(s.saving_ratio, -0.6);
        assert_eq!(s.saved_eur, Some(-0.15));
    }

    #[test]
    fn test_measured_power_replaces_the_typed_one() {
        let on = vec![(1, "A".to_string(), Some(2000), h(1, 0)), (1, "A".to_string(), Some(2000), h(1, 1))];
        // Hour 0 measured at 500 W on average; hour 1 not measured, uses 2000 W
        let readings = vec![
            (1, h(1, 0) + chrono::Duration::minutes(2), 400.0),
            (1, h(1, 0) + chrono::Duration::minutes(7), 600.0),
        ];
        let s = compute_savings(day(), day(), on, &prices(), &readings);
        assert_eq!(s.measured_hours, 1);
        assert_eq!(s.devices[0].energy_kwh, Some(2.5));
        assert_eq!(s.cost_eur, Some(0.45)); // 0.5 × 0.10 + 2 × 0.20
        assert!((s.saved_eur.unwrap() - 0.175).abs() < 0.01); // 0.5 × 0.15 + 2 × 0.05
    }

    #[test]
    fn test_measured_device_without_typed_power_has_euros() {
        let on = vec![(1, "A".to_string(), None, h(1, 0)), (1, "A".to_string(), None, h(1, 1))];
        // Only hour 0 measured (1000 W); hour 1 takes the device's average
        let readings = vec![(1, h(1, 0) + chrono::Duration::minutes(2), 1000.0)];
        let s = compute_savings(day(), day(), on, &prices(), &readings);
        assert_eq!(s.devices_without_power, 0);
        assert_eq!(s.devices[0].energy_kwh, Some(2.0));
        assert_eq!(s.cost_eur, Some(0.3));
    }

    #[test]
    fn test_readings_of_other_devices_or_hours_are_ignored() {
        let on = vec![(1, "A".to_string(), None, h(1, 0))];
        let readings = vec![(2, h(1, 0), 1000.0), (1, h(1, 3), 1000.0)];
        let s = compute_savings(day(), day(), on, &prices(), &readings);
        assert_eq!(s.measured_hours, 0);
        assert_eq!(s.cost_eur, None);
    }

    #[test]
    fn test_no_hours() {
        let s = compute_savings(day(), day(), vec![], &prices(), &[]);
        assert_eq!(s.hours_on, 0);
        assert_eq!(s.saving_ratio, 0.0);
        assert!(s.devices.is_empty());
    }
}
