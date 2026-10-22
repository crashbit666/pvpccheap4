//! Checks Google Play subscriptions with the Android Publisher API, with a
//! service account of the Play Console.
//!
//! GOOGLE_PLAY_SERVICE_ACCOUNT is the service account's JSON key in base64
//! (one line, so it fits in the env files); GOOGLE_PLAY_PACKAGE the app's
//! package (com.crashbit.pvpccheap4 by default).

use chrono::{DateTime, NaiveDateTime, Utc};
use chrono_tz::Europe::Madrid;
use serde::Deserialize;
use serde_json::Value;
use std::sync::Mutex;
use std::time::Duration;

const SCOPE: &str = "https://www.googleapis.com/auth/androidpublisher";
const DEFAULT_PACKAGE: &str = "com.crashbit.pvpccheap4";

#[derive(Deserialize)]
struct ServiceAccount {
    client_email: String,
    private_key: String,
    token_uri: String,
}

fn service_account() -> Result<ServiceAccount, String> {
    use base64::Engine;
    let encoded = std::env::var("GOOGLE_PLAY_SERVICE_ACCOUNT").map_err(|_| "Google Play is not configured".to_string())?;
    let json = base64::engine::general_purpose::STANDARD
        .decode(encoded.trim())
        .map_err(|e| format!("The service account is not valid base64: {}", e))?;
    serde_json::from_slice(&json).map_err(|e| format!("Invalid service account: {}", e))
}

pub fn is_configured() -> bool {
    std::env::var("GOOGLE_PLAY_SERVICE_ACCOUNT").is_ok_and(|v| !v.trim().is_empty())
}

fn package() -> String {
    std::env::var("GOOGLE_PLAY_PACKAGE").unwrap_or_else(|_| DEFAULT_PACKAGE.to_string())
}

/// OAuth token of the service account, reused until it is about to expire
async fn access_token() -> Result<String, String> {
    static CACHE: Mutex<Option<(String, i64)>> = Mutex::new(None);
    let now = Utc::now().timestamp();
    if let Some((token, expires)) = CACHE.lock().unwrap_or_else(|e| e.into_inner()).clone() {
        if expires - 60 > now {
            return Ok(token);
        }
    }

    let account = service_account()?;
    let claims = serde_json::json!({
        "iss": account.client_email,
        "scope": SCOPE,
        "aud": account.token_uri,
        "iat": now,
        "exp": now + 3600,
    });
    let key = jsonwebtoken::EncodingKey::from_rsa_pem(account.private_key.as_bytes())
        .map_err(|e| format!("Invalid service account key: {}", e))?;
    let assertion = jsonwebtoken::encode(&jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256), &claims, &key)
        .map_err(|e| e.to_string())?;

    let response: Value = reqwest::Client::new()
        .post(&account.token_uri)
        .timeout(Duration::from_secs(20))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(format!(
            "grant_type={}&assertion={}",
            urlencoding::encode("urn:ietf:params:oauth:grant-type:jwt-bearer"),
            urlencoding::encode(&assertion)
        ))
        .send()
        .await
        .map_err(|e| e.without_url().to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let token = response["access_token"].as_str().ok_or("Google did not give an access token")?.to_string();
    let expires = now + response["expires_in"].as_i64().unwrap_or(3600);
    *CACHE.lock().unwrap_or_else(|e| e.into_inner()) = Some((token.clone(), expires));
    Ok(token)
}

/// What Google says about a subscription purchase
#[derive(Debug, PartialEq)]
pub struct Subscription {
    pub product_id: String,
    /// SUBSCRIPTION_STATE_ACTIVE, ..._CANCELED, ..._EXPIRED...
    pub state: String,
    /// Madrid time, like the rest of the database
    pub expires_at: NaiveDateTime,
    pub needs_acknowledge: bool,
    /// The account id the app passed when buying ("u<user id>")
    pub account_tag: Option<String>,
}

impl Subscription {
    /// Paid until expires_at: active, in grace period, or cancelled but not over
    pub fn is_paid(&self) -> bool {
        matches!(
            self.state.as_str(),
            "SUBSCRIPTION_STATE_ACTIVE" | "SUBSCRIPTION_STATE_IN_GRACE_PERIOD" | "SUBSCRIPTION_STATE_CANCELED"
        )
    }
}

/// The account tag the app sends with the purchase, to bind it to the user
pub fn account_tag(user_id: i32) -> String {
    format!("u{}", user_id)
}

/// Reads a purchases.subscriptionsv2 resource
pub fn parse_subscription(body: &Value) -> Result<Subscription, String> {
    let item = body["lineItems"]
        .as_array()
        .and_then(|items| {
            items.iter().max_by_key(|i| i["expiryTime"].as_str().unwrap_or_default().to_string())
        })
        .ok_or("The subscription has no line items")?;
    let expiry = item["expiryTime"].as_str().ok_or("No expiry time")?;
    let expires_at = DateTime::parse_from_rfc3339(expiry)
        .map_err(|e| format!("Invalid expiry time: {}", e))?
        .with_timezone(&Madrid)
        .naive_local();
    Ok(Subscription {
        product_id: item["productId"].as_str().unwrap_or_default().to_string(),
        state: body["subscriptionState"].as_str().unwrap_or("SUBSCRIPTION_STATE_UNSPECIFIED").to_string(),
        expires_at,
        needs_acknowledge: body["acknowledgementState"] == "ACKNOWLEDGEMENT_STATE_PENDING",
        account_tag: body["externalAccountIdentifiers"]["obfuscatedExternalAccountId"]
            .as_str()
            .map(str::to_string),
    })
}

pub async fn get_subscription(purchase_token: &str) -> Result<Subscription, String> {
    let url = format!(
        "https://androidpublisher.googleapis.com/androidpublisher/v3/applications/{}/purchases/subscriptionsv2/tokens/{}",
        package(),
        urlencoding::encode(purchase_token)
    );
    let response = reqwest::Client::new()
        .get(url)
        .timeout(Duration::from_secs(20))
        .bearer_auth(access_token().await?)
        .send()
        .await
        .map_err(|e| e.without_url().to_string())?;
    let status = response.status();
    let body: Value = response.json().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("Google Play answered {}: {}", status, body["error"]["message"]));
    }
    parse_subscription(&body)
}

/// Purchases not acknowledged in 3 days are refunded by Google
pub async fn acknowledge(product_id: &str, purchase_token: &str) -> Result<(), String> {
    let url = format!(
        "https://androidpublisher.googleapis.com/androidpublisher/v3/applications/{}/purchases/subscriptions/{}/tokens/{}:acknowledge",
        package(),
        urlencoding::encode(product_id),
        urlencoding::encode(purchase_token)
    );
    let response = reqwest::Client::new()
        .post(url)
        .timeout(Duration::from_secs(20))
        .bearer_auth(access_token().await?)
        .json(&serde_json::json!({}))
        .send()
        .await
        .map_err(|e| e.without_url().to_string())?;
    if response.status().is_success() {
        Ok(())
    } else {
        Err(format!("Google Play answered {} to the acknowledge", response.status()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_subscription() {
        let body = serde_json::json!({
            "subscriptionState": "SUBSCRIPTION_STATE_ACTIVE",
            "acknowledgementState": "ACKNOWLEDGEMENT_STATE_PENDING",
            "externalAccountIdentifiers": {"obfuscatedExternalAccountId": "u7"},
            "lineItems": [{"productId": "premium", "expiryTime": "2026-11-23T10:00:00.000Z"}]
        });
        let sub = parse_subscription(&body).unwrap();
        assert_eq!(sub.product_id, "premium");
        assert!(sub.is_paid() && sub.needs_acknowledge);
        assert_eq!(sub.account_tag.as_deref(), Some("u7"));
        // 10:00 UTC is 11:00 in Madrid in November
        assert_eq!(sub.expires_at.to_string(), "2026-11-23 11:00:00");
    }

    #[test]
    fn test_expired_or_on_hold_is_not_paid() {
        for state in ["SUBSCRIPTION_STATE_EXPIRED", "SUBSCRIPTION_STATE_ON_HOLD", "SUBSCRIPTION_STATE_PAUSED"] {
            let body = serde_json::json!({
                "subscriptionState": state,
                "lineItems": [{"productId": "premium", "expiryTime": "2026-11-23T10:00:00Z"}]
            });
            assert!(!parse_subscription(&body).unwrap().is_paid(), "{state}");
        }
        assert!(parse_subscription(&serde_json::json!({"lineItems": []})).is_err());
    }
}
