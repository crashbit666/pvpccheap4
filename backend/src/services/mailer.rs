//! Sends the account emails (verify the address, reset the password)
//! through Brevo's transactional API.
//!
//! BREVO_API_KEY, MAIL_FROM (a sender verified in Brevo) and PUBLIC_URL (the
//! address the links point to) must be set; MAIL_FROM_NAME is optional.

use serde_json::json;
use std::time::Duration;

const API_URL: &str = "https://api.brevo.com/v3/smtp/email";

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

pub fn is_configured() -> bool {
    env("BREVO_API_KEY").is_some() && env("MAIL_FROM").is_some() && env("PUBLIC_URL").is_some()
}

/// "https://pvpccheap.crashbit.cat" + path
pub fn public_link(path: &str) -> Option<String> {
    env("PUBLIC_URL").map(|base| format!("{}/{}", base.trim_end_matches('/'), path.trim_start_matches('/')))
}

pub async fn send(to: &str, subject: &str, text: &str) -> Result<(), String> {
    let (Some(key), Some(from)) = (env("BREVO_API_KEY"), env("MAIL_FROM")) else {
        return Err("Email is not configured".to_string());
    };
    let name = env("MAIL_FROM_NAME").unwrap_or_else(|| "PVPC Cheap".to_string());
    let response = reqwest::Client::new()
        .post(API_URL)
        .timeout(Duration::from_secs(20))
        .header("api-key", key)
        .json(&json!({
            "sender": {"name": name, "email": from},
            "to": [{"email": to}],
            "subject": subject,
            "textContent": text,
        }))
        .send()
        .await
        .map_err(|e| e.without_url().to_string())?;
    if response.status().is_success() {
        Ok(())
    } else {
        Err(format!("Brevo answered {}", response.status()))
    }
}
