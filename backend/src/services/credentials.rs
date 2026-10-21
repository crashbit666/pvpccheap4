//! Encryption at rest of the provider credentials (email, password, tokens)
//! stored in `user_integrations.credentials_json`.
//!
//! Stored value: `enc:v1:` + base64(nonce ‖ AES-256-GCM ciphertext). The key is
//! CREDENTIALS_KEY (64 hex chars, `openssl rand -hex 32`). Rows written before
//! encryption existed are plain JSON: they are still read, and
//! `encrypt_legacy_rows` rewrites them encrypted.

use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, Generate, Key, KeyInit},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use diesel::prelude::*;
use log::info;
use serde_json::Value;
use std::sync::OnceLock;

use crate::schema::user_integrations;

const PREFIX: &str = "enc:v1:";
const NONCE_LEN: usize = 12;

fn parse_key(hex_key: &str) -> Result<Key<Aes256Gcm>, String> {
    let bytes = hex::decode(hex_key.trim()).map_err(|_| "CREDENTIALS_KEY must be hex".to_string())?;
    Key::<Aes256Gcm>::try_from(bytes.as_slice())
        .map_err(|_| "CREDENTIALS_KEY must be 32 bytes (64 hex chars)".to_string())
}

fn cipher() -> &'static Aes256Gcm {
    static CIPHER: OnceLock<Aes256Gcm> = OnceLock::new();
    CIPHER.get_or_init(|| {
        #[cfg(test)]
        let hex_key = std::env::var("CREDENTIALS_KEY").unwrap_or_else(|_| "11".repeat(32));
        #[cfg(not(test))]
        let hex_key = std::env::var("CREDENTIALS_KEY").expect("CREDENTIALS_KEY must be set");
        Aes256Gcm::new(&parse_key(&hex_key).expect("invalid CREDENTIALS_KEY"))
    })
}

/// Panics at startup if CREDENTIALS_KEY is missing or malformed, so a bad
/// deploy fails at once instead of at the first device action.
pub fn ensure_credentials_key() {
    let key = std::env::var("CREDENTIALS_KEY").unwrap_or_default();
    if let Err(e) = parse_key(&key) {
        panic!("{}", e);
    }
    cipher();
}

pub fn encrypt(credentials: &Value) -> String {
    let nonce = Nonce::generate();
    let ciphertext = cipher()
        .encrypt(&nonce, credentials.to_string().as_bytes())
        .expect("AES-GCM encryption cannot fail for in-memory data");
    let mut blob = nonce.to_vec();
    blob.extend_from_slice(&ciphertext);
    format!("{}{}", PREFIX, STANDARD.encode(blob))
}

pub fn decrypt(stored: &str) -> Result<Value, String> {
    let Some(encoded) = stored.strip_prefix(PREFIX) else {
        // Legacy plain JSON row
        return serde_json::from_str(stored).map_err(|e| format!("invalid credentials: {}", e));
    };
    let blob = STANDARD.decode(encoded).map_err(|_| "invalid credentials encoding".to_string())?;
    if blob.len() <= NONCE_LEN {
        return Err("invalid credentials length".to_string());
    }
    let (nonce, ciphertext) = blob.split_at(NONCE_LEN);
    let nonce = Nonce::try_from(nonce).map_err(|_| "invalid credentials nonce".to_string())?;
    let plain = cipher()
        .decrypt(&nonce, ciphertext)
        .map_err(|_| "credentials cannot be decrypted (wrong CREDENTIALS_KEY?)".to_string())?;
    serde_json::from_slice(&plain).map_err(|e| format!("invalid credentials: {}", e))
}

/// Encrypt the rows still stored as plain JSON. Idempotent.
pub fn encrypt_legacy_rows(conn: &mut PgConnection) -> QueryResult<usize> {
    let rows: Vec<(i32, String)> = user_integrations::table
        .filter(user_integrations::credentials_json.not_like(format!("{}%", PREFIX)))
        .select((user_integrations::id, user_integrations::credentials_json))
        .load(conn)?;
    let mut done = 0;
    for (id, plain) in rows {
        let Ok(value) = serde_json::from_str::<Value>(&plain) else {
            log::warn!("Integration {} has unreadable credentials, left as is", id);
            continue;
        };
        diesel::update(user_integrations::table.find(id))
            .set(user_integrations::credentials_json.eq(encrypt(&value)))
            .execute(conn)?;
        done += 1;
    }
    if done > 0 {
        info!("Encrypted the credentials of {} integrations", done);
    }
    Ok(done)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_round_trip() {
        let creds = json!({"email": "a@b.c", "password": "secret", "token": "t"});
        let stored = encrypt(&creds);
        assert!(stored.starts_with(PREFIX));
        assert!(!stored.contains("secret"));
        assert_eq!(decrypt(&stored).unwrap(), creds);
    }

    #[test]
    fn test_nonce_is_random() {
        let creds = json!({"password": "secret"});
        assert_ne!(encrypt(&creds), encrypt(&creds));
    }

    #[test]
    fn test_reads_legacy_plain_json() {
        assert_eq!(decrypt(r#"{"token":"x"}"#).unwrap(), json!({"token": "x"}));
    }

    #[test]
    fn test_tampered_value_is_rejected() {
        let stored = encrypt(&json!({"password": "secret"}));
        let mut blob = STANDARD.decode(stored.strip_prefix(PREFIX).unwrap()).unwrap();
        let last = blob.len() - 1;
        blob[last] ^= 1;
        assert!(decrypt(&format!("{}{}", PREFIX, STANDARD.encode(blob))).is_err());
    }

    #[test]
    fn test_key_validation() {
        assert!(parse_key(&"ab".repeat(32)).is_ok());
        assert!(parse_key("short").is_err());
        assert!(parse_key(&"zz".repeat(32)).is_err());
    }
}
