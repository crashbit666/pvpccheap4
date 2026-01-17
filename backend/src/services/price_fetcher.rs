use chrono::{DateTime, Datelike, NaiveDate, NaiveDateTime, Timelike};
use reqwest::Error;
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
}

#[derive(Debug)]
pub struct PriceData {
    pub timestamp: NaiveDateTime,
    pub price: f64,
}

pub async fn fetch_pvpc_prices(date: NaiveDate, token: &str) -> Result<Vec<PriceData>, Error> {
    // URL for PVPC 2.0TD (Indicator 1001)
    let url = format!(
        "https://api.esios.ree.es/indicators/1001?start_date={}T00:00&end_date={}T23:59",
        date, date
    );

    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .header("Authorization", format!("Token token=\"{}\"", token))
        .send()
        .await?
        .json::<EsiosResponse>()
        .await?;

    let prices = resp
        .indicator
        .values
        .into_iter()
        .map(|v| {
            // Parse datetime. Example: "2024-01-17T00:00:00+01:00"
            // We want to store it as NaiveDateTime (local time usually for user schedules).
            // Let's parse as DateTime first to handle the offset.
            let dt = DateTime::parse_from_rfc3339(&v.datetime)
                .expect("Failed to parse datetime from ESIOS");

            PriceData {
                timestamp: dt.naive_local(),
                price: v.value / 1000.0, // Convert €/MWh to €/kWh
            }
        })
        .collect();

    Ok(prices)
}

/// Parse a single ESIOS value into PriceData (exposed for testing)
pub fn parse_esios_value(value: f64, datetime: &str) -> Result<PriceData, String> {
    let dt = DateTime::parse_from_rfc3339(datetime)
        .map_err(|e| format!("Failed to parse datetime: {}", e))?;

    Ok(PriceData {
        timestamp: dt.naive_local(),
        price: value / 1000.0, // Convert €/MWh to €/kWh
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
        // naive_local() will convert to local time
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
