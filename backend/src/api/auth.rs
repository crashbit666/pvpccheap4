use crate::{db::DbPool, models::User, schema::users, services::auth};
use actix_web::{HttpResponse, Responder, post, web};
use diesel::prelude::*;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct AuthRequest {
    pub username: String,
    pub password: String,
}

#[post("/register")]
pub async fn register(pool: web::Data<DbPool>, item: web::Json<AuthRequest>) -> impl Responder {
    let mut conn = pool.get().expect("Couldn't get db connection");

    // Hash password
    let hashed = match auth::hash_password(&item.password) {
        Ok(h) => h,
        Err(_) => return HttpResponse::InternalServerError().body("Error hashing password"),
    };

    // Insert user
    let new_user = diesel::insert_into(users::table)
        .values((
            users::username.eq(&item.username),
            users::password_hash.eq(&hashed),
        ))
        .get_result::<User>(&mut conn);

    match new_user {
        Ok(user) => {
            HttpResponse::Ok().json(serde_json::json!({"id": user.id, "username": user.username}))
        }
        Err(_) => HttpResponse::Conflict().body("Username already exists"),
    }
}

#[post("/login")]
pub async fn login(pool: web::Data<DbPool>, item: web::Json<AuthRequest>) -> impl Responder {
    let mut conn = pool.get().expect("Couldn't get db connection");

    // Find user
    let user_result = users::table
        .filter(users::username.eq(&item.username))
        .first::<User>(&mut conn);

    match user_result {
        Ok(user) => {
            // Verify password
            match auth::verify_password(&item.password, &user.password_hash) {
                Ok(true) => {
                    // Generate JWT
                    match auth::create_jwt(user.id) {
                        Ok(token) => HttpResponse::Ok().json(serde_json::json!({"token": token})),
                        Err(_) => HttpResponse::InternalServerError().body("Error creating token"),
                    }
                }
                _ => HttpResponse::Unauthorized().body("Invalid credentials"),
            }
        }
        Err(_) => HttpResponse::Unauthorized().body("Invalid credentials"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_request_deserialization() {
        let json = r#"{"username": "testuser", "password": "testpass"}"#;
        let request: AuthRequest = serde_json::from_str(json).unwrap();

        assert_eq!(request.username, "testuser");
        assert_eq!(request.password, "testpass");
    }

    #[test]
    fn test_auth_request_with_special_characters() {
        let json = r#"{"username": "user@example.com", "password": "p@ss!w0rd#123"}"#;
        let request: AuthRequest = serde_json::from_str(json).unwrap();

        assert_eq!(request.username, "user@example.com");
        assert_eq!(request.password, "p@ss!w0rd#123");
    }

    #[test]
    fn test_auth_request_with_unicode() {
        let json = r#"{"username": "usuari_català", "password": "contraseña123"}"#;
        let request: AuthRequest = serde_json::from_str(json).unwrap();

        assert_eq!(request.username, "usuari_català");
        assert_eq!(request.password, "contraseña123");
    }

    #[test]
    fn test_auth_request_missing_field_fails() {
        let json = r#"{"username": "testuser"}"#;
        let result: Result<AuthRequest, _> = serde_json::from_str(json);

        assert!(result.is_err());
    }

    #[test]
    fn test_auth_request_empty_strings() {
        let json = r#"{"username": "", "password": ""}"#;
        let request: AuthRequest = serde_json::from_str(json).unwrap();

        assert_eq!(request.username, "");
        assert_eq!(request.password, "");
    }

    #[test]
    fn test_auth_request_long_values() {
        let long_username = "a".repeat(255);
        let long_password = "b".repeat(1000);
        let json = format!(
            r#"{{"username": "{}", "password": "{}"}}"#,
            long_username, long_password
        );
        let request: AuthRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(request.username.len(), 255);
        assert_eq!(request.password.len(), 1000);
    }
}
