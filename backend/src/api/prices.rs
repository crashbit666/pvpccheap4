use crate::{
    db::DbPool,
    services::price_fetcher::PriceService,
};
use actix_web::{get, post, web, HttpResponse, Responder};
use chrono::{Local, NaiveDate};
use serde::{Deserialize, Serialize};

// ============================================================================
// Request/Response DTOs
// ============================================================================

#[derive(Deserialize)]
pub struct DateQuery {
    pub date: Option<String>, // Format: YYYY-MM-DD
}

#[derive(Deserialize)]
pub struct CheapestHoursQuery {
    pub date: Option<String>,
    pub count: Option<usize>,
}

#[derive(Serialize)]
pub struct PriceResponse {
    pub timestamp: String,
    pub hour: u32,
    pub price: f64,
    pub price_formatted: String,
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
pub struct SyncResponse {
    pub success: bool,
    pub prices_synced: usize,
    pub message: String,
}

// ============================================================================
// Endpoints
// ============================================================================

/// Get prices for a specific date (defaults to today)
#[get("")]
pub async fn get_prices(pool: web::Data<DbPool>, query: web::Query<DateQuery>) -> impl Responder {
    let service = PriceService::new(pool.get_ref().clone());

    let date = match &query.date {
        Some(d) => match NaiveDate::parse_from_str(d, "%Y-%m-%d") {
            Ok(date) => date,
            Err(_) => return HttpResponse::BadRequest().body("Invalid date format. Use YYYY-MM-DD"),
        },
        None => Local::now().date_naive(),
    };

    match service.get_prices_for_date(date) {
        Ok(prices) => {
            let response: Vec<PriceResponse> = prices
                .into_iter()
                .map(|p| PriceResponse {
                    timestamp: p.timestamp.to_string(),
                    hour: p.timestamp.format("%H").to_string().parse().unwrap_or(0),
                    price: p.price,
                    price_formatted: format!("{:.4} €/kWh", p.price),
                })
                .collect();
            HttpResponse::Ok().json(response)
        }
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

/// Get current hour's price
#[get("/current")]
pub async fn get_current_price(pool: web::Data<DbPool>) -> impl Responder {
    let service = PriceService::new(pool.get_ref().clone());

    match service.get_current_price() {
        Ok(Some(price)) => {
            let response = PriceResponse {
                timestamp: price.timestamp.to_string(),
                hour: price.timestamp.format("%H").to_string().parse().unwrap_or(0),
                price: price.price,
                price_formatted: format!("{:.4} €/kWh", price.price),
            };
            HttpResponse::Ok().json(response)
        }
        Ok(None) => HttpResponse::NotFound().json(serde_json::json!({
            "error": "No price available for current hour",
            "hint": "Prices may need to be synced. Call POST /api/prices/sync"
        })),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

/// Get summary statistics for a date
#[get("/summary")]
pub async fn get_price_summary(pool: web::Data<DbPool>, query: web::Query<DateQuery>) -> impl Responder {
    let service = PriceService::new(pool.get_ref().clone());

    let date = match &query.date {
        Some(d) => match NaiveDate::parse_from_str(d, "%Y-%m-%d") {
            Ok(date) => date,
            Err(_) => return HttpResponse::BadRequest().body("Invalid date format. Use YYYY-MM-DD"),
        },
        None => Local::now().date_naive(),
    };

    match service.get_prices_for_date(date) {
        Ok(prices) if !prices.is_empty() => {
            let min_price = prices.iter().map(|p| p.price).fold(f64::INFINITY, f64::min);
            let max_price = prices.iter().map(|p| p.price).fold(f64::NEG_INFINITY, f64::max);
            let avg_price: f64 = prices.iter().map(|p| p.price).sum::<f64>() / prices.len() as f64;

            let cheapest = prices.iter().min_by(|a, b| a.price.partial_cmp(&b.price).unwrap()).unwrap();
            let most_expensive = prices.iter().max_by(|a, b| a.price.partial_cmp(&b.price).unwrap()).unwrap();

            let summary = PriceSummary {
                date: date.to_string(),
                min_price,
                max_price,
                avg_price,
                cheapest_hour: cheapest.timestamp.format("%H").to_string().parse().unwrap_or(0),
                most_expensive_hour: most_expensive.timestamp.format("%H").to_string().parse().unwrap_or(0),
            };
            HttpResponse::Ok().json(summary)
        }
        Ok(_) => HttpResponse::NotFound().json(serde_json::json!({
            "error": "No prices available for this date",
            "date": date.to_string()
        })),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

/// Get the N cheapest hours for a date
#[get("/cheapest")]
pub async fn get_cheapest_hours(
    pool: web::Data<DbPool>,
    query: web::Query<CheapestHoursQuery>,
) -> impl Responder {
    let service = PriceService::new(pool.get_ref().clone());

    let date = match &query.date {
        Some(d) => match NaiveDate::parse_from_str(d, "%Y-%m-%d") {
            Ok(date) => date,
            Err(_) => return HttpResponse::BadRequest().body("Invalid date format. Use YYYY-MM-DD"),
        },
        None => Local::now().date_naive(),
    };

    let count = query.count.unwrap_or(6).min(24);

    match service.get_cheapest_hours(date, count) {
        Ok(prices) => {
            let response: Vec<PriceResponse> = prices
                .into_iter()
                .map(|p| PriceResponse {
                    timestamp: p.timestamp.to_string(),
                    hour: p.timestamp.format("%H").to_string().parse().unwrap_or(0),
                    price: p.price,
                    price_formatted: format!("{:.4} €/kWh", p.price),
                })
                .collect();
            HttpResponse::Ok().json(response)
        }
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

/// Get the N most expensive hours for a date
#[get("/expensive")]
pub async fn get_expensive_hours(
    pool: web::Data<DbPool>,
    query: web::Query<CheapestHoursQuery>,
) -> impl Responder {
    let service = PriceService::new(pool.get_ref().clone());

    let date = match &query.date {
        Some(d) => match NaiveDate::parse_from_str(d, "%Y-%m-%d") {
            Ok(date) => date,
            Err(_) => return HttpResponse::BadRequest().body("Invalid date format. Use YYYY-MM-DD"),
        },
        None => Local::now().date_naive(),
    };

    let count = query.count.unwrap_or(6).min(24);

    match service.get_most_expensive_hours(date, count) {
        Ok(prices) => {
            let response: Vec<PriceResponse> = prices
                .into_iter()
                .map(|p| PriceResponse {
                    timestamp: p.timestamp.to_string(),
                    hour: p.timestamp.format("%H").to_string().parse().unwrap_or(0),
                    price: p.price,
                    price_formatted: format!("{:.4} €/kWh", p.price),
                })
                .collect();
            HttpResponse::Ok().json(response)
        }
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

/// Sync prices from ESIOS API (today and tomorrow if available)
#[post("/sync")]
pub async fn sync_prices(pool: web::Data<DbPool>) -> impl Responder {
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
        (true, false) => format!("Synced today's prices. Tomorrow not available yet."),
        (false, true) => format!("Failed to sync today. Synced tomorrow."),
        (false, false) => format!("Failed to sync prices. Check ESIOS_TOKEN configuration."),
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
