use crate::{
    db::DbPool,
    integrations::ProviderRegistry,
    models::User,
    schema::{automation_rules, devices, user_integrations, users},
    services::auth::{self, Claims},
    services::{mailer, schedule_computation::ScheduleComputationService, user_tokens},
};
use actix_web::{HttpResponse, Responder, delete, get, post, put, web};
use diesel::prelude::*;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct AuthRequest {
    pub username: String,
    pub password: String,
    /// To recover the password; optional so older apps can still register
    #[serde(default)]
    pub email: Option<String>,
}

/// Enough to catch typos; the verification email does the rest
fn valid_email(email: &str) -> bool {
    let Some((local, domain)) = email.split_once('@') else { return false };
    email.len() <= 254
        && !local.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !email.chars().any(|c| c.is_whitespace() || c == '<' || c == '>')
        && !domain.contains('@')
}

/// Whether another account already uses this address
fn email_taken(conn: &mut PgConnection, email: &str, except_user: Option<i32>) -> bool {
    users::table
        .filter(diesel::dsl::sql::<diesel::sql_types::Bool>("lower(email) = lower(")
            .bind::<diesel::sql_types::Text, _>(email)
            .sql(")"))
        .filter(users::id.ne(except_user.unwrap_or(-1)))
        .select(users::id)
        .first::<i32>(conn)
        .is_ok()
}

/// Sends the link that verifies `email`. Errors are logged: the account
/// works without it, and the user can ask again.
async fn send_verification(pool: &DbPool, user_id: i32, email: &str) -> bool {
    if !mailer::is_configured() {
        log::warn!("Email not configured: no verification for user {}", user_id);
        return false;
    }
    let token = {
        let Ok(mut conn) = pool.get() else { return false };
        let _ = user_tokens::invalidate(&mut conn, user_id, user_tokens::VERIFY_EMAIL);
        match user_tokens::create(&mut conn, user_id, user_tokens::VERIFY_EMAIL, email, chrono::Duration::days(2)) {
            Ok(t) => t,
            Err(e) => {
                log::warn!("Could not create a verification token: {}", e);
                return false;
            }
        }
    };
    let Some(link) = mailer::public_link(&format!("verificar-correu?token={}", token)) else { return false };
    let text = format!(
        "Hola,\n\nPer confirmar que aquest és el correu del teu compte de PVPC Cheap, obre aquest enllaç:\n\n{}\n\n\
         Serveix per recuperar la contrasenya si mai l'oblides. Caduca d'aquí a 2 dies.\n\n\
         Si no has estat tu, no cal que facis res.\n",
        link
    );
    match mailer::send(email, "Confirma el teu correu", &text).await {
        Ok(()) => true,
        Err(e) => {
            log::warn!("Could not send the verification email of user {}: {}", user_id, e);
            false
        }
    }
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
    let email = item.email.as_deref().map(str::trim).filter(|e| !e.is_empty());
    if email.is_some_and(|e| !valid_email(e)) {
        return HttpResponse::BadRequest().body("Invalid email");
    }
    let mut conn = pool.get().expect("Couldn't get db connection");
    if email.is_some_and(|e| email_taken(&mut conn, e, None)) {
        return HttpResponse::Conflict().body("Email already in use");
    }

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
            users::email.eq(email),
        ))
        .get_result::<User>(&mut conn);

    match new_user {
        Ok(user) => {
            drop(conn);
            if let Some(email) = email {
                send_verification(&pool, user.id, email).await;
            }
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
            "email": user.email,
            "email_verified": user.email_verified,
            "zone": user.zone,
            // Rules written in plain words: paid, and the server needs an API key
            "ai_rules_available": crate::services::rule_assistant::is_configured(),
        })),
        Err(_) => HttpResponse::NotFound().body("User not found"),
    }
}

#[derive(Deserialize)]
pub struct ChangeEmailRequest {
    pub email: String,
    pub password: String,
}

/// Sets the account's email (unverified) and sends the link that verifies
/// it. Sending the same address again resends the link.
#[put("/email")]
pub async fn change_email(pool: web::Data<DbPool>, claims: Claims, item: web::Json<ChangeEmailRequest>) -> impl Responder {
    let email = item.email.trim();
    if !valid_email(email) {
        return HttpResponse::BadRequest().body("Invalid email");
    }
    let user = {
        let Ok(mut conn) = pool.get() else {
            return HttpResponse::InternalServerError().body("Database connection error");
        };
        let user = match authenticated_user(&mut conn, &claims, &item.password) {
            Ok(u) => u,
            Err(response) => return response,
        };
        if email_taken(&mut conn, email, Some(user.id)) {
            return HttpResponse::Conflict().body("Email already in use");
        }
        let same = user.email.as_deref().is_some_and(|e| e.eq_ignore_ascii_case(email));
        if same && user.email_verified {
            return HttpResponse::Ok().json(serde_json::json!({"email": email, "email_verified": true, "sent": false}));
        }
        if !same
            && diesel::update(users::table.find(user.id))
                .set((users::email.eq(email), users::email_verified.eq(false)))
                .execute(&mut conn)
                .is_err()
        {
            return HttpResponse::InternalServerError().body("Error saving email");
        }
        user
    };
    let sent = send_verification(&pool, user.id, email).await;
    HttpResponse::Ok().json(serde_json::json!({"email": email, "email_verified": false, "sent": sent}))
}

#[derive(Deserialize)]
pub struct ChangeZoneRequest {
    pub zone: String,
}

/// The zone whose prices the rules follow; the rules are rescheduled with them
#[put("/zone")]
pub async fn change_zone(pool: web::Data<DbPool>, claims: Claims, item: web::Json<ChangeZoneRequest>) -> impl Responder {
    if !crate::zones::is_valid(&item.zone) {
        return HttpResponse::BadRequest().body("Unknown zone");
    }
    let Ok(user_id) = claims.sub.parse::<i32>() else {
        return HttpResponse::BadRequest().body("Invalid user ID");
    };
    let rule_ids: Vec<i32> = {
        let Ok(mut conn) = pool.get() else {
            return HttpResponse::InternalServerError().body("Database connection error");
        };
        if diesel::update(users::table.find(user_id))
            .set(users::zone.eq(&item.zone))
            .execute(&mut conn)
            .is_err()
        {
            return HttpResponse::InternalServerError().body("Error saving zone");
        }
        automation_rules::table
            .filter(automation_rules::user_id.eq(user_id))
            .select(automation_rules::id)
            .load(&mut conn)
            .unwrap_or_default()
    };
    let schedules = ScheduleComputationService::new(pool.get_ref().clone());
    for rule_id in rule_ids {
        if let Err(e) = schedules.refresh_rule(rule_id) {
            log::warn!("Failed to reschedule rule {} after a zone change: {}", rule_id, e);
        }
    }
    HttpResponse::Ok().json(serde_json::json!({"zone": item.zone}))
}

#[derive(Deserialize)]
pub struct ForgotPasswordRequest {
    pub email: String,
}

/// A link to set a new password, sent to a verified address. The answer is
/// the same whether the address has an account or not.
#[post("/forgot-password")]
pub async fn forgot_password(pool: web::Data<DbPool>, item: web::Json<ForgotPasswordRequest>) -> impl Responder {
    if !mailer::is_configured() {
        return HttpResponse::ServiceUnavailable().body("Email is not configured");
    }
    let email = item.email.trim().to_string();
    let answer = HttpResponse::Ok().json(serde_json::json!({"ok": true}));
    if !valid_email(&email) {
        return answer;
    }
    let token = {
        let Ok(mut conn) = pool.get() else { return answer };
        let user: Option<User> = users::table
            .filter(users::email_verified.eq(true))
            .filter(diesel::dsl::sql::<diesel::sql_types::Bool>("lower(email) = lower(")
                .bind::<diesel::sql_types::Text, _>(&email)
                .sql(")"))
            .first(&mut conn)
            .ok();
        let Some(user) = user else { return answer };
        // One email every few minutes at most
        let now = crate::time::now_naive();
        if user_tokens::last_sent(&mut conn, user.id, user_tokens::RESET_PASSWORD)
            .is_some_and(|at| now - at < chrono::Duration::minutes(2))
        {
            return answer;
        }
        let _ = user_tokens::invalidate(&mut conn, user.id, user_tokens::RESET_PASSWORD);
        match user_tokens::create(&mut conn, user.id, user_tokens::RESET_PASSWORD, &email, chrono::Duration::hours(1)) {
            Ok(t) => (user, t),
            Err(e) => {
                log::warn!("Could not create a reset token: {}", e);
                return answer;
            }
        }
    };
    let (user, token) = token;
    let Some(link) = mailer::public_link(&format!("restablir-contrasenya?token={}", token)) else { return answer };
    let text = format!(
        "Hola, {},\n\nHas demanat canviar la contrasenya del teu compte de PVPC Cheap. Obre aquest enllaç per posar-ne una de nova:\n\n{}\n\n\
         Caduca d'aquí a una hora i només es pot fer servir un cop.\n\n\
         Si no ho has demanat tu, no cal que facis res: la contrasenya no canviarà.\n",
        user.username, link
    );
    if let Err(e) = mailer::send(&email, "Canvia la contrasenya", &text).await {
        log::warn!("Could not send the reset email of user {}: {}", user.id, e);
    }
    answer
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

#[cfg(test)]
mod email_tests {
    use super::valid_email;

    #[test]
    fn test_valid_email() {
        assert!(valid_email("anna@example.cat"));
        assert!(valid_email("a.b+c@sub.example.com"));
        for bad in ["", "anna", "anna@", "@example.cat", "anna@example", "an na@example.cat", "a@b@c.cat", "a@.cat", "a@cat."] {
            assert!(!valid_email(bad), "{bad}");
        }
    }
}
