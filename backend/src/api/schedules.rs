use crate::{
    db::DbPool,
    models::ScheduledExecution,
    schema::{automation_rules, devices, scheduled_executions},
    services::{auth::Claims, price_fetcher::PriceService, schedule_computation::ScheduleComputationService},
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

/// Get scheduled actions for a specific date from scheduled_executions table
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

    // Ensure schedules are computed for this date (in case they're missing)
    let schedule_service = ScheduleComputationService::new(pool.get_ref().clone());
    let _ = schedule_service.compute_schedule_for_date(date);

    // Mark any past hours as missed (important for displaying correct status)
    let _ = schedule_service.mark_missed_hours();

    // Get scheduled executions for this user's rules on the given date
    let start_of_day = date.and_hms_opt(0, 0, 0).unwrap();
    let end_of_day = date.and_hms_opt(23, 59, 59).unwrap();

    // Join scheduled_executions with automation_rules and devices to get all needed info
    let executions: Vec<(ScheduledExecution, String, String, i32, String)> = match scheduled_executions::table
        .inner_join(automation_rules::table.on(
            scheduled_executions::rule_id.eq(automation_rules::id)
        ))
        .inner_join(devices::table.on(
            automation_rules::device_id.eq(devices::id)
        ))
        .filter(automation_rules::user_id.eq(user_id))
        .filter(scheduled_executions::scheduled_hour.ge(start_of_day))
        .filter(scheduled_executions::scheduled_hour.le(end_of_day))
        .select((
            ScheduledExecution::as_select(),
            automation_rules::name,
            devices::name,
            devices::id,
            automation_rules::action,
        ))
        .load(&mut conn)
    {
        Ok(e) => e,
        Err(e) => {
            log::error!("Error fetching scheduled executions: {}", e);
            return HttpResponse::InternalServerError().body("Error fetching schedule");
        }
    };

    // Get prices for the date for price info
    let price_service = PriceService::new(pool.get_ref().clone());
    let prices = price_service.get_prices_for_date(date).unwrap_or_default();

    let scheduled_hours: Vec<ScheduledHour> = executions
        .into_iter()
        .map(|(exec, rule_name, device_name, device_id, action)| {
            let hour = exec.scheduled_hour.hour();

            // Find price for this hour
            let price_at_hour = prices.iter()
                .find(|p| p.timestamp.hour() == hour)
                .map(|p| p.price);

            // Map database status to API status
            let status = match exec.status.as_str() {
                "executed" => {
                    if action == "turn_on" {
                        "completed_on"
                    } else {
                        "completed_off"
                    }
                }
                "pending" => "pending",
                "retrying" => "retrying",
                "failed" => "failed",
                "missed" => "missed",
                _ => "pending",
            };

            ScheduledHour {
                hour,
                device_id,
                device_name,
                rule_id: exec.rule_id,
                rule_name,
                action,
                status: status.to_string(),
                price: price_at_hour,
                price_formatted: price_at_hour.map(|p| format!("{:.4} €/kWh", p)),
            }
        })
        .collect();

    // Sort by hour
    let mut sorted_hours = scheduled_hours;
    sorted_hours.sort_by_key(|s| s.hour);

    HttpResponse::Ok().json(ScheduleResponse {
        date: date.to_string(),
        scheduled_hours: sorted_hours,
    })
}
