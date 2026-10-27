use crate::{
    db::DbPool,
    schema::prices,
    services::{
        auth::Claims,
        price_fetcher::PriceService,
        tariff::{self, HourPrice, Tariff},
    },
};
use actix_web::{HttpResponse, Responder, get, post, web};
use chrono::{NaiveDate, Timelike};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

// ============================================================================
// Request/Response DTOs
// ============================================================================

#[derive(Deserialize)]
pub struct DateQuery {
    /// Local date in the zone (YYYY-MM-DD); today when missing
    pub date: Option<String>,
    /// crate::zones code; the logged-in user's tariff, or the Peninsula, when missing
    pub zone: Option<String>,
}

#[derive(Deserialize)]
pub struct CheapestHoursQuery {
    pub date: Option<String>,
    pub count: Option<usize>,
    pub zone: Option<String>,
}

#[derive(Deserialize)]
pub struct ZoneQuery {
    pub zone: Option<String>,
}

/// The prices to show: the zone asked for (its PVPC in Spain, its market
/// price elsewhere), else the logged-in user's tariff, else the Peninsula's
/// PVPC. An error answer when the zone is not one we serve.
fn tariff_param(
    pool: &DbPool,
    claims: Option<Claims>,
    zone: &Option<String>,
) -> Result<Tariff, Box<HttpResponse>> {
    match zone.as_deref() {
        Some(z) => crate::zones::find(z)
            .map(Tariff::default_for)
            .ok_or_else(|| Box::new(HttpResponse::BadRequest().body("Unknown zone"))),
        None => Ok(claims
            .and_then(|c| c.sub.parse::<i32>().ok())
            .and_then(|user_id| {
                let mut conn = pool.get().ok()?;
                Some(Tariff::of_user(&mut conn, user_id))
            })
            .unwrap_or_else(|| Tariff::default_for(crate::zones::get(crate::zones::DEFAULT)))),
    }
}

/// The local date asked for, or today in the tariff's zone
fn date_param(date: &Option<String>, tariff: &Tariff) -> Result<NaiveDate, Box<HttpResponse>> {
    match date {
        Some(d) => NaiveDate::parse_from_str(d, "%Y-%m-%d").map_err(|_| {
            Box::new(HttpResponse::BadRequest().body("Invalid date format. Use YYYY-MM-DD"))
        }),
        None => Ok(tariff.zone.now().date()),
    }
}

/// Prices of a local day, with local times
fn day_prices(
    pool: &DbPool,
    tariff: &Tariff,
    date: NaiveDate,
) -> Result<Vec<PriceResponse>, Box<HttpResponse>> {
    let mut conn = pool.get().map_err(|_| {
        Box::new(HttpResponse::InternalServerError().body("Database connection error"))
    })?;
    tariff
        .day(&mut conn, date)
        .map(|prices| {
            prices
                .into_iter()
                .map(|p| PriceResponse::local(tariff, p))
                .collect()
        })
        .map_err(|e| Box::new(HttpResponse::InternalServerError().body(e.to_string())))
}

#[derive(Serialize)]
pub struct PriceResponse {
    /// Local time in the zone
    pub timestamp: String,
    pub hour: u32,
    pub price: f64,
    pub price_formatted: String,
}

impl PriceResponse {
    fn local(tariff: &Tariff, p: HourPrice) -> Self {
        let local = tariff.zone.to_local(p.hour);
        Self {
            timestamp: local.to_string(),
            hour: local.hour(),
            price: p.price,
            price_formatted: format!("{:.4} €/kWh", p.price),
        }
    }
}

#[derive(Serialize)]
pub struct PriceSummary {
    pub date: String,
    pub min_price: f64,
    pub max_price: f64,
    pub avg_price: f64,
    pub cheapest_hour: u32,
    pub most_expensive_hour: u32,
}

#[derive(Serialize)]
pub struct ZoneInfo {
    pub code: &'static str,
    pub name: &'static str,
    pub country: &'static str,
    /// Tariffs a user of the zone can choose (crate::services::tariff::Kind)
    pub tariffs: Vec<&'static str>,
    /// Whether the server has today's market prices for it
    pub available: bool,
}

#[derive(Serialize)]
pub struct SyncResponse {
    pub success: bool,
    pub prices_synced: usize,
    pub message: String,
}

// ============================================================================
// Endpoints
// ============================================================================

/// Prices of a local day (defaults to today)
#[get("")]
pub async fn get_prices(
    pool: web::Data<DbPool>,
    claims: Option<Claims>,
    query: web::Query<DateQuery>,
) -> impl Responder {
    let result = tariff_param(&pool, claims, &query.zone).and_then(|tariff| {
        let date = date_param(&query.date, &tariff)?;
        day_prices(&pool, &tariff, date)
    });
    match result {
        Ok(prices) => HttpResponse::Ok().json(prices),
        Err(response) => *response,
    }
}

/// Price of the current hour
#[get("/current")]
pub async fn get_current_price(
    pool: web::Data<DbPool>,
    claims: Option<Claims>,
    query: web::Query<ZoneQuery>,
) -> impl Responder {
    let tariff = match tariff_param(&pool, claims, &query.zone) {
        Ok(t) => t,
        Err(response) => return *response,
    };
    let Ok(mut conn) = pool.get() else {
        return HttpResponse::InternalServerError().body("Database connection error");
    };
    let hour = crate::time::current_hour();
    match tariff.at(&mut conn, hour) {
        Ok(Some(price)) => {
            HttpResponse::Ok().json(PriceResponse::local(&tariff, HourPrice { hour, price }))
        }
        Ok(None) => HttpResponse::NotFound().json(serde_json::json!({
            "error": "No price available for current hour",
            "hint": "Prices may need to be synced. Call POST /api/prices/sync"
        })),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

/// Summary statistics of a local day
#[get("/summary")]
pub async fn get_price_summary(
    pool: web::Data<DbPool>,
    claims: Option<Claims>,
    query: web::Query<DateQuery>,
) -> impl Responder {
    let result = tariff_param(&pool, claims, &query.zone).and_then(|tariff| {
        let date = date_param(&query.date, &tariff)?;
        Ok((date, day_prices(&pool, &tariff, date)?))
    });
    let (date, prices) = match result {
        Ok(r) => r,
        Err(response) => return *response,
    };
    let (Some(cheapest), Some(most_expensive)) = (
        prices.iter().min_by(|a, b| a.price.total_cmp(&b.price)),
        prices.iter().max_by(|a, b| a.price.total_cmp(&b.price)),
    ) else {
        return HttpResponse::NotFound().json(serde_json::json!({
            "error": "No prices available for this date",
            "date": date.to_string()
        }));
    };
    HttpResponse::Ok().json(PriceSummary {
        date: date.to_string(),
        min_price: cheapest.price,
        max_price: most_expensive.price,
        avg_price: prices.iter().map(|p| p.price).sum::<f64>() / prices.len() as f64,
        cheapest_hour: cheapest.hour,
        most_expensive_hour: most_expensive.hour,
    })
}

/// The N cheapest (or most expensive) hours of a local day
fn ranked_hours(
    pool: &DbPool,
    claims: Option<Claims>,
    query: &CheapestHoursQuery,
    cheapest_first: bool,
) -> HttpResponse {
    let result = tariff_param(pool, claims, &query.zone).and_then(|tariff| {
        let date = date_param(&query.date, &tariff)?;
        day_prices(pool, &tariff, date)
    });
    match result {
        Ok(mut prices) => {
            prices.sort_by(|a, b| {
                if cheapest_first {
                    a.price.total_cmp(&b.price)
                } else {
                    b.price.total_cmp(&a.price)
                }
            });
            prices.truncate(query.count.unwrap_or(6).min(24));
            HttpResponse::Ok().json(prices)
        }
        Err(response) => *response,
    }
}

/// Get the N cheapest hours for a date
#[get("/cheapest")]
pub async fn get_cheapest_hours(
    pool: web::Data<DbPool>,
    claims: Option<Claims>,
    query: web::Query<CheapestHoursQuery>,
) -> impl Responder {
    ranked_hours(&pool, claims, &query, true)
}

/// Get the N most expensive hours for a date
#[get("/expensive")]
pub async fn get_expensive_hours(
    pool: web::Data<DbPool>,
    claims: Option<Claims>,
    query: web::Query<CheapestHoursQuery>,
) -> impl Responder {
    ranked_hours(&pool, claims, &query, false)
}

/// The zones served, with the tariffs each one offers
#[get("/zones")]
pub async fn list_zones(pool: web::Data<DbPool>) -> impl Responder {
    let Ok(mut conn) = pool.get() else {
        return HttpResponse::InternalServerError().body("Database connection error");
    };
    let today = crate::time::today().and_hms_opt(0, 0, 0).unwrap();
    let with_prices: Vec<String> = prices::table
        .filter(prices::timestamp.ge(today))
        .select(prices::series)
        .distinct()
        .load(&mut conn)
        .unwrap_or_default();
    let zones: Vec<ZoneInfo> = crate::zones::ZONES
        .iter()
        .map(|zone| ZoneInfo {
            code: zone.code,
            name: zone.name,
            country: zone.country,
            tariffs: if zone.is_spain() {
                vec!["pvpc", "indexed"]
            } else {
                vec!["indexed"]
            },
            available: with_prices.contains(&tariff::spot_series(zone)),
        })
        .collect();
    HttpResponse::Ok().json(zones)
}

/// Sync prices from ESIOS API (today and tomorrow if available)
#[post("/sync")]
pub async fn sync_prices(
    pool: web::Data<DbPool>,
    _claims: Claims, // Requires authentication: each call hits the ESIOS API
) -> impl Responder {
    let service = PriceService::new(pool.get_ref().clone());

    // Sync today's prices
    let today_result = service.sync_today().await;
    let today_count = today_result.as_ref().map(|&c| c).unwrap_or(0);
    let today_ok = today_result.is_ok();

    // Try to sync tomorrow's prices (available after 20:00)
    let tomorrow_result = service.sync_tomorrow().await;
    let tomorrow_count = tomorrow_result.as_ref().map(|&c| c).unwrap_or(0);
    let tomorrow_ok = tomorrow_result.is_ok();

    let total_synced = today_count + tomorrow_count;

    let message = match (today_ok, tomorrow_ok) {
        (true, true) => "Synced prices for today and tomorrow".to_string(),
        (true, false) => "Synced today's prices. Tomorrow not available yet.".to_string(),
        (false, true) => "Failed to sync today. Synced tomorrow.".to_string(),
        (false, false) => "Failed to sync prices. Check ESIOS_TOKEN configuration.".to_string(),
    };

    let success = today_ok || tomorrow_ok;

    HttpResponse::Ok().json(SyncResponse {
        success,
        prices_synced: total_synced,
        message,
    })
}

/// Sync prices for a specific date
#[post("/sync/{date}")]
pub async fn sync_prices_for_date(
    pool: web::Data<DbPool>,
    _claims: Claims, // Requires authentication: each call hits the ESIOS API
    path: web::Path<String>,
) -> impl Responder {
    let date_str = path.into_inner();
    let date = match NaiveDate::parse_from_str(&date_str, "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => return HttpResponse::BadRequest().body("Invalid date format. Use YYYY-MM-DD"),
    };

    let service = PriceService::new(pool.get_ref().clone());

    match service.sync_prices_for_date(date).await {
        Ok(count) => HttpResponse::Ok().json(SyncResponse {
            success: true,
            prices_synced: count,
            message: format!("Synced {} prices for {}", count, date),
        }),
        Err(e) => HttpResponse::InternalServerError().json(SyncResponse {
            success: false,
            prices_synced: 0,
            message: e.to_string(),
        }),
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_date_query_parsing() {
        let json = r#"{"date": "2024-01-15"}"#;
        let query: DateQuery = serde_json::from_str(json).unwrap();
        assert_eq!(query.date, Some("2024-01-15".to_string()));
    }

    #[test]
    fn test_cheapest_hours_query_parsing() {
        let json = r#"{"date": "2024-01-15", "count": 3}"#;
        let query: CheapestHoursQuery = serde_json::from_str(json).unwrap();
        assert_eq!(query.date, Some("2024-01-15".to_string()));
        assert_eq!(query.count, Some(3));
    }

    #[test]
    fn test_price_response_serialization() {
        let response = PriceResponse {
            timestamp: "2024-01-15 10:00:00".to_string(),
            hour: 10,
            price: 0.15,
            price_formatted: "0.1500 €/kWh".to_string(),
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("0.15"));
        assert!(json.contains("10"));
    }

    #[test]
    fn test_sync_response_serialization() {
        let response = SyncResponse {
            success: true,
            prices_synced: 24,
            message: "All good".to_string(),
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("true"));
        assert!(json.contains("24"));
    }
}
