//! The paid plan: more than FREE_RULES rules and rules written in plain
//! words. A user has it while a store subscription is paid (premium_until)
//! or when it was granted by hand (is_premium).

use crate::models::User;
use crate::schema::{subscriptions, users};
use chrono::NaiveDateTime;
use diesel::prelude::*;

/// Rules a user without the paid plan can have
pub const FREE_RULES: i64 = 4;

pub fn has_premium(user: &User, now: NaiveDateTime) -> bool {
    user.is_premium || user.premium_until.is_some_and(|until| until > now)
}

pub fn user_has_premium(conn: &mut PgConnection, user_id: i32) -> bool {
    users::table
        .find(user_id)
        .first::<User>(conn)
        .is_ok_and(|user| has_premium(&user, crate::time::now_naive()))
}

/// Sets premium_until to the latest end of the user's paid subscriptions
pub fn refresh_premium_until(
    conn: &mut PgConnection,
    user_id: i32,
) -> QueryResult<Option<NaiveDateTime>> {
    let until: Option<NaiveDateTime> = subscriptions::table
        .filter(subscriptions::user_id.eq(user_id))
        .select(diesel::dsl::max(subscriptions::expires_at))
        .first(conn)?;
    diesel::update(users::table.find(user_id))
        .set(users::premium_until.eq(until))
        .execute(conn)?;
    Ok(until)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(is_premium: bool, premium_until: Option<NaiveDateTime>) -> User {
        User {
            id: 1,
            username: "u".to_string(),
            password_hash: String::new(),
            created_at: NaiveDateTime::default(),
            token_version: 0,
            is_premium,
            email: None,
            email_verified: false,
            zone: "peninsula".to_string(),
            premium_until,
            tariff: "pvpc".to_string(),
            tariff_margin: 0.0,
        }
    }

    #[test]
    fn test_has_premium() {
        let now = chrono::NaiveDate::from_ymd_opt(2026, 10, 23)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap();
        assert!(!has_premium(&user(false, None), now));
        assert!(has_premium(&user(true, None), now));
        assert!(has_premium(
            &user(false, Some(now + chrono::Duration::days(3))),
            now
        ));
        assert!(!has_premium(
            &user(false, Some(now - chrono::Duration::days(1))),
            now
        ));
    }
}
