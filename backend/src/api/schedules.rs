use crate::{
    db::DbPool,
    models::AutomationRule,
    schema::{automation_rules, devices},
    services::{auth::Claims, price_fetcher::PriceService},
};
use actix_web::{get, web, HttpResponse, Responder};
use chrono::{Local, NaiveDate, Timelike};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

// ============================================================================
// Request/Response DTOs
// ============================================================================

#[derive(Deserialize)]
pub struct ScheduleQuery {
    pub date: Option<String>, // Format: YYYY-MM-DD
}

#[derive(Serialize)]
pub struct ScheduledHour {
    pub hour: u32,
    pub device_id: i32,
    pub device_name: String,
    pub rule_id: i32,
    pub rule_name: String,
    pub action: String,
    pub status: String, // "pending", "completed_on", "completed_off", "failed"
    pub price: Option<f64>,
    pub price_formatted: Option<String>,
}

#[derive(Serialize)]
pub struct ScheduleResponse {
    pub date: String,
    pub scheduled_hours: Vec<ScheduledHour>,
}

// ============================================================================
// Endpoints
// ============================================================================

/// Get scheduled actions for a specific date based on active rules
#[get("")]
pub async fn get_schedule(
    pool: web::Data<DbPool>,
    claims: Claims,
    query: web::Query<ScheduleQuery>,
) -> impl Responder {
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Database connection error"),
    };

    let user_id: i32 = match claims.sub.parse() {
        Ok(id) => id,
        Err(_) => return HttpResponse::BadRequest().body("Invalid user ID"),
    };

    let date = match &query.date {
        Some(d) => match NaiveDate::parse_from_str(d, "%Y-%m-%d") {
            Ok(date) => date,
            Err(_) => return HttpResponse::BadRequest().body("Invalid date format. Use YYYY-MM-DD"),
        },
        None => Local::now().date_naive(),
    };

    // Get active rules for this user with device names
    let rules_with_devices: Vec<(AutomationRule, String)> = match automation_rules::table
        .inner_join(devices::table)
        .filter(automation_rules::user_id.eq(user_id))
        .filter(automation_rules::is_enabled.eq(true))
        .select((AutomationRule::as_select(), devices::name))
        .load(&mut conn)
    {
        Ok(r) => r,
        Err(_) => return HttpResponse::InternalServerError().body("Error fetching rules"),
    };

    // Get prices for the date
    let price_service = PriceService::new(pool.get_ref().clone());
    let prices = price_service.get_prices_for_date(date).unwrap_or_default();

    let mut scheduled_hours: Vec<ScheduledHour> = Vec::new();
    let current_hour = if date == Local::now().date_naive() {
        Local::now().hour()
    } else if date < Local::now().date_naive() {
        24 // All hours are in the past
    } else {
        0 // All hours are in the future
    };

    for (rule, device_name) in rules_with_devices {
        // Parse the config
        let config = &rule.config;

        match rule.rule_type.as_str() {
            "cheapest_hours" => {
                let hours_needed = config.get("cheapest_hours")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(6) as usize;

                // Get the cheapest N hours
                let cheapest = price_service.get_cheapest_hours(date, hours_needed).unwrap_or_default();

                for price in cheapest {
                    let hour = price.timestamp.hour();
                    let status = if date < Local::now().date_naive() || hour < current_hour {
                        if rule.action == "turn_on" {
                            "completed_on"
                        } else {
                            "completed_off"
                        }
                    } else {
                        "pending"
                    };

                    scheduled_hours.push(ScheduledHour {
                        hour,
                        device_id: rule.device_id,
                        device_name: device_name.clone(),
                        rule_id: rule.id,
                        rule_name: rule.name.clone(),
                        action: rule.action.clone(),
                        status: status.to_string(),
                        price: Some(price.price),
                        price_formatted: Some(format!("{:.4} €/kWh", price.price)),
                    });
                }
            }
            "price_threshold" => {
                let threshold = config.get("price_threshold")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.10);

                // Get all hours below threshold
                for price in &prices {
                    if price.price <= threshold {
                        let hour = price.timestamp.hour();
                        let status = if date < Local::now().date_naive() || hour < current_hour {
                            if rule.action == "turn_on" {
                                "completed_on"
                            } else {
                                "completed_off"
                            }
                        } else {
                            "pending"
                        };

                        scheduled_hours.push(ScheduledHour {
                            hour,
                            device_id: rule.device_id,
                            device_name: device_name.clone(),
                            rule_id: rule.id,
                            rule_name: rule.name.clone(),
                            action: rule.action.clone(),
                            status: status.to_string(),
                            price: Some(price.price),
                            price_formatted: Some(format!("{:.4} €/kWh", price.price)),
                        });
                    }
                }
            }
            "time_schedule" => {
                let start_str = config.get("time_range_start")
                    .and_then(|v| v.as_str())
                    .unwrap_or("00:00");
                let end_str = config.get("time_range_end")
                    .and_then(|v| v.as_str())
                    .unwrap_or("23:59");

                let start_hour: u32 = start_str.split(':').next()
                    .and_then(|h| h.parse().ok())
                    .unwrap_or(0);
                let end_hour: u32 = end_str.split(':').next()
                    .and_then(|h| h.parse().ok())
                    .unwrap_or(23);

                // Handle overnight ranges (e.g., 22:00 to 08:00)
                let hours_in_range: Vec<u32> = if start_hour <= end_hour {
                    (start_hour..=end_hour).collect()
                } else {
                    (start_hour..24).chain(0..=end_hour).collect()
                };

                for hour in hours_in_range {
                    let price_at_hour = prices.iter()
                        .find(|p| p.timestamp.hour() == hour)
                        .map(|p| p.price);

                    let status = if date < Local::now().date_naive() || hour < current_hour {
                        if rule.action == "turn_on" {
                            "completed_on"
                        } else {
                            "completed_off"
                        }
                    } else {
                        "pending"
                    };

                    scheduled_hours.push(ScheduledHour {
                        hour,
                        device_id: rule.device_id,
                        device_name: device_name.clone(),
                        rule_id: rule.id,
                        rule_name: rule.name.clone(),
                        action: rule.action.clone(),
                        status: status.to_string(),
                        price: price_at_hour,
                        price_formatted: price_at_hour.map(|p| format!("{:.4} €/kWh", p)),
                    });
                }
            }
            // "manual" rules don't generate scheduled hours
            _ => {}
        }
    }

    // Sort by hour
    scheduled_hours.sort_by_key(|s| s.hour);

    HttpResponse::Ok().json(ScheduleResponse {
        date: date.to_string(),
        scheduled_hours,
    })
}
