//! Single-use links sent by email. The token travels in the link; only its
//! SHA-256 is stored, so a copy of the database does not open any account.

use crate::schema::user_tokens;
use chrono::{Duration, NaiveDateTime};
use diesel::prelude::*;
use rand::RngExt;
use sha2::{Digest, Sha256};

pub const VERIFY_EMAIL: &str = "verify_email";
pub const RESET_PASSWORD: &str = "reset_password";

fn hash(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

/// Creates a token for `user_id` and returns it (to put in the link)
pub fn create(
    conn: &mut PgConnection,
    user_id: i32,
    kind: &str,
    email: &str,
    valid_for: Duration,
) -> QueryResult<String> {
    let bytes: [u8; 32] = rand::rng().random();
    let token = hex::encode(bytes);
    let now = crate::time::now_naive();
    diesel::insert_into(user_tokens::table)
        .values((
            user_tokens::user_id.eq(user_id),
            user_tokens::kind.eq(kind),
            user_tokens::token_hash.eq(hash(&token)),
            user_tokens::email.eq(email),
            user_tokens::created_at.eq(now),
            user_tokens::expires_at.eq(now + valid_for),
        ))
        .execute(conn)?;
    Ok(token)
}

/// The user and email of a token that is still valid, without using it
pub fn peek(conn: &mut PgConnection, kind: &str, token: &str) -> Option<(i32, String)> {
    user_tokens::table
        .filter(user_tokens::kind.eq(kind))
        .filter(user_tokens::token_hash.eq(hash(token)))
        .filter(user_tokens::used_at.is_null())
        .filter(user_tokens::expires_at.gt(crate::time::now_naive()))
        .select((user_tokens::user_id, user_tokens::email))
        .first(conn)
        .ok()
}

/// Uses the token: valid only once. Returns its user and email.
pub fn consume(conn: &mut PgConnection, kind: &str, token: &str) -> Option<(i32, String)> {
    let now = crate::time::now_naive();
    diesel::update(
        user_tokens::table
            .filter(user_tokens::kind.eq(kind))
            .filter(user_tokens::token_hash.eq(hash(token)))
            .filter(user_tokens::used_at.is_null())
            .filter(user_tokens::expires_at.gt(now)),
    )
    .set(user_tokens::used_at.eq(now))
    .returning((user_tokens::user_id, user_tokens::email))
    .get_result(conn)
    .ok()
}

/// When the user last got a token of this kind (to avoid sending emails in a loop)
pub fn last_sent(conn: &mut PgConnection, user_id: i32, kind: &str) -> Option<NaiveDateTime> {
    user_tokens::table
        .filter(user_tokens::user_id.eq(user_id))
        .filter(user_tokens::kind.eq(kind))
        .select(diesel::dsl::max(user_tokens::created_at))
        .first::<Option<NaiveDateTime>>(conn)
        .ok()
        .flatten()
}

/// Marks the user's unused tokens of this kind as used (a new one replaces them)
pub fn invalidate(conn: &mut PgConnection, user_id: i32, kind: &str) -> QueryResult<usize> {
    diesel::update(
        user_tokens::table
            .filter(user_tokens::user_id.eq(user_id))
            .filter(user_tokens::kind.eq(kind))
            .filter(user_tokens::used_at.is_null()),
    )
    .set(user_tokens::used_at.eq(crate::time::now_naive()))
    .execute(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_is_stable_and_not_the_token() {
        let h = hash("abc");
        assert_eq!(h, hash("abc"));
        assert_ne!(h, "abc");
        assert_eq!(h.len(), 64);
    }
}
