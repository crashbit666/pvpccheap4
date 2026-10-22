use crate::{
    db::DbPool,
    integrations::ProviderRegistry,
    models::User,
    schema::{devices, user_integrations, users},
    services::auth::{self, Claims},
};
use actix_web::{HttpResponse, Responder, delete, get, post, put, web};
use diesel::prelude::*;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct AuthRequest {
    pub username: String,
    pub password: String,
}

pub const MIN_PASSWORD_LEN: usize = 8;

fn password_too_short(password: &str) -> bool {
    password.chars().count() < MIN_PASSWORD_LEN
}

#[post("/register")]
pub async fn register(pool: web::Data<DbPool>, item: web::Json<AuthRequest>) -> impl Responder {
    if item.username.trim().is_empty() {
        return HttpResponse::BadRequest().body("Username is required");
    }
    if password_too_short(&item.password) {
        return HttpResponse::BadRequest()
            .body(format!("Password must have at least {} characters", MIN_PASSWORD_LEN));
    }
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
                    match auth::create_jwt(user.id, user.token_version) {
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

/// The logged-in user and what their plan unlocks
#[get("/me")]
pub async fn me(pool: web::Data<DbPool>, claims: Claims) -> impl Responder {
    let Ok(user_id) = claims.sub.parse::<i32>() else {
        return HttpResponse::BadRequest().body("Invalid user ID");
    };
    let Ok(mut conn) = pool.get() else {
        return HttpResponse::InternalServerError().body("Database connection error");
    };
    match users::table.find(user_id).first::<User>(&mut conn) {
        Ok(user) => HttpResponse::Ok().json(serde_json::json!({
            "id": user.id,
            "username": user.username,
            "is_premium": user.is_premium,
            // Rules written in plain words: paid, and the server needs an API key
            "ai_rules_available": crate::services::rule_assistant::is_configured(),
        })),
        Err(_) => HttpResponse::NotFound().body("User not found"),
    }
}

/// Loads the logged-in user and checks their password.
fn authenticated_user(
    conn: &mut PgConnection,
    claims: &Claims,
    password: &str,
) -> Result<User, HttpResponse> {
    let user_id: i32 = claims
        .sub
        .parse()
        .map_err(|_| HttpResponse::BadRequest().body("Invalid user ID"))?;
    let user = users::table
        .find(user_id)
        .first::<User>(conn)
        .map_err(|_| HttpResponse::NotFound().body("User not found"))?;
    match auth::verify_password(password, &user.password_hash) {
        Ok(true) => Ok(user),
        _ => Err(HttpResponse::Forbidden().body("Wrong password")),
    }
}

#[derive(Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[put("/password")]
pub async fn change_password(
    pool: web::Data<DbPool>,
    claims: Claims,
    item: web::Json<ChangePasswordRequest>,
) -> impl Responder {
    if password_too_short(&item.new_password) {
        return HttpResponse::BadRequest()
            .body(format!("Password must have at least {} characters", MIN_PASSWORD_LEN));
    }
    let Ok(mut conn) = pool.get() else {
        return HttpResponse::InternalServerError().body("Database connection error");
    };
    let user = match authenticated_user(&mut conn, &claims, &item.current_password) {
        Ok(u) => u,
        Err(response) => return response,
    };
    let Ok(hashed) = auth::hash_password(&item.new_password) else {
        return HttpResponse::InternalServerError().body("Error hashing password");
    };
    // Bumping the version logs out every session; this one gets a new token
    let version = user.token_version + 1;
    if diesel::update(users::table.find(user.id))
        .set((users::password_hash.eq(hashed), users::token_version.eq(version)))
        .execute(&mut conn)
        .is_err()
    {
        return HttpResponse::InternalServerError().body("Error saving password");
    }
    match auth::create_jwt(user.id, version) {
        Ok(token) => HttpResponse::Ok().json(serde_json::json!({"changed": true, "token": token})),
        Err(_) => HttpResponse::InternalServerError().body("Error creating token"),
    }
}

#[derive(Deserialize)]
pub struct DeleteAccountRequest {
    pub password: String,
}

/// Deletes the account and everything in it (integrations, devices, rules,
/// history). Devices that a rule switched on this hour are turned off first.
#[delete("/account")]
pub async fn delete_account(
    pool: web::Data<DbPool>,
    registry: web::Data<ProviderRegistry>,
    claims: Claims,
    item: web::Json<DeleteAccountRequest>,
) -> impl Responder {
    let Ok(mut conn) = pool.get() else {
        return HttpResponse::InternalServerError().body("Database connection error");
    };
    let user = match authenticated_user(&mut conn, &claims, &item.password) {
        Ok(u) => u,
        Err(response) => return response,
    };

    let device_ids: Vec<i32> = devices::table
        .inner_join(user_integrations::table)
        .filter(user_integrations::user_id.eq(user.id))
        .select(devices::id)
        .load(&mut conn)
        .unwrap_or_default();
    drop(conn);
    super::rules::release_devices(&pool, &registry, &device_ids).await;

    let Ok(mut conn) = pool.get() else {
        return HttpResponse::InternalServerError().body("Database connection error");
    };
    // Everything else goes with ON DELETE CASCADE
    match diesel::delete(users::table.find(user.id)).execute(&mut conn) {
        Ok(_) => {
            log::info!("Deleted account {}", user.id);
            HttpResponse::Ok().json(serde_json::json!({"deleted": true}))
        }
        Err(_) => HttpResponse::InternalServerError().body("Error deleting account"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_password_minimum_length() {
        assert!(password_too_short("1234567"));
        assert!(!password_too_short("12345678"));
        // Counted in characters, not bytes
        assert!(password_too_short("àèòàèòà"));
    }

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
