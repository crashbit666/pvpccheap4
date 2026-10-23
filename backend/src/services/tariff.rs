//! What a user pays each hour, built from the price series in the database.
//!
//! Series (prices.series):
//! - "<zone>": the PVPC of a Spanish zone (ESIOS), by hour
//! - "spot-es": the Iberian day-ahead market (ESIOS), in 15-minute slots
//! - "tolls-<zone>": tolls and charges of the 2.0TD tariff of a Spanish zone
//!   (REE), by hour, so an indexed tariff follows its peak and off-peak periods
//! - "spot-<zone>": the day-ahead market of a European zone (ENTSO-E)
//!
//! Each row covers `minutes` from its timestamp. Rules switch devices by whole
//! hours, so shorter slots are averaged into their hour; an hour counts only
//! when every series covers all of it.

use crate::schema::{prices, users};
use crate::zones::{Market, Zone};
use chrono::{Duration, NaiveDate, NaiveDateTime, Timelike};
use diesel::prelude::*;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    /// Spain's regulated price
    Pvpc,
    /// The day-ahead market price (plus, in Spain, the tolls of the hour)
    /// and the retailer's margin
    Indexed,
}

impl Kind {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "pvpc" => Some(Self::Pvpc),
            "indexed" => Some(Self::Indexed),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pvpc => "pvpc",
            Self::Indexed => "indexed",
        }
    }
}

/// The highest margin accepted, €/kWh: anything above is a typing mistake
pub const MAX_MARGIN: f64 = 1.0;

/// One hour of a tariff, in engine (Madrid) time
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HourPrice {
    pub hour: NaiveDateTime,
    /// €/kWh
    pub price: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tariff {
    pub zone: &'static Zone,
    pub kind: Kind,
    /// €/kWh added to every hour of an indexed tariff
    pub margin: f64,
}

pub fn spot_series(zone: &Zone) -> String {
    match zone.market {
        Market::Spain { .. } => "spot-es".to_string(),
        Market::Entsoe { .. } => format!("spot-{}", zone.code),
    }
}

pub fn tolls_series(zone: &Zone) -> String {
    format!("tolls-{}", zone.code)
}

impl Tariff {
    /// None for a combination we cannot price (the PVPC outside Spain)
    pub fn new(zone: &'static Zone, kind: Kind, margin: f64) -> Option<Self> {
        if kind == Kind::Pvpc && !zone.is_spain() || !(0.0..=MAX_MARGIN).contains(&margin) {
            return None;
        }
        Some(Self { zone, kind, margin: if kind == Kind::Pvpc { 0.0 } else { margin } })
    }

    /// What a zone shows to someone without an account: the PVPC in Spain,
    /// the bare market price elsewhere
    pub fn default_for(zone: &'static Zone) -> Self {
        let kind = if zone.is_spain() { Kind::Pvpc } else { Kind::Indexed };
        Self { zone, kind, margin: 0.0 }
    }

    pub fn of_user(conn: &mut PgConnection, user_id: i32) -> Self {
        let row: Option<(String, String, f64)> = users::table
            .find(user_id)
            .select((users::zone, users::tariff, users::tariff_margin))
            .first(conn)
            .ok();
        let Some((zone, kind, margin)) = row else {
            return Self::default_for(crate::zones::get(crate::zones::DEFAULT));
        };
        let zone = crate::zones::get(&zone);
        Kind::parse(&kind)
            .and_then(|kind| Self::new(zone, kind, margin))
            .unwrap_or_else(|| Self::default_for(zone))
    }

    /// The series added together to price an hour
    pub fn series(&self) -> Vec<String> {
        match (self.kind, self.zone.market) {
            (Kind::Pvpc, _) => vec![self.zone.code.to_string()],
            (Kind::Indexed, Market::Spain { .. }) => vec![spot_series(self.zone), tolls_series(self.zone)],
            (Kind::Indexed, Market::Entsoe { .. }) => vec![spot_series(self.zone)],
        }
    }

    /// Engine-time span [start, end) of a day in the zone's local time
    pub fn local_day(&self, date: NaiveDate) -> (NaiveDateTime, NaiveDateTime) {
        let midnight = date.and_hms_opt(0, 0, 0).unwrap();
        (self.zone.to_engine(midnight), self.zone.to_engine(midnight + Duration::days(1)))
    }

    /// Prices of the hours in [start, end) (engine time) that have every
    /// series, in time order
    pub fn hourly(
        &self,
        conn: &mut PgConnection,
        start: NaiveDateTime,
        end: NaiveDateTime,
    ) -> QueryResult<Vec<HourPrice>> {
        let series = self.series();
        let rows: Vec<(NaiveDateTime, String, f64, i16)> = prices::table
            .filter(prices::series.eq_any(&series))
            .filter(prices::timestamp.ge(start))
            .filter(prices::timestamp.lt(end))
            .select((prices::timestamp, prices::series, prices::price, prices::minutes))
            .load(conn)?;
        Ok(combine(&rows, &series, self.margin))
    }

    /// The prices of a local day, in time order
    pub fn day(&self, conn: &mut PgConnection, date: NaiveDate) -> QueryResult<Vec<HourPrice>> {
        let (start, end) = self.local_day(date);
        self.hourly(conn, start, end)
    }

    /// Price of the hour that contains `t` (engine time)
    pub fn at(&self, conn: &mut PgConnection, t: NaiveDateTime) -> QueryResult<Option<f64>> {
        let hour = start_of_hour(t);
        Ok(self.hourly(conn, hour, hour + Duration::hours(1))?.first().map(|p| p.price))
    }

    /// True when every hour of [start, end) has its price
    pub fn complete(&self, conn: &mut PgConnection, start: NaiveDateTime, end: NaiveDateTime) -> QueryResult<bool> {
        let have = self.hourly(conn, start, end)?;
        Ok(hours_between(start, end).all(|h| have.iter().any(|p| p.hour == h)))
    }
}

pub fn start_of_hour(t: NaiveDateTime) -> NaiveDateTime {
    t.date().and_hms_opt(t.hour(), 0, 0).unwrap()
}

/// The hours of [start, end) that happen in Madrid (no spring-forward hour)
pub fn hours_between(start: NaiveDateTime, end: NaiveDateTime) -> impl Iterator<Item = NaiveDateTime> {
    let first = start_of_hour(start);
    let first = if first < start { first + Duration::hours(1) } else { first };
    std::iter::successors(Some(first), |h| Some(*h + Duration::hours(1)))
        .take_while(move |h| *h < end)
        .filter(|h| crate::time::exists(*h))
}

/// Hourly prices from rows of (start, series, €/kWh, minutes): each series
/// is averaged over its slots in the hour (weighted by length), the series
/// are added up and the margin on top. Hours a series does not fully cover
/// are left out.
pub(crate) fn combine(rows: &[(NaiveDateTime, String, f64, i16)], series: &[String], margin: f64) -> Vec<HourPrice> {
    // hour -> series -> (sum of price × minutes, minutes)
    let mut hours: BTreeMap<NaiveDateTime, BTreeMap<&str, (f64, i64)>> = BTreeMap::new();
    for (t, s, price, minutes) in rows {
        let e = hours.entry(start_of_hour(*t)).or_default().entry(s.as_str()).or_default();
        e.0 += price * *minutes as f64;
        e.1 += *minutes as i64;
    }
    hours
        .into_iter()
        .filter_map(|(hour, by_series)| {
            let mut total = margin;
            for s in series {
                let (sum, minutes) = by_series.get(s.as_str())?;
                if *minutes < 60 {
                    return None;
                }
                total += sum / *minutes as f64;
            }
            Some(HourPrice { hour, price: total })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zones::get;

    fn at(d: u32, h: u32, m: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, d).unwrap().and_hms_opt(h, m, 0).unwrap()
    }

    #[test]
    fn test_series() {
        let t = |zone, kind| Tariff::new(get(zone), kind, 0.01).map(|t| t.series());
        assert_eq!(t("canarias", Kind::Pvpc), Some(vec!["canarias".to_string()]));
        assert_eq!(t("ceuta", Kind::Indexed), Some(vec!["spot-es".to_string(), "tolls-ceuta".to_string()]));
        assert_eq!(t("fi", Kind::Indexed), Some(vec!["spot-fi".to_string()]));
        assert_eq!(t("fi", Kind::Pvpc), None);
        assert!(Tariff::new(get("peninsula"), Kind::Indexed, 2.0).is_none());
        // The PVPC has no margin
        assert_eq!(Tariff::new(get("peninsula"), Kind::Pvpc, 0.05).unwrap().margin, 0.0);
    }

    #[test]
    fn test_combine_quarters_and_tolls() {
        let series = vec!["spot-es".to_string(), "tolls-peninsula".to_string()];
        let mut rows = vec![(at(24, 10, 0), "tolls-peninsula".to_string(), 0.0975, 60)];
        for (q, p) in [0.04, 0.05, 0.06, 0.05].iter().enumerate() {
            rows.push((at(24, 10, 15 * q as u32), "spot-es".to_string(), *p, 15));
        }
        // 11:00 has the market price but no tolls yet, 12:00 only half an hour
        rows.push((at(24, 11, 0), "spot-es".to_string(), 0.05, 60));
        rows.push((at(24, 12, 0), "spot-es".to_string(), 0.05, 15));
        rows.push((at(24, 12, 15), "spot-es".to_string(), 0.05, 15));
        rows.push((at(24, 12, 0), "tolls-peninsula".to_string(), 0.0975, 60));
        let prices = combine(&rows, &series, 0.01);
        assert_eq!(prices.len(), 1);
        assert_eq!(prices[0].hour, at(24, 10, 0));
        assert!((prices[0].price - (0.05 + 0.0975 + 0.01)).abs() < 1e-9);
    }

    #[test]
    fn test_local_day() {
        let date = NaiveDate::from_ymd_opt(2026, 10, 24).unwrap();
        let canarias = Tariff::default_for(get("canarias"));
        assert_eq!(canarias.local_day(date), (at(24, 1, 0), at(25, 1, 0)));
        let finland = Tariff::default_for(get("fi"));
        assert_eq!(finland.local_day(date), (at(23, 23, 0), at(24, 23, 0)));
    }

    #[test]
    fn test_hours_between_skips_spring_forward() {
        let day = |d| NaiveDate::from_ymd_opt(2026, 3, d).unwrap().and_hms_opt(0, 0, 0).unwrap();
        assert_eq!(hours_between(day(29), day(30)).count(), 23);
        assert_eq!(hours_between(at(24, 10, 30), at(24, 13, 0)).collect::<Vec<_>>(), vec![at(24, 11, 0), at(24, 12, 0)]);
    }
}
