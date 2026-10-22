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
use diesel::prelude::*;
use serde::Deserialize;

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
    if sub.needs_acknowledge && sub.is_paid() {
        if let Err(e) = google_play::acknowledge(&sub.product_id, token).await {
            // The app acknowledges too; Google refunds only after 3 days
            log::warn!("Could not acknowledge the purchase of user {}: {}", user_id, e);
        }
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
