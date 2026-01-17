use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use jsonwebtoken::{EncodingKey, Header, encode};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use actix_web::{FromRequest, HttpRequest, dev::Payload, error::ErrorUnauthorized};
use jsonwebtoken::{DecodingKey, Validation, decode};
use std::future::{Ready, ready};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // Subject (User ID)
    pub exp: usize,  // Expiration
}

impl FromRequest for Claims {
    type Error = actix_web::Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        let auth_header = match req.headers().get("Authorization") {
            Some(h) => h,
            None => return ready(Err(ErrorUnauthorized("No Auth header"))),
        };

        let token_str = match auth_header.to_str() {
            Ok(s) => s.replace("Bearer ", ""),
            Err(_) => return ready(Err(ErrorUnauthorized("Invalid Auth header"))),
        };

        let secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "secret".to_string());

        match decode::<Claims>(
            &token_str,
            &DecodingKey::from_secret(secret.as_bytes()),
            &Validation::default(),
        ) {
            Ok(token_data) => ready(Ok(token_data.claims)),
            Err(_) => ready(Err(ErrorUnauthorized("Invalid Token"))),
        }
    }
}

pub fn hash_password(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| e.to_string())?
        .to_string();
    Ok(password_hash)
}

pub fn verify_password(password: &str, password_hash: &str) -> Result<bool, String> {
    let parsed_hash = PasswordHash::new(password_hash).map_err(|e| e.to_string())?;
    let argon2 = Argon2::default();
    Ok(argon2
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

pub fn create_jwt(user_id: i32) -> Result<String, String> {
    let expiration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs() as usize
        + 24 * 3600; // 24 hours

    let claims = Claims {
        sub: user_id.to_string(),
        exp: expiration,
    };

    // In production, use env var for secret!
    let secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "secret".to_string());

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_password_returns_valid_hash() {
        let password = "my_secure_password";
        let result = hash_password(password);

        assert!(result.is_ok());
        let hash = result.unwrap();
        assert!(!hash.is_empty());
        assert!(hash.starts_with("$argon2")); // Argon2 hash format
    }

    #[test]
    fn test_hash_password_produces_different_hashes_for_same_password() {
        let password = "my_secure_password";
        let hash1 = hash_password(password).unwrap();
        let hash2 = hash_password(password).unwrap();

        // Different salts should produce different hashes
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_verify_password_correct() {
        let password = "my_secure_password";
        let hash = hash_password(password).unwrap();

        let result = verify_password(password, &hash);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_verify_password_incorrect() {
        let password = "my_secure_password";
        let wrong_password = "wrong_password";
        let hash = hash_password(password).unwrap();

        let result = verify_password(wrong_password, &hash);
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_verify_password_invalid_hash() {
        let result = verify_password("password", "invalid_hash");
        assert!(result.is_err());
    }

    #[test]
    fn test_create_jwt_returns_valid_token() {
        let user_id = 42;
        let result = create_jwt(user_id);

        assert!(result.is_ok());
        let token = result.unwrap();
        assert!(!token.is_empty());
        // JWT has 3 parts separated by dots
        assert_eq!(token.split('.').count(), 3);
    }

    #[test]
    fn test_create_jwt_contains_correct_user_id() {
        let user_id = 123;
        let token = create_jwt(user_id).unwrap();

        let secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "secret".to_string());
        let token_data = decode::<Claims>(
            &token,
            &DecodingKey::from_secret(secret.as_bytes()),
            &Validation::default(),
        )
        .unwrap();

        assert_eq!(token_data.claims.sub, "123");
    }

    #[test]
    fn test_jwt_expiration_is_in_future() {
        let user_id = 1;
        let token = create_jwt(user_id).unwrap();

        let secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "secret".to_string());
        let token_data = decode::<Claims>(
            &token,
            &DecodingKey::from_secret(secret.as_bytes()),
            &Validation::default(),
        )
        .unwrap();

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as usize;

        assert!(token_data.claims.exp > now);
        // Should be approximately 24 hours in the future
        assert!(token_data.claims.exp <= now + 24 * 3600 + 1);
    }
}
