//! Price series from their sources, stored in `prices` (see services::tariff):
//! - ESIOS indicator 1001: the PVPC of each Spanish zone, by hour
//! - ESIOS indicator 600: the Iberian day-ahead market, in 15-minute slots
//! - REE's PVPC file (archive 70): the hourly tolls and charges of 2.0TD
//! - ENTSO-E (document A44): the day-ahead market of the European zones,
//!   only when ENTSOE_TOKEN is set
//!
//! Each source is asked for a Madrid day and its timestamps are stored in
//! Madrid wall time. The Canaries are the exception: REE labels their PVPC
//! and tolls with the peninsular hour, but they apply by Canary local time,
//! one hour later in real time (so their day runs 01:00 to 01:00 in Madrid).
//! Rows keep the resolution of their source (`minutes`), so a source moving
//! from hours to quarters of an hour needs no change here.

use crate::db::DbPool;
use crate::models::Price;
use crate::schema::prices;
use crate::services::tariff::{combine, hours_between, spot_series, tolls_series};
use crate::zones::{Market, ZONES, Zone};
use chrono::{DateTime, Duration, NaiveDate, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Europe::Madrid;
use diesel::prelude::*;
use log::{error, info, warn};
use serde::Deserialize;

/// ESIOS geo_id of Spain in the market indicators
const ESIOS_SPAIN: i32 = 3;
const ENTSOE_API: &str = "https://web-api.tp.entsoe.eu/api";

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

#[derive(Debug, Clone, PartialEq)]
pub struct PriceData {
    pub timestamp: NaiveDateTime,
    /// €/kWh
    pub price: f64,
    pub series: String,
    pub minutes: i16,
    pub source: &'static str,
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

/// Service for fetching and storing prices
pub struct PriceService {
    pool: DbPool,
    esios_token: Option<String>,
    entsoe_token: Option<String>,
}

impl PriceService {
    pub fn new(pool: DbPool) -> Self {
        let token = |name| std::env::var(name).ok().filter(|t| !t.is_empty());
        Self {
            pool,
            esios_token: token("ESIOS_TOKEN"),
            entsoe_token: token("ENTSOE_TOKEN"),
        }
    }

    /// Fetch every series of `date` and store them. The PVPC is what most
    /// users follow, so only its failure fails the sync; the other sources
    /// are logged and tried again on the next sync.
    pub async fn sync_prices_for_date(&self, date: NaiveDate) -> Result<usize, PriceFetchError> {
        let token = self
            .esios_token
            .as_ref()
            .ok_or(PriceFetchError::MissingToken)?;
        let client = reqwest::Client::new();
        let mut prices = fetch_pvpc_prices(&client, date, token).await?;

        let mut others = vec![
            (
                "Iberian market",
                fetch_esios_spot(&client, date, token).await,
            ),
            ("tolls", fetch_tolls(&client, date).await),
        ];
        if let Some(token) = &self.entsoe_token {
            for zone in ZONES.iter().filter(|z| !z.is_spain()) {
                others.push((zone.code, fetch_entsoe(&client, zone, date, token).await));
            }
        }
        for (name, result) in others {
            match result {
                Ok(p) => prices.extend(p),
                Err(e) => warn!("Prices of {} for {} not fetched: {}", name, date, e),
            }
        }
        self.store_prices(&prices)
    }

    /// Sync prices for today
    pub async fn sync_today(&self) -> Result<usize, PriceFetchError> {
        let today = crate::time::today();
        info!("Syncing prices for today: {}", today);
        self.sync_prices_for_date(today).await
    }

    /// Sync prices for tomorrow (the PVPC is published after 20:00)
    pub async fn sync_tomorrow(&self) -> Result<usize, PriceFetchError> {
        let tomorrow = crate::time::today() + Duration::days(1);
        info!("Syncing prices for tomorrow: {}", tomorrow);
        self.sync_prices_for_date(tomorrow).await
    }

    /// Store prices in the database (upsert)
    pub fn store_prices(&self, prices: &[PriceData]) -> Result<usize, PriceFetchError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| PriceFetchError::DatabaseError(e.to_string()))?;

        let mut count = 0;
        for p in prices {
            let row = Price {
                timestamp: p.timestamp,
                price: p.price,
                source: p.source.to_string(),
                series: p.series.clone(),
                minutes: p.minutes,
            };
            let result = diesel::insert_into(prices::table)
                .values(&row)
                .on_conflict((prices::series, prices::timestamp))
                .do_update()
                .set((
                    prices::price.eq(&row.price),
                    prices::source.eq(&row.source),
                    prices::minutes.eq(row.minutes),
                ))
                .execute(&mut conn);
            match result {
                Ok(_) => count += 1,
                Err(e) => warn!(
                    "Failed to store price {} at {}: {}",
                    p.series, p.timestamp, e
                ),
            }
        }

        info!("Stored {} prices in database", count);
        Ok(count)
    }

    /// The series this server fetches
    fn expected_series(&self) -> Vec<String> {
        let mut series = vec![spot_series(&ZONES[0])];
        for zone in &ZONES {
            match zone.market {
                Market::Spain { .. } => series.extend([zone.code.to_string(), tolls_series(zone)]),
                Market::Entsoe { .. } if self.entsoe_token.is_some() => {
                    series.push(spot_series(zone))
                }
                Market::Entsoe { .. } => {}
            }
        }
        series
    }

    /// The series still missing hours of `date`
    pub fn missing_series(&self, date: NaiveDate) -> Result<Vec<String>, PriceFetchError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| PriceFetchError::DatabaseError(e.to_string()))?;
        let series = self.expected_series();
        let (start, end) = (
            start_of(date),
            start_of(date + Duration::days(1)) + Duration::hours(1),
        );
        let rows: Vec<(NaiveDateTime, String, f64, i16)> = prices::table
            .filter(prices::series.eq_any(&series))
            .filter(prices::timestamp.ge(start))
            .filter(prices::timestamp.lt(end))
            .select((
                prices::timestamp,
                prices::series,
                prices::price,
                prices::minutes,
            ))
            .load(&mut conn)
            .map_err(|e| PriceFetchError::DatabaseError(e.to_string()))?;

        Ok(series
            .into_iter()
            .filter(|s| {
                let have = combine(&rows, std::slice::from_ref(s), 0.0);
                let (start, end) = day_span(s, date);
                !hours_between(start, end).all(|h| have.iter().any(|p| p.hour == h))
            })
            .collect())
    }

    /// Whether every series has all the hours of `date`
    pub fn has_prices_for_date(&self, date: NaiveDate) -> Result<bool, PriceFetchError> {
        Ok(self.missing_series(date)?.is_empty())
    }
}

fn start_of(date: NaiveDate) -> NaiveDateTime {
    date.and_hms_opt(0, 0, 0).unwrap()
}

/// The series follows Canary time: REE labels it one hour early
fn is_canary(series: &str) -> bool {
    let canarias = crate::zones::get("canarias");
    series == canarias.code || series == tolls_series(canarias)
}

/// Engine-time span of a source day of the series
fn day_span(series: &str, date: NaiveDate) -> (NaiveDateTime, NaiveDateTime) {
    let shift = Duration::hours(if is_canary(series) { 1 } else { 0 });
    (
        start_of(date) + shift,
        start_of(date + Duration::days(1)) + shift,
    )
}

async fn esios_values(
    client: &reqwest::Client,
    indicator: i32,
    date: NaiveDate,
    geo_ids: &[i32],
    token: &str,
) -> Result<Vec<EsiosValue>, PriceFetchError> {
    let geo_ids: String = geo_ids
        .iter()
        .map(|id| format!("&geo_ids[]={}", id))
        .collect();
    let url = format!(
        "https://api.esios.ree.es/indicators/{}?start_date={}T00:00&end_date={}T23:59{}",
        indicator, date, date, geo_ids
    );
    let resp = client
        .get(&url)
        .header("x-api-key", token)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| PriceFetchError::NetworkError(e.to_string()))?;
    if !resp.status().is_success() {
        return Err(PriceFetchError::NetworkError(format!(
            "ESIOS API returned status {}",
            resp.status()
        )));
    }
    let body: EsiosResponse = resp
        .json()
        .await
        .map_err(|e| PriceFetchError::ParseError(e.to_string()))?;
    Ok(body.indicator.values)
}

/// PVPC prices of every Spanish zone (ESIOS indicator 1001)
pub async fn fetch_pvpc_prices(
    client: &reqwest::Client,
    date: NaiveDate,
    token: &str,
) -> Result<Vec<PriceData>, PriceFetchError> {
    let geo_ids: Vec<i32> = crate::zones::geo_ids().collect();
    Ok(parse_esios_values(
        esios_values(client, 1001, date, &geo_ids, token).await?,
    ))
}

/// The Iberian day-ahead market (ESIOS indicator 600), Spain's price
async fn fetch_esios_spot(
    client: &reqwest::Client,
    date: NaiveDate,
    token: &str,
) -> Result<Vec<PriceData>, PriceFetchError> {
    let values = esios_values(client, 600, date, &[ESIOS_SPAIN], token).await?;
    let mut slots: Vec<(DateTime<Utc>, f64)> = Vec::new();
    for v in values
        .into_iter()
        .filter(|v| v.geo_id.is_none_or(|id| id == ESIOS_SPAIN))
    {
        match DateTime::parse_from_rfc3339(&v.datetime) {
            Ok(t) => slots.push((t.with_timezone(&Utc), v.value)),
            Err(e) => error!("Bad ESIOS datetime '{}': {}", v.datetime, e),
        }
    }
    slots.sort_by_key(|(t, _)| *t);
    let minutes = slot_minutes(&slots.iter().map(|(t, _)| *t).collect::<Vec<_>>());
    let series = spot_series(&ZONES[0]);
    Ok(merge_duplicate_slots(
        slots
            .into_iter()
            .zip(minutes)
            .map(|((t, value), minutes)| PriceData {
                timestamp: t.with_timezone(&Madrid).naive_local(),
                price: value / 1000.0,
                series: series.clone(),
                minutes,
                source: "esios",
            })
            .collect(),
    ))
}

/// The length of each slot: the gap to the next one (the last repeats the
/// one before; a lone slot is an hour)
fn slot_minutes(times: &[DateTime<Utc>]) -> Vec<i16> {
    let gaps: Vec<i16> = times
        .windows(2)
        .map(|w| (w[1] - w[0]).num_minutes().clamp(1, 60) as i16)
        .collect();
    let last = gaps.last().copied().unwrap_or(60);
    gaps.into_iter()
        .chain(std::iter::once(last))
        .take(times.len())
        .collect()
}

#[derive(Deserialize)]
struct PvpcFile {
    #[serde(rename = "PVPC")]
    hours: Vec<PvpcFileHour>,
}

#[derive(Deserialize)]
struct PvpcFileHour {
    #[serde(rename = "Dia")]
    day: String,
    #[serde(rename = "Hora")]
    hour: String,
    /// Tolls and charges, €/MWh, Peninsula, Canaries and Balearics
    #[serde(rename = "TEUPCB")]
    tolls_pcb: String,
    /// Same for Ceuta and Melilla, whose peak periods are an hour later
    #[serde(rename = "TEUCYM")]
    tolls_cym: String,
}

/// Tolls and charges of 2.0TD by hour, from REE's daily PVPC file (it needs
/// no token). They follow the tariff periods, weekends and national holidays,
/// and REE updates them when the regulator changes them.
async fn fetch_tolls(
    client: &reqwest::Client,
    date: NaiveDate,
) -> Result<Vec<PriceData>, PriceFetchError> {
    let url = format!(
        "https://api.esios.ree.es/archives/70/download_json?locale=es&date={}",
        date
    );
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| PriceFetchError::NetworkError(e.to_string()))?;
    if !resp.status().is_success() {
        return Err(PriceFetchError::NetworkError(format!(
            "REE PVPC file returned status {}",
            resp.status()
        )));
    }
    let file: PvpcFile = resp
        .json()
        .await
        .map_err(|e| PriceFetchError::ParseError(e.to_string()))?;
    parse_tolls(file)
}

fn parse_tolls(file: PvpcFile) -> Result<Vec<PriceData>, PriceFetchError> {
    let number = |s: &str| s.replace('.', "").replace(',', ".").parse::<f64>();
    let mut prices = Vec::new();
    for h in file.hours {
        let bad =
            |what: &str| PriceFetchError::ParseError(format!("{} in '{} {}'", what, h.day, h.hour));
        let day = NaiveDate::parse_from_str(&h.day, "%d/%m/%Y").map_err(|_| bad("Bad day"))?;
        let hour: i64 = h
            .hour
            .get(..2)
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| bad("Bad hour"))?;
        let t = start_of(day) + Duration::hours(hour);
        let pcb = number(&h.tolls_pcb).map_err(|_| bad("Bad tolls"))? / 1000.0;
        let cym = number(&h.tolls_cym).map_err(|_| bad("Bad tolls"))? / 1000.0;
        for zone in ZONES.iter().filter(|z| z.is_spain()) {
            let series = tolls_series(zone);
            let (timestamp, price) = match zone.code {
                "ceuta" | "melilla" => (t, cym),
                _ if is_canary(&series) => (t + Duration::hours(1), pcb),
                _ => (t, pcb),
            };
            prices.push(PriceData {
                timestamp,
                price,
                series,
                minutes: 60,
                source: "ree",
            });
        }
    }
    Ok(merge_duplicate_slots(prices))
}

#[derive(Deserialize)]
struct EntsoeDocument {
    #[serde(rename = "TimeSeries", default)]
    series: Vec<EntsoeSeries>,
}

#[derive(Deserialize)]
struct EntsoeSeries {
    /// A01: day-ahead; other contracts are left out
    #[serde(rename = "contract_MarketAgreement.type")]
    contract: Option<String>,
    /// Some zones publish a second sequence (e.g. another exchange): the first is kept
    #[serde(rename = "classificationSequence_AttributeInstanceComponent.position")]
    sequence: Option<u32>,
    #[serde(rename = "Period", default)]
    periods: Vec<EntsoePeriod>,
}

#[derive(Deserialize)]
struct EntsoePeriod {
    #[serde(rename = "timeInterval")]
    interval: EntsoeInterval,
    resolution: String,
    #[serde(rename = "Point", default)]
    points: Vec<EntsoePoint>,
}

#[derive(Deserialize)]
struct EntsoeInterval {
    start: String,
    end: String,
}

#[derive(Deserialize)]
struct EntsoePoint {
    position: u32,
    #[serde(rename = "price.amount")]
    price: f64,
}

/// Day-ahead prices of a European zone (ENTSO-E document A44)
async fn fetch_entsoe(
    client: &reqwest::Client,
    zone: &Zone,
    date: NaiveDate,
    token: &str,
) -> Result<Vec<PriceData>, PriceFetchError> {
    let Market::Entsoe { eic } = zone.market else {
        return Ok(vec![]);
    };
    let utc = |d: NaiveDate| {
        Madrid
            .from_local_datetime(&start_of(d))
            .earliest()
            .unwrap()
            .with_timezone(&Utc)
            .format("%Y%m%d%H%M")
            .to_string()
    };
    let url = format!(
        "{}?securityToken={}&documentType=A44&in_Domain={}&out_Domain={}&periodStart={}&periodEnd={}",
        ENTSOE_API,
        urlencoding::encode(token),
        urlencoding::encode(eic),
        urlencoding::encode(eic),
        utc(date),
        utc(date + Duration::days(1)),
    );
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| PriceFetchError::NetworkError(e.without_url().to_string()))?;
    if !resp.status().is_success() {
        return Err(PriceFetchError::NetworkError(format!(
            "ENTSO-E returned status {}",
            resp.status()
        )));
    }
    let xml = resp
        .text()
        .await
        .map_err(|e| PriceFetchError::NetworkError(e.without_url().to_string()))?;
    parse_entsoe(&xml, &spot_series(zone))
}

fn parse_entsoe(xml: &str, series: &str) -> Result<Vec<PriceData>, PriceFetchError> {
    // Without data ENTSO-E answers with an acknowledgement document instead
    if xml.contains("Acknowledgement_MarketDocument") {
        return Err(PriceFetchError::ParseError(
            "ENTSO-E has no prices for that day yet".to_string(),
        ));
    }
    let doc: EntsoeDocument =
        quick_xml::de::from_str(xml).map_err(|e| PriceFetchError::ParseError(e.to_string()))?;
    let parse_time = |s: &str| {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%MZ")
            .map(|t| t.and_utc())
            .map_err(|e| PriceFetchError::ParseError(format!("Bad time '{}': {}", s, e)))
    };

    let mut prices = Vec::new();
    for ts in doc.series {
        if ts.contract.as_deref().is_some_and(|c| c != "A01") || ts.sequence.is_some_and(|s| s != 1)
        {
            continue;
        }
        for period in ts.periods {
            let minutes: i16 = match period.resolution.as_str() {
                "PT15M" => 15,
                "PT30M" => 30,
                "PT60M" => 60,
                other => {
                    return Err(PriceFetchError::ParseError(format!(
                        "Unknown resolution {}",
                        other
                    )));
                }
            };
            let (start, end) = (
                parse_time(&period.interval.start)?,
                parse_time(&period.interval.end)?,
            );
            let slots = ((end - start).num_minutes() / minutes as i64).max(0) as u32;
            let mut points = period.points;
            points.sort_by_key(|p| p.position);
            // Curve A03 leaves out the points that repeat the one before
            let mut last = None;
            let mut next = points.iter().peekable();
            for position in 1..=slots {
                while let Some(p) = next.next_if(|p| p.position <= position) {
                    last = Some(p.price);
                }
                let Some(price) = last else { continue };
                let t = start + Duration::minutes((position - 1) as i64 * minutes as i64);
                prices.push(PriceData {
                    timestamp: t.with_timezone(&Madrid).naive_local(),
                    price: price / 1000.0,
                    series: series.to_string(),
                    minutes,
                    source: "entsoe",
                });
            }
        }
    }
    Ok(merge_duplicate_slots(prices))
}

/// Convert raw ESIOS PVPC values into one price per zone and local hour.
/// A value without geo_id is the Peninsula.
fn parse_esios_values(values: Vec<EsiosValue>) -> Vec<PriceData> {
    let parsed: Vec<PriceData> = values
        .into_iter()
        .filter_map(|v| {
            let zone = match v.geo_id {
                None => crate::zones::get(crate::zones::DEFAULT),
                Some(id) => crate::zones::from_geo_id(id)?,
            };
            match parse_esios_value(v.value, &v.datetime) {
                Ok(p) if is_canary(zone.code) => Some(PriceData {
                    timestamp: p.timestamp + Duration::hours(1),
                    series: zone.code.to_string(),
                    ..p
                }),
                Ok(p) => Some(PriceData {
                    series: zone.code.to_string(),
                    ..p
                }),
                Err(e) => {
                    error!("{} ('{}')", e, v.datetime);
                    None
                }
            }
        })
        .collect();

    merge_duplicate_slots(parsed)
}

/// Average prices that share series and local timestamp, keeping first-seen
/// order: the repeated hour of the 25-hour DST day would otherwise collide on
/// the primary key and silently overwrite each other.
fn merge_duplicate_slots(prices: Vec<PriceData>) -> Vec<PriceData> {
    let mut merged: Vec<(PriceData, u32)> = Vec::with_capacity(prices.len());
    for p in prices {
        match merged
            .iter_mut()
            .find(|(m, _)| m.timestamp == p.timestamp && m.series == p.series)
        {
            Some((m, n)) => {
                m.price = (m.price * *n as f64 + p.price) / (*n + 1) as f64;
                *n += 1;
            }
            None => merged.push((p, 1)),
        }
    }
    merged.into_iter().map(|(p, _)| p).collect()
}

/// Parse a single ESIOS PVPC value (€/MWh) into PriceData
pub fn parse_esios_value(value: f64, datetime: &str) -> Result<PriceData, String> {
    let dt = DateTime::parse_from_rfc3339(datetime)
        .map_err(|e| format!("Failed to parse datetime: {}", e))?;

    Ok(PriceData {
        // Madrid wall time, whatever offset ESIOS uses
        timestamp: dt.with_timezone(&Madrid).naive_local(),
        price: value / 1000.0, // Convert €/MWh to €/kWh
        series: crate::zones::DEFAULT.to_string(),
        minutes: 60,
        source: "esios",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Timelike;

    #[test]
    fn test_parse_esios_value_valid() {
        let price_data = parse_esios_value(150.5, "2024-01-15T10:00:00+01:00").unwrap();
        assert_eq!(price_data.timestamp.hour(), 10);
        assert!((price_data.price - 0.1505).abs() < 0.0001); // €/MWh to €/kWh
    }

    #[test]
    fn test_parse_esios_value_utc_timezone() {
        let price_data = parse_esios_value(200.0, "2024-01-15T12:00:00+00:00").unwrap();
        // Converted to Madrid wall time (CET in January)
        assert_eq!(price_data.timestamp.hour(), 13);
    }

    #[test]
    fn test_parse_esios_value_invalid_datetime() {
        let result = parse_esios_value(100.0, "invalid-datetime");
        assert!(result.unwrap_err().contains("Failed to parse datetime"));
    }

    #[test]
    fn test_parse_esios_value_negative_price() {
        // Negative prices can happen in wholesale markets
        let price_data = parse_esios_value(-50.0, "2024-01-15T03:00:00+01:00").unwrap();
        assert!((price_data.price - (-0.05)).abs() < 0.0001);
    }

    fn esios(value: f64, datetime: &str, geo_id: Option<i32>) -> EsiosValue {
        EsiosValue {
            value,
            datetime: datetime.to_string(),
            geo_id,
        }
    }

    #[test]
    fn test_parse_esios_values_one_price_per_zone() {
        let values = vec![
            esios(100.0, "2026-01-15T10:00:00.000+01:00", Some(8741)),
            esios(200.0, "2026-01-15T10:00:00.000+01:00", Some(8742)),
            esios(300.0, "2026-01-15T10:00:00.000+01:00", Some(8745)),
            esios(400.0, "2026-01-15T10:00:00.000+01:00", Some(9999)),
        ];

        let prices = parse_esios_values(values);

        assert_eq!(prices.len(), 3);
        assert_eq!(prices[0].series, "peninsula");
        assert!((prices[0].price - 0.1).abs() < 0.0001);
        // The Canaries' 10:00 is 11:00 in Madrid
        assert_eq!(prices[1].series, "canarias");
        assert_eq!(prices[1].timestamp.hour(), 11);
        assert_eq!(prices[2].series, "melilla");
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
    fn test_slot_minutes() {
        let t =
            |m: i64| Utc.with_ymd_and_hms(2026, 10, 24, 10, 0, 0).unwrap() + Duration::minutes(m);
        assert_eq!(
            slot_minutes(&[t(0), t(15), t(30), t(45)]),
            vec![15, 15, 15, 15]
        );
        assert_eq!(slot_minutes(&[t(0), t(60)]), vec![60, 60]);
        assert_eq!(slot_minutes(&[t(0)]), vec![60]);
        assert!(slot_minutes(&[]).is_empty());
    }

    #[test]
    fn test_parse_tolls() {
        let hour = |h: &str, pcb: &str, cym: &str| PvpcFileHour {
            day: "23/10/2026".into(),
            hour: h.into(),
            tolls_pcb: pcb.into(),
            tolls_cym: cym.into(),
        };
        let file = PvpcFile {
            hours: vec![
                hour("10-11", "97,55", "29,27"),
                hour("11-12", "97,55", "97,55"),
            ],
        };
        let prices = parse_tolls(file).unwrap();
        let find = |series: &str, h: u32| {
            prices
                .iter()
                .find(|p| p.series == series && p.timestamp.hour() == h)
                .map(|p| p.price)
        };
        assert_eq!(find("tolls-peninsula", 10), Some(0.09755));
        assert_eq!(find("tolls-ceuta", 10), Some(0.02927));
        // The Canaries' 10:00 peak is 11:00 in Madrid
        assert_eq!(find("tolls-canarias", 11), Some(0.09755));
        assert_eq!(find("tolls-canarias", 10), None);
    }

    #[test]
    fn test_parse_entsoe() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<Publication_MarketDocument xmlns="urn:iec62325.351:tc57wg16:451-3:publicationdocument:7:3">
  <mRID>1</mRID>
  <TimeSeries>
    <mRID>1</mRID>
    <contract_MarketAgreement.type>A01</contract_MarketAgreement.type>
    <in_Domain.mRID codingScheme="A01">10YFI-1--------U</in_Domain.mRID>
    <currency_Unit.name>EUR</currency_Unit.name>
    <curveType>A03</curveType>
    <Period>
      <timeInterval><start>2026-10-23T22:00Z</start><end>2026-10-23T23:00Z</end></timeInterval>
      <resolution>PT15M</resolution>
      <Point><position>1</position><price.amount>40.0</price.amount></Point>
      <Point><position>3</position><price.amount>80.5</price.amount></Point>
    </Period>
  </TimeSeries>
  <TimeSeries>
    <mRID>2</mRID>
    <contract_MarketAgreement.type>A07</contract_MarketAgreement.type>
    <Period>
      <timeInterval><start>2026-10-23T22:00Z</start><end>2026-10-23T23:00Z</end></timeInterval>
      <resolution>PT60M</resolution>
      <Point><position>1</position><price.amount>999</price.amount></Point>
    </Period>
  </TimeSeries>
</Publication_MarketDocument>"#;
        let prices = parse_entsoe(xml, "spot-fi").unwrap();
        // 22:00 UTC is 00:00 in Madrid (CEST)
        let got: Vec<(u32, u32, f64, i16)> = prices
            .iter()
            .map(|p| (p.timestamp.hour(), p.timestamp.minute(), p.price, p.minutes))
            .collect();
        assert_eq!(
            got,
            vec![
                (0, 0, 0.04, 15),
                (0, 15, 0.04, 15),
                (0, 30, 0.0805, 15),
                (0, 45, 0.0805, 15)
            ]
        );
    }

    #[test]
    fn test_parse_entsoe_without_data() {
        let xml = "<Acknowledgement_MarketDocument><Reason><text>No matching data found</text></Reason></Acknowledgement_MarketDocument>";
        assert!(parse_entsoe(xml, "spot-fi").is_err());
    }

    #[test]
    fn test_day_span() {
        let date = NaiveDate::from_ymd_opt(2026, 10, 24).unwrap();
        assert_eq!(day_span("canarias", date).0.hour(), 1);
        assert_eq!(day_span("tolls-canarias", date).0.hour(), 1);
        assert_eq!(day_span("spot-es", date).0.hour(), 0);
    }

    /// Against the real sources; the market ones need their tokens:
    ///   ESIOS_TOKEN=… ENTSOE_TOKEN=… cargo test -p backend live_prices -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn test_live_prices() {
        let client = reqwest::Client::new();
        let today = crate::time::today();
        let tolls = fetch_tolls(&client, today).await.unwrap();
        assert_eq!(
            tolls
                .iter()
                .filter(|p| p.series == "tolls-peninsula")
                .count() as i64,
            hours_between(start_of(today), start_of(today + Duration::days(1))).count() as i64
        );
        println!("tolls: {} rows", tolls.len());
        if let Ok(token) = std::env::var("ESIOS_TOKEN") {
            let spot = fetch_esios_spot(&client, today, &token).await.unwrap();
            println!("spot-es: {} rows of {} min", spot.len(), spot[0].minutes);
            assert!(spot.len() >= 23);
        }
        if let Ok(token) = std::env::var("ENTSOE_TOKEN") {
            let fi = fetch_entsoe(&client, crate::zones::get("fi"), today, &token)
                .await
                .unwrap();
            println!("spot-fi: {} rows of {} min", fi.len(), fi[0].minutes);
            assert!(fi.len() >= 23);
        }
    }
}
