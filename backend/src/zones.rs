//! Price zones of the PVPC: ESIOS publishes one price per zone (geo_id).
//! Ceuta and Melilla have their own tariff periods, so some hours differ.
//! Canarias is left out until it is clear whether ESIOS gives its prices in
//! peninsular or local time (it is one hour behind).

use crate::schema::users;
use diesel::prelude::*;

pub const DEFAULT: &str = "peninsula";

/// (code, ESIOS geo_id, name)
pub const ZONES: [(&str, i32, &str); 4] = [
    ("peninsula", 8741, "Península"),
    ("balears", 8743, "Illes Balears"),
    ("ceuta", 8744, "Ceuta"),
    ("melilla", 8745, "Melilla"),
];

pub fn is_valid(zone: &str) -> bool {
    ZONES.iter().any(|(code, ..)| *code == zone)
}

pub fn from_geo_id(geo_id: i32) -> Option<&'static str> {
    ZONES.iter().find(|(_, id, _)| *id == geo_id).map(|(code, ..)| *code)
}

pub fn geo_ids() -> impl Iterator<Item = i32> {
    ZONES.iter().map(|(_, id, _)| *id)
}

/// The zone whose prices a user's rules follow
pub fn of_user(conn: &mut PgConnection, user_id: i32) -> String {
    users::table
        .find(user_id)
        .select(users::zone)
        .first(conn)
        .unwrap_or_else(|_| DEFAULT.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zones() {
        assert!(is_valid("peninsula") && is_valid("melilla"));
        assert!(!is_valid("canarias") && !is_valid(""));
        assert_eq!(from_geo_id(8744), Some("ceuta"));
        assert_eq!(from_geo_id(8742), None);
        assert_eq!(geo_ids().count(), 4);
    }
}
