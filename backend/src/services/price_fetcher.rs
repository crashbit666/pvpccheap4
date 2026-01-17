use chrono::{DateTime, NaiveDate, NaiveDateTime};
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
