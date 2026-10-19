use crate::db::DbPool;
use crate::models::Price;
use crate::schema::prices;
use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Timelike};
use chrono_tz::Europe::Madrid;
use diesel::prelude::*;
use log::{error, info, warn};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct EsiosResponse {
    indicator: Indicator,
}

#[derive(Deserialize, Debug)]
struct Indicator {
    values: Vec<EsiosValue>,
}

#[derive(Deserialize, Debug)]
struct EsiosValue {
    value: f64,
    datetime: String,
    geo_id: Option<i32>,
}

/// ESIOS geo_id for the Iberian Peninsula. Indicator 1001 also returns
/// Canarias, Baleares, Ceuta and Melilla with the same timestamps.
const GEO_ID_PENINSULA: i32 = 8741;

#[derive(Debug, Clone)]
pub struct PriceData {
    pub timestamp: NaiveDateTime,
    pub price: f64,
}

/// Error types for price fetching operations
#[derive(Debug)]
pub enum PriceFetchError {
    NetworkError(String),
    ParseError(String),
    DatabaseError(String),
    MissingToken,
}

impl std::fmt::Display for PriceFetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PriceFetchError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            PriceFetchError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            PriceFetchError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
            PriceFetchError::MissingToken => write!(f, "ESIOS API token not configured"),
        }
    }
}

impl std::error::Error for PriceFetchError {}

/// Service for fetching and storing PVPC prices
pub struct PriceService {
    pool: DbPool,
    esios_token: Option<String>,
}

impl PriceService {
    pub fn new(pool: DbPool) -> Self {
        let esios_token = std::env::var("ESIOS_TOKEN").ok();
        Self { pool, esios_token }
    }

    pub fn with_token(pool: DbPool, token: String) -> Self {
        Self {
            pool,
            esios_token: Some(token),
        }
    }

    /// Fetch prices from ESIOS API for a specific date
    pub async fn fetch_prices_from_api(&self, date: NaiveDate) -> Result<Vec<PriceData>, PriceFetchError> {
        let token = self.esios_token.as_ref().ok_or(PriceFetchError::MissingToken)?;
        fetch_pvpc_prices(date, token).await
    }

    /// Sync prices for a specific date (fetch from API and store in DB)
    pub async fn sync_prices_for_date(&self, date: NaiveDate) -> Result<usize, PriceFetchError> {
        let prices = self.fetch_prices_from_api(date).await?;
        self.store_prices(&prices)
    }

    /// Sync prices for today
    pub async fn sync_today(&self) -> Result<usize, PriceFetchError> {
        let today = crate::time::today();
        info!("Syncing PVPC prices for today: {}", today);
        self.sync_prices_for_date(today).await
    }

    /// Sync prices for tomorrow (available after 20:00)
    pub async fn sync_tomorrow(&self) -> Result<usize, PriceFetchError> {
        let tomorrow = crate::time::today() + chrono::Duration::days(1);
        info!("Syncing PVPC prices for tomorrow: {}", tomorrow);
        self.sync_prices_for_date(tomorrow).await
    }

    /// Store prices in the database (upsert)
    pub fn store_prices(&self, prices: &[PriceData]) -> Result<usize, PriceFetchError> {
        let mut conn = self.pool.get()
            .map_err(|e| PriceFetchError::DatabaseError(e.to_string()))?;

        let mut count = 0;
        for price_data in prices {
            let new_price = Price {
                timestamp: price_data.timestamp,
                price: price_data.price,
                source: "esios".to_string(),
            };

            // Upsert: insert or update on conflict
            let result = diesel::insert_into(prices::table)
                .values(&new_price)
                .on_conflict(prices::timestamp)
                .do_update()
                .set((
                    prices::price.eq(&new_price.price),
                    prices::source.eq(&new_price.source),
                ))
                .execute(&mut conn);

            match result {
                Ok(_) => count += 1,
                Err(e) => {
                    warn!("Failed to store price for {}: {}", price_data.timestamp, e);
                }
            }
        }

        info!("Stored {} prices in database", count);
        Ok(count)
    }

    /// Get prices for a specific date from the database
    pub fn get_prices_for_date(&self, date: NaiveDate) -> Result<Vec<Price>, PriceFetchError> {
        let mut conn = self.pool.get()
            .map_err(|e| PriceFetchError::DatabaseError(e.to_string()))?;

        let start = date.and_hms_opt(0, 0, 0).unwrap();
        let end = date.and_hms_opt(23, 59, 59).unwrap();

        prices::table
            .filter(prices::timestamp.ge(start))
            .filter(prices::timestamp.le(end))
            .order(prices::timestamp.asc())
            .load::<Price>(&mut conn)
            .map_err(|e| PriceFetchError::DatabaseError(e.to_string()))
    }

    /// Get current hour's price
    pub fn get_current_price(&self) -> Result<Option<Price>, PriceFetchError> {
        let mut conn = self.pool.get()
            .map_err(|e| PriceFetchError::DatabaseError(e.to_string()))?;

        let now = crate::time::now_naive();
        let hour_start = now.date().and_hms_opt(now.hour(), 0, 0).unwrap();

        prices::table
            .filter(prices::timestamp.eq(hour_start))
            .first::<Price>(&mut conn)
            .optional()
            .map_err(|e| PriceFetchError::DatabaseError(e.to_string()))
    }

    /// Get the cheapest N hours for a date
    pub fn get_cheapest_hours(&self, date: NaiveDate, n: usize) -> Result<Vec<Price>, PriceFetchError> {
        let mut conn = self.pool.get()
            .map_err(|e| PriceFetchError::DatabaseError(e.to_string()))?;

        let start = date.and_hms_opt(0, 0, 0).unwrap();
        let end = date.and_hms_opt(23, 59, 59).unwrap();

        prices::table
            .filter(prices::timestamp.ge(start))
            .filter(prices::timestamp.le(end))
            .order(prices::price.asc())
            .limit(n as i64)
            .load::<Price>(&mut conn)
            .map_err(|e| PriceFetchError::DatabaseError(e.to_string()))
    }

    /// Get the most expensive N hours for a date
    pub fn get_most_expensive_hours(&self, date: NaiveDate, n: usize) -> Result<Vec<Price>, PriceFetchError> {
        let mut conn = self.pool.get()
            .map_err(|e| PriceFetchError::DatabaseError(e.to_string()))?;

        let start = date.and_hms_opt(0, 0, 0).unwrap();
        let end = date.and_hms_opt(23, 59, 59).unwrap();

        prices::table
            .filter(prices::timestamp.ge(start))
            .filter(prices::timestamp.le(end))
            .order(prices::price.desc())
            .limit(n as i64)
            .load::<Price>(&mut conn)
            .map_err(|e| PriceFetchError::DatabaseError(e.to_string()))
    }

    /// Check if we have prices for a specific date
    pub fn has_prices_for_date(&self, date: NaiveDate) -> Result<bool, PriceFetchError> {
        let mut conn = self.pool.get()
            .map_err(|e| PriceFetchError::DatabaseError(e.to_string()))?;

        let start = date.and_hms_opt(0, 0, 0).unwrap();
        let end = date.and_hms_opt(23, 59, 59).unwrap();

        let count: i64 = prices::table
            .filter(prices::timestamp.ge(start))
            .filter(prices::timestamp.le(end))
            .count()
            .get_result(&mut conn)
            .map_err(|e| PriceFetchError::DatabaseError(e.to_string()))?;

        Ok(count >= expected_price_rows(date))
    }
}

/// Fetch PVPC prices from ESIOS API
pub async fn fetch_pvpc_prices(date: NaiveDate, token: &str) -> Result<Vec<PriceData>, PriceFetchError> {
    // URL for PVPC 2.0TD (Indicator 1001)
    let url = format!(
        "https://api.esios.ree.es/indicators/1001?start_date={}T00:00&end_date={}T23:59&geo_ids[]={}",
        date, date, GEO_ID_PENINSULA
    );

    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .header("x-api-key", token)
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| PriceFetchError::NetworkError(e.to_string()))?;

    if !resp.status().is_success() {
        return Err(PriceFetchError::NetworkError(format!(
            "ESIOS API returned status {}",
            resp.status()
        )));
    }

    let esios_response: EsiosResponse = resp
        .json()
        .await
        .map_err(|e| PriceFetchError::ParseError(e.to_string()))?;

    Ok(parse_esios_values(esios_response.indicator.values))
}

/// Convert raw ESIOS values into one price per local hour.
///
/// Keeps only the Peninsula (in case the geo filter is ignored) and merges the
/// repeated 02:00 of the 25-hour DST day, which would otherwise collide on the
/// `prices.timestamp` primary key and silently overwrite each other.
fn parse_esios_values(values: Vec<EsiosValue>) -> Vec<PriceData> {
    let parsed: Vec<PriceData> = values
        .into_iter()
        .filter(|v| v.geo_id.is_none_or(|id| id == GEO_ID_PENINSULA))
        .filter_map(|v| match parse_esios_value(v.value, &v.datetime) {
            Ok(p) => Some(p),
            Err(e) => {
                error!("{} ('{}')", e, v.datetime);
                None
            }
        })
        .collect();

    merge_duplicate_hours(parsed)
}

/// Average prices that share the same local timestamp, keeping first-seen order.
fn merge_duplicate_hours(prices: Vec<PriceData>) -> Vec<PriceData> {
    let mut merged: Vec<(PriceData, u32)> = Vec::with_capacity(prices.len());
    for p in prices {
        match merged.iter_mut().find(|(m, _)| m.timestamp == p.timestamp) {
            Some((m, n)) => {
                warn!("Duplicate price for {} (DST change), averaging", p.timestamp);
                m.price = (m.price * *n as f64 + p.price) / (*n + 1) as f64;
                *n += 1;
            }
            None => merged.push((p, 1)),
        }
    }
    merged.into_iter().map(|(p, _)| p).collect()
}

/// Number of price rows a fully synced day has: 24, or 23 on the spring DST day.
/// The autumn DST day has 25 hours but its repeated hour is merged into one row.
pub fn expected_price_rows(date: NaiveDate) -> i64 {
    let local_midnight = |d: NaiveDate| {
        Madrid
            .from_local_datetime(&d.and_hms_opt(0, 0, 0).unwrap())
            .earliest()
            .unwrap()
    };
    let next = date + chrono::Duration::days(1);
    let hours = (local_midnight(next) - local_midnight(date)).num_hours();
    hours.min(24)
}

/// Parse a single ESIOS value into PriceData (exposed for testing)
pub fn parse_esios_value(value: f64, datetime: &str) -> Result<PriceData, String> {
    let dt = DateTime::parse_from_rfc3339(datetime)
        .map_err(|e| format!("Failed to parse datetime: {}", e))?;

    Ok(PriceData {
        // Madrid wall time, whatever offset ESIOS uses
        timestamp: dt.with_timezone(&Madrid).naive_local(),
        price: value / 1000.0, // Convert €/MWh to €/kWh
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;

    #[test]
    fn test_parse_esios_value_valid() {
        let result = parse_esios_value(150.5, "2024-01-15T10:00:00+01:00");

        assert!(result.is_ok());
        let price_data = result.unwrap();
        assert_eq!(price_data.timestamp.hour(), 10);
        assert!((price_data.price - 0.1505).abs() < 0.0001); // €/MWh to €/kWh
    }

    #[test]
    fn test_parse_esios_value_midnight() {
        let result = parse_esios_value(100.0, "2024-01-15T00:00:00+01:00");

        assert!(result.is_ok());
        let price_data = result.unwrap();
        assert_eq!(price_data.timestamp.hour(), 0);
        assert_eq!(price_data.timestamp.minute(), 0);
    }

    #[test]
    fn test_parse_esios_value_utc_timezone() {
        let result = parse_esios_value(200.0, "2024-01-15T12:00:00+00:00");

        assert!(result.is_ok());
        let price_data = result.unwrap();
        // Converted to Madrid wall time (CET in January)
        assert_eq!(price_data.timestamp.hour(), 13);
        assert!((price_data.price - 0.2).abs() < 0.0001);
    }

    #[test]
    fn test_parse_esios_value_invalid_datetime() {
        let result = parse_esios_value(100.0, "invalid-datetime");

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to parse datetime"));
    }

    #[test]
    fn test_parse_esios_value_price_conversion() {
        // Test that €/MWh is correctly converted to €/kWh
        let result = parse_esios_value(1000.0, "2024-01-15T12:00:00+01:00");

        assert!(result.is_ok());
        let price_data = result.unwrap();
        assert!((price_data.price - 1.0).abs() < 0.0001); // 1000 €/MWh = 1 €/kWh
    }

    #[test]
    fn test_parse_esios_value_negative_price() {
        // Negative prices can happen in wholesale markets
        let result = parse_esios_value(-50.0, "2024-01-15T03:00:00+01:00");

        assert!(result.is_ok());
        let price_data = result.unwrap();
        assert!((price_data.price - (-0.05)).abs() < 0.0001);
    }

    #[test]
    fn test_parse_esios_value_zero_price() {
        let result = parse_esios_value(0.0, "2024-01-15T04:00:00+01:00");

        assert!(result.is_ok());
        let price_data = result.unwrap();
        assert!((price_data.price - 0.0).abs() < 0.0001);
    }

    fn esios(value: f64, datetime: &str, geo_id: Option<i32>) -> EsiosValue {
        EsiosValue {
            value,
            datetime: datetime.to_string(),
            geo_id,
        }
    }

    #[test]
    fn test_parse_esios_values_keeps_only_peninsula() {
        let values = vec![
            esios(100.0, "2026-01-15T10:00:00.000+01:00", Some(8741)),
            esios(200.0, "2026-01-15T10:00:00.000+01:00", Some(8742)),
            esios(300.0, "2026-01-15T10:00:00.000+01:00", Some(8745)),
        ];

        let prices = parse_esios_values(values);

        assert_eq!(prices.len(), 1);
        assert!((prices[0].price - 0.1).abs() < 0.0001);
    }

    #[test]
    fn test_parse_esios_values_without_geo_id() {
        let values = vec![
            esios(100.0, "2026-01-15T10:00:00.000+01:00", None),
            esios(110.0, "2026-01-15T11:00:00.000+01:00", None),
        ];

        assert_eq!(parse_esios_values(values).len(), 2);
    }

    #[test]
    fn test_parse_esios_values_merges_repeated_dst_hour() {
        // 25 October 2026: 02:00 happens twice (CEST, then CET)
        let values = vec![
            esios(100.0, "2026-10-25T01:00:00.000+02:00", Some(8741)),
            esios(80.0, "2026-10-25T02:00:00.000+02:00", Some(8741)),
            esios(60.0, "2026-10-25T02:00:00.000+01:00", Some(8741)),
            esios(90.0, "2026-10-25T03:00:00.000+01:00", Some(8741)),
        ];

        let prices = parse_esios_values(values);

        assert_eq!(prices.len(), 3);
        assert_eq!(prices[1].timestamp.hour(), 2);
        assert!((prices[1].price - 0.07).abs() < 0.0001);
        assert_eq!(prices[2].timestamp.hour(), 3);
    }

    #[test]
    fn test_expected_price_rows() {
        let d = |y, m, day| NaiveDate::from_ymd_opt(y, m, day).unwrap();
        assert_eq!(expected_price_rows(d(2026, 1, 15)), 24);
        assert_eq!(expected_price_rows(d(2026, 3, 29)), 23); // spring forward
        assert_eq!(expected_price_rows(d(2026, 10, 25)), 24); // fall back, merged
    }

    #[test]
    fn test_price_data_struct() {
        let timestamp = NaiveDate::from_ymd_opt(2024, 1, 15)
            .unwrap()
            .and_hms_opt(10, 0, 0)
            .unwrap();

        let price_data = PriceData {
            timestamp,
            price: 0.15,
        };

        assert_eq!(price_data.timestamp.year(), 2024);
        assert_eq!(price_data.timestamp.month(), 1);
        assert_eq!(price_data.timestamp.day(), 15);
        assert_eq!(price_data.price, 0.15);
    }
}
