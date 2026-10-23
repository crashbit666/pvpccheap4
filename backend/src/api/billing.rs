//! Store purchases of the paid plan. The app sends the purchase token; the
//! server asks the store, stores the subscription and sets premium_until.

use crate::{
    db::DbPool,
    schema::subscriptions,
    services::{
        auth::Claims,
        google_play::{self, Subscription},
        premium,
    },
};
use actix_web::{HttpResponse, Responder, post, web};
use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
pub struct GooglePurchaseRequest {
    pub purchase_token: String,
}

/// Stores what Google says about the purchase and updates the user's
/// premium_until. A token already linked to another account is not moved.
pub fn store_subscription(
    conn: &mut PgConnection,
    user_id: i32,
    purchase_token: &str,
    sub: &Subscription,
) -> Result<(), String> {
    let owner: Option<i32> = subscriptions::table
        .filter(subscriptions::purchase_token.eq(purchase_token))
        .select(subscriptions::user_id)
        .first(conn)
        .optional()
        .map_err(|e| e.to_string())?;
    if owner.is_some_and(|owner| owner != user_id) {
        return Err("This purchase belongs to another account".to_string());
    }
    // An unpaid subscription counts as already over
    let now = crate::time::now_naive();
    let expires_at = if sub.is_paid() { sub.expires_at } else { sub.expires_at.min(now) };
    diesel::insert_into(subscriptions::table)
        .values((
            subscriptions::user_id.eq(user_id),
            subscriptions::platform.eq("google"),
            subscriptions::product_id.eq(&sub.product_id),
            subscriptions::purchase_token.eq(purchase_token),
            subscriptions::state.eq(&sub.state),
            subscriptions::expires_at.eq(expires_at),
            subscriptions::updated_at.eq(now),
        ))
        .on_conflict(subscriptions::purchase_token)
        .do_update()
        .set((
            subscriptions::product_id.eq(&sub.product_id),
            subscriptions::state.eq(&sub.state),
            subscriptions::expires_at.eq(expires_at),
            subscriptions::updated_at.eq(now),
        ))
        .execute(conn)
        .map_err(|e| e.to_string())?;
    premium::refresh_premium_until(conn, user_id).map_err(|e| e.to_string())?;
    Ok(())
}

/// A Google Play purchase made in the app
#[post("/google")]
pub async fn google_purchase(
    pool: web::Data<DbPool>,
    claims: Claims,
    body: web::Json<GooglePurchaseRequest>,
) -> impl Responder {
    let Ok(user_id) = claims.sub.parse::<i32>() else {
        return HttpResponse::BadRequest().body("Invalid user ID");
    };
    if !google_play::is_configured() {
        return HttpResponse::ServiceUnavailable().body("Purchases are not configured");
    }
    let token = body.purchase_token.trim();
    if token.is_empty() || token.len() > 4096 {
        return HttpResponse::BadRequest().body("Invalid purchase token");
    }

    let sub = match google_play::get_subscription(token).await {
        Ok(s) => s,
        Err(e) => {
            log::warn!("Could not check a purchase of user {}: {}", user_id, e);
            return HttpResponse::BadGateway().body("Could not check the purchase");
        }
    };
    // The app tags the purchase with the account that made it
    if sub.account_tag.as_deref().is_some_and(|tag| tag != google_play::account_tag(user_id)) {
        return HttpResponse::Conflict().body("This purchase belongs to another account");
    }

    {
        let Ok(mut conn) = pool.get() else {
            return HttpResponse::InternalServerError().body("Database connection error");
        };
        if let Err(e) = store_subscription(&mut conn, user_id, token, &sub) {
            return HttpResponse::Conflict().body(e);
        }
    }
    if sub.needs_acknowledge && sub.is_paid()
        && let Err(e) = google_play::acknowledge(&sub.product_id, token).await {
            // The app acknowledges too; Google refunds only after 3 days
            log::warn!("Could not acknowledge the purchase of user {}: {}", user_id, e);
        }
    log::info!("User {} bought {} ({}) until {}", user_id, sub.product_id, sub.state, sub.expires_at);

    let Ok(mut conn) = pool.get() else {
        return HttpResponse::InternalServerError().body("Database connection error");
    };
    HttpResponse::Ok().json(serde_json::json!({
        "is_premium": premium::user_has_premium(&mut conn, user_id),
        "state": sub.state,
    }))
}

/// Asks Google about one purchase and stores it. With no user_id, the user
/// comes from the account tag the app set when buying.
async fn refresh_from_google(pool: &DbPool, user_id: Option<i32>, token: &str) -> Result<(), String> {
    let sub = google_play::get_subscription(token).await?;
    let user_id = match user_id.or_else(|| sub.account_tag.as_deref().and_then(google_play::user_of_tag)) {
        Some(id) => id,
        None => {
            log::warn!("A Google Play purchase has no account; the app will send it");
            return Ok(());
        }
    };
    {
        let mut conn = pool.get().map_err(|e| e.to_string())?;
        // The tag may name a deleted account
        let exists: bool = diesel::select(diesel::dsl::exists(
            crate::schema::users::table.find(user_id),
        ))
        .get_result(&mut conn)
        .map_err(|e| e.to_string())?;
        if !exists {
            log::warn!("A Google Play purchase names user {}, who does not exist", user_id);
            return Ok(());
        }
        store_subscription(&mut conn, user_id, token, &sub)?;
    }
    if sub.needs_acknowledge && sub.is_paid()
        && let Err(e) = google_play::acknowledge(&sub.product_id, token).await {
            log::warn!("Could not acknowledge the purchase of user {}: {}", user_id, e);
        }
    log::info!("Subscription of user {} is {} until {}", user_id, sub.state, sub.expires_at);
    Ok(())
}

#[derive(Deserialize)]
pub struct NotificationQuery {
    #[serde(default)]
    pub token: String,
}

/// The purchase token of a Pub/Sub push from Google Play, or None for a test
/// or a notification that is not about a subscription
fn notification_token(body: &Value) -> Result<Option<String>, String> {
    use base64::Engine;
    let data = body["message"]["data"].as_str().ok_or("No message data")?;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|e| format!("Invalid message data: {}", e))?;
    let notification: Value = serde_json::from_slice(&decoded).map_err(|e| e.to_string())?;
    if notification.get("testNotification").is_some() {
        log::info!("Google Play test notification received");
        return Ok(None);
    }
    Ok(notification["subscriptionNotification"]["purchaseToken"]
        .as_str()
        .filter(|t| !t.is_empty() && t.len() <= 4096)
        .map(str::to_string))
}

/// Real-time developer notifications: Google Play publishes every renewal,
/// cancellation, refund or failed payment to Pub/Sub, which pushes it here.
/// The URL carries GOOGLE_PLAY_RTDN_SECRET; a non-2xx answer makes Pub/Sub retry.
#[post("/google/notifications")]
pub async fn google_notification(
    pool: web::Data<DbPool>,
    query: web::Query<NotificationQuery>,
    body: web::Json<Value>,
) -> impl Responder {
    let secret = std::env::var("GOOGLE_PLAY_RTDN_SECRET").unwrap_or_default();
    if secret.trim().is_empty() || !google_play::is_configured() {
        return HttpResponse::ServiceUnavailable().finish();
    }
    if !google_play::same_secret(query.token.as_bytes(), secret.trim().as_bytes()) {
        return HttpResponse::Forbidden().finish();
    }
    let token = match notification_token(&body) {
        Ok(Some(token)) => token,
        Ok(None) => return HttpResponse::Ok().finish(),
        Err(e) => {
            // Retrying would not fix it
            log::warn!("Unreadable Google Play notification: {}", e);
            return HttpResponse::Ok().finish();
        }
    };
    let user_id: Option<i32> = {
        let Ok(mut conn) = pool.get() else {
            return HttpResponse::ServiceUnavailable().finish();
        };
        subscriptions::table
            .filter(subscriptions::purchase_token.eq(&token))
            .select(subscriptions::user_id)
            .first(&mut conn)
            .optional()
            .unwrap_or(None)
    };
    match refresh_from_google(&pool, user_id, &token).await {
        Ok(()) => HttpResponse::Ok().finish(),
        Err(e) => {
            log::warn!("Could not handle a Google Play notification: {}", e);
            HttpResponse::ServiceUnavailable().finish()
        }
    }
}

/// Whether to ask Google again about a subscription now. Renewals happen at
/// expires_at, so one about to end is asked every few minutes until Google
/// renews or ends it; later, less often. EXPIRED is final.
fn recheck_due(state: &str, expires_at: NaiveDateTime, updated_at: NaiveDateTime, now: NaiveDateTime) -> bool {
    use chrono::Duration;
    if state == "SUBSCRIPTION_STATE_EXPIRED" || expires_at > now + Duration::minutes(5) {
        return false;
    }
    if expires_at < now - Duration::days(7) {
        return false;
    }
    let every = if expires_at > now - Duration::hours(1) {
        Duration::minutes(4)
    } else if expires_at > now - Duration::days(1) {
        Duration::hours(1)
    } else {
        Duration::hours(6)
    };
    updated_at <= now - every
}

/// Safety net for lost notifications, run every 5 minutes: subscriptions
/// that end now or just ended.
pub async fn recheck_due_subscriptions(pool: &DbPool) {
    if !google_play::is_configured() {
        return;
    }
    let now = crate::time::now_naive();
    let due: Vec<(i32, String)> = {
        let Ok(mut conn) = pool.get() else { return };
        let rows: Vec<(i32, String, String, NaiveDateTime, NaiveDateTime)> = subscriptions::table
            .filter(subscriptions::platform.eq("google"))
            .filter(subscriptions::expires_at.lt(now + chrono::Duration::minutes(5)))
            .filter(subscriptions::expires_at.gt(now - chrono::Duration::days(7)))
            .select((
                subscriptions::user_id,
                subscriptions::purchase_token,
                subscriptions::state,
                subscriptions::expires_at,
                subscriptions::updated_at,
            ))
            .load(&mut conn)
            .unwrap_or_default();
        rows.into_iter()
            .filter(|(_, _, state, expires_at, updated_at)| recheck_due(state, *expires_at, *updated_at, now))
            .map(|(user_id, token, ..)| (user_id, token))
            .collect()
    };
    for (user_id, token) in due {
        if let Err(e) = refresh_from_google(pool, Some(user_id), &token).await {
            log::warn!("Could not recheck a subscription of user {}: {}", user_id, e);
        }
    }
}

/// Asks the store again about subscriptions that end soon or just ended:
/// renewals move expires_at forward, cancellations and refunds end it.
pub async fn recheck_subscriptions(pool: &DbPool) {
    if !google_play::is_configured() {
        return;
    }
    let now = crate::time::now_naive();
    let due: Vec<(i32, String)> = {
        let Ok(mut conn) = pool.get() else { return };
        subscriptions::table
            .filter(subscriptions::platform.eq("google"))
            .filter(subscriptions::expires_at.gt(now - chrono::Duration::days(7)))
            .filter(subscriptions::expires_at.lt(now + chrono::Duration::days(2)))
            .select((subscriptions::user_id, subscriptions::purchase_token))
            .load(&mut conn)
            .unwrap_or_default()
    };
    for (user_id, token) in due {
        match google_play::get_subscription(&token).await {
            Ok(sub) => {
                let Ok(mut conn) = pool.get() else { return };
                if let Err(e) = store_subscription(&mut conn, user_id, &token, &sub) {
                    log::warn!("Could not update a subscription of user {}: {}", user_id, e);
                }
            }
            Err(e) => log::warn!("Could not recheck a subscription of user {}: {}", user_id, e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn at(h: u32, m: u32) -> NaiveDateTime {
        chrono::NaiveDate::from_ymd_opt(2026, 10, 23).unwrap().and_hms_opt(h, m, 0).unwrap()
    }

    #[test]
    fn test_recheck_due() {
        let now = at(12, 0);
        let active = "SUBSCRIPTION_STATE_ACTIVE";
        // Far from the end: the notifications and the daily check cover it
        assert!(!recheck_due(active, now + Duration::days(20), now - Duration::days(1), now));
        // Ends in 3 minutes, checked 10 minutes ago
        assert!(recheck_due(active, now + Duration::minutes(3), now - Duration::minutes(10), now));
        // Just checked
        assert!(!recheck_due(active, now - Duration::minutes(1), now - Duration::minutes(2), now));
        // Ended 5 hours ago: once an hour
        assert!(!recheck_due(active, now - Duration::hours(5), now - Duration::minutes(30), now));
        assert!(recheck_due(active, now - Duration::hours(5), now - Duration::minutes(61), now));
        // Final, or too old
        assert!(!recheck_due("SUBSCRIPTION_STATE_EXPIRED", now - Duration::minutes(1), now - Duration::hours(1), now));
        assert!(!recheck_due(active, now - Duration::days(8), now - Duration::days(1), now));
    }

    #[test]
    fn test_notification_token() {
        use base64::Engine;
        let push = |json: &str| {
            serde_json::json!({"message": {"data": base64::engine::general_purpose::STANDARD.encode(json)}})
        };
        let body = push(r#"{"version":"1.0","packageName":"p","subscriptionNotification":{"notificationType":2,"purchaseToken":"abc","subscriptionId":"premium"}}"#);
        assert_eq!(notification_token(&body).unwrap(), Some("abc".to_string()));
        assert_eq!(notification_token(&push(r#"{"testNotification":{"version":"1.0"}}"#)).unwrap(), None);
        assert!(notification_token(&serde_json::json!({})).is_err());
    }
}
