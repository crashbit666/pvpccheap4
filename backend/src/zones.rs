//! Where a user lives: which prices they can follow and their local time.
//!
//! Every naive timestamp in the database is Madrid wall time (crate::time),
//! the clock the engine runs on. A zone's local time differs from Madrid's by
//! whole hours that never change: every zone here follows the EU summer-time
//! rule, which moves all their clocks at the same instant (01:00 UTC). So the
//! Canaries are always one hour behind Madrid and Finland one ahead, and
//! converting between them is adding hours, even on the days the clocks change.

use crate::schema::users;
use chrono::{Duration, NaiveDateTime, Offset, TimeZone, Utc};
use chrono_tz::{Europe::Madrid, Tz};
use diesel::prelude::*;

pub const DEFAULT: &str = "peninsula";

/// Where a zone's prices come from
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Market {
    /// Spain: the PVPC that ESIOS publishes for the zone (geo_id), or a
    /// tariff indexed to the Iberian day-ahead market
    Spain { geo_id: i32 },
    /// The day-ahead market of an ENTSO-E bidding zone (EIC code)
    Entsoe { eic: &'static str },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Zone {
    pub code: &'static str,
    pub name: &'static str,
    /// ISO 3166 country, so the app can name it in the user's language
    pub country: &'static str,
    pub tz: Tz,
    pub market: Market,
}

const fn spain(code: &'static str, name: &'static str, geo_id: i32, tz: Tz) -> Zone {
    Zone {
        code,
        name,
        country: "ES",
        tz,
        market: Market::Spain { geo_id },
    }
}

const fn entsoe(
    code: &'static str,
    name: &'static str,
    country: &'static str,
    eic: &'static str,
    tz: Tz,
) -> Zone {
    Zone {
        code,
        name,
        country,
        tz,
        market: Market::Entsoe { eic },
    }
}

pub const ZONES: [Zone; 21] = [
    spain("peninsula", "Península", 8741, Tz::Europe__Madrid),
    spain("canarias", "Canarias", 8742, Tz::Atlantic__Canary),
    spain("balears", "Illes Balears", 8743, Tz::Europe__Madrid),
    spain("ceuta", "Ceuta", 8744, Tz::Africa__Ceuta),
    spain("melilla", "Melilla", 8745, Tz::Africa__Ceuta),
    // Euro countries whose day-ahead price ENTSO-E publishes
    entsoe(
        "pt",
        "Portugal",
        "PT",
        "10YPT-REN------W",
        Tz::Europe__Lisbon,
    ),
    entsoe("fr", "France", "FR", "10YFR-RTE------C", Tz::Europe__Paris),
    entsoe(
        "de",
        "Deutschland / Luxembourg",
        "DE",
        "10Y1001A1001A82H",
        Tz::Europe__Berlin,
    ),
    entsoe(
        "at",
        "Österreich",
        "AT",
        "10YAT-APG------L",
        Tz::Europe__Vienna,
    ),
    entsoe(
        "be",
        "Belgique / België",
        "BE",
        "10YBE----------2",
        Tz::Europe__Brussels,
    ),
    entsoe(
        "nl",
        "Nederland",
        "NL",
        "10YNL----------L",
        Tz::Europe__Amsterdam,
    ),
    entsoe(
        "ie",
        "Ireland",
        "IE",
        "10Y1001A1001A59C",
        Tz::Europe__Dublin,
    ),
    entsoe(
        "fi",
        "Suomi",
        "FI",
        "10YFI-1--------U",
        Tz::Europe__Helsinki,
    ),
    entsoe("ee", "Eesti", "EE", "10Y1001A1001A39I", Tz::Europe__Tallinn),
    entsoe("lv", "Latvija", "LV", "10YLV-1001A00074", Tz::Europe__Riga),
    entsoe(
        "lt",
        "Lietuva",
        "LT",
        "10YLT-1001A0008Q",
        Tz::Europe__Vilnius,
    ),
    entsoe(
        "sk",
        "Slovensko",
        "SK",
        "10YSK-SEPS-----K",
        Tz::Europe__Bratislava,
    ),
    entsoe(
        "si",
        "Slovenija",
        "SI",
        "10YSI-ELES-----O",
        Tz::Europe__Ljubljana,
    ),
    entsoe(
        "hr",
        "Hrvatska",
        "HR",
        "10YHR-HEP------M",
        Tz::Europe__Zagreb,
    ),
    entsoe("gr", "Elláda", "GR", "10YGR-HTSO-----Y", Tz::Europe__Athens),
    entsoe(
        "bg",
        "Bulgaria",
        "BG",
        "10YCA-BULGARIA-R",
        Tz::Europe__Sofia,
    ),
];

pub fn find(code: &str) -> Option<&'static Zone> {
    ZONES.iter().find(|z| z.code == code)
}

pub fn is_valid(code: &str) -> bool {
    find(code).is_some()
}

/// The zone, or the Peninsula for a code no longer served
pub fn get(code: &str) -> &'static Zone {
    find(code).unwrap_or(&ZONES[0])
}

pub fn from_geo_id(geo_id: i32) -> Option<&'static Zone> {
    ZONES.iter().find(|z| z.market == Market::Spain { geo_id })
}

/// ESIOS geo_ids of the Spanish zones
pub fn geo_ids() -> impl Iterator<Item = i32> {
    ZONES.iter().filter_map(|z| match z.market {
        Market::Spain { geo_id } => Some(geo_id),
        Market::Entsoe { .. } => None,
    })
}

/// The zone whose prices a user's rules follow
pub fn of_user(conn: &mut PgConnection, user_id: i32) -> &'static Zone {
    let code: String = users::table
        .find(user_id)
        .select(users::zone)
        .first(conn)
        .unwrap_or_else(|_| DEFAULT.to_string());
    get(&code)
}

impl Zone {
    pub fn is_spain(&self) -> bool {
        matches!(self.market, Market::Spain { .. })
    }

    /// Local time minus Madrid time, in hours (the same all year)
    pub fn offset_hours(&self) -> i64 {
        let now = Utc::now().naive_utc();
        let seconds = |tz: Tz| tz.offset_from_utc_datetime(&now).fix().local_minus_utc() as i64;
        (seconds(self.tz) - seconds(Madrid)) / 3600
    }

    /// Engine (Madrid) time to the zone's wall time
    pub fn to_local(&self, t: NaiveDateTime) -> NaiveDateTime {
        t + Duration::hours(self.offset_hours())
    }

    /// The zone's wall time to engine (Madrid) time
    pub fn to_engine(&self, t: NaiveDateTime) -> NaiveDateTime {
        t - Duration::hours(self.offset_hours())
    }

    /// Current wall time in the zone
    pub fn now(&self) -> NaiveDateTime {
        self.to_local(crate::time::now_naive())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn test_zones() {
        assert!(is_valid("peninsula") && is_valid("canarias") && is_valid("fi"));
        assert!(!is_valid("canaries") && !is_valid(""));
        assert_eq!(from_geo_id(8744).map(|z| z.code), Some("ceuta"));
        assert_eq!(from_geo_id(8742).map(|z| z.code), Some("canarias"));
        assert_eq!(geo_ids().count(), 5);
        assert_eq!(get("gone").code, DEFAULT);
        let codes: std::collections::HashSet<_> = ZONES.iter().map(|z| z.code).collect();
        assert_eq!(codes.len(), ZONES.len());
    }

    #[test]
    fn test_offsets() {
        let offset = |code: &str| get(code).offset_hours();
        assert_eq!(offset("peninsula"), 0);
        assert_eq!(offset("canarias"), -1);
        assert_eq!(offset("pt"), -1);
        assert_eq!(offset("ie"), -1);
        assert_eq!(offset("fi"), 1);
        assert_eq!(offset("ceuta"), 0);
    }

    /// The fixed offset is only right if each zone changes its clocks at the
    /// same instant as Madrid: check it in winter, in summer and around both
    /// changes of 2026 (29 March and 25 October, 01:00 UTC)
    #[test]
    fn test_offset_never_changes() {
        let instants = [
            (1, 15, 12),
            (3, 29, 0),
            (3, 29, 2),
            (7, 15, 12),
            (10, 25, 0),
            (10, 25, 2),
        ];
        for zone in ZONES {
            let offsets: Vec<i32> = instants
                .iter()
                .map(|&(m, d, h)| {
                    let at = NaiveDate::from_ymd_opt(2026, m, d)
                        .unwrap()
                        .and_hms_opt(h, 0, 0)
                        .unwrap();
                    zone.tz
                        .offset_from_utc_datetime(&at)
                        .fix()
                        .local_minus_utc()
                        - Madrid.offset_from_utc_datetime(&at).fix().local_minus_utc()
                })
                .collect();
            assert!(
                offsets.iter().all(|o| *o == offsets[0] && o % 3600 == 0),
                "{}: {:?}",
                zone.code,
                offsets
            );
        }
    }

    #[test]
    fn test_local_time() {
        let t = NaiveDate::from_ymd_opt(2026, 10, 24)
            .unwrap()
            .and_hms_opt(15, 0, 0)
            .unwrap();
        let canarias = get("canarias");
        assert_eq!(canarias.to_local(t).format("%H").to_string(), "14");
        assert_eq!(canarias.to_engine(canarias.to_local(t)), t);
    }
}
