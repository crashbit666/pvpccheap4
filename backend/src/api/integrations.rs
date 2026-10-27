use crate::{
    db::DbPool,
    integrations::{ProviderError, ProviderRegistry},
    models::UserIntegration,
    schema::{devices, user_integrations},
    services::{auth::Claims, credentials, provider_health},
};
use actix_web::{HttpResponse, Responder, delete, get, post, web};
use diesel::prelude::*;
use log::info;
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
pub struct AddIntegrationRequest {
    pub provider: String,
    pub credentials: Value, // JSON object with email/password
}

#[post("")]
pub async fn add_integration(
    pool: web::Data<DbPool>,
    claims: Claims, // JWT Protected
    item: web::Json<AddIntegrationRequest>,
) -> impl Responder {
    let mut conn = pool.get().expect("Couldn't get db connection");
    let user_id = claims.sub.parse::<i32>().unwrap();

    // Get the provider and authenticate
    let registry = ProviderRegistry::new();
    let provider = match registry.get(&item.provider) {
        Some(p) => p,
        None => {
            return HttpResponse::BadRequest().body(format!("Unknown provider: {}", item.provider));
        }
    };

    // Perform login to get session credentials
    let session_credentials = match provider.login(&item.credentials).await {
        Ok(creds) => creds,
        // Unreachable provider (or Home Assistant URL) vs wrong credentials
        Err(e @ (ProviderError::ConnectionError(_) | ProviderError::Timeout)) => {
            return HttpResponse::BadGateway().body(format!("Cannot reach the provider: {}", e));
        }
        Err(e) => {
            return HttpResponse::Unauthorized().body(format!("Authentication failed: {}", e));
        }
    };

    // Merge original credentials (email/password) with session credentials (token/key)
    // This ensures we can refresh the token later
    let mut final_credentials = session_credentials.clone();
    if let (Some(obj), Some(orig)) = (
        final_credentials.as_object_mut(),
        item.credentials.as_object(),
    ) {
        // Copy email and password from original request if not present
        if !obj.contains_key("email")
            && let Some(email) = orig.get("email")
        {
            obj.insert("email".to_string(), email.clone());
        }
        if !obj.contains_key("password")
            && let Some(password) = orig.get("password")
        {
            obj.insert("password".to_string(), password.clone());
        }
    }

    info!(
        "Integration login successful for provider: {}",
        item.provider
    );

    // Adding the same provider again (e.g. after changing its password)
    // updates the existing integration, so its devices and rules are kept
    let existing = user_integrations::table
        .filter(user_integrations::user_id.eq(user_id))
        .filter(user_integrations::provider_name.eq(&item.provider))
        .select(user_integrations::id)
        .first::<i32>(&mut conn)
        .optional()
        .unwrap_or(None);

    let new_integration = match existing {
        Some(id) => diesel::update(user_integrations::table.find(id))
            .set((
                user_integrations::credentials_json.eq(credentials::encrypt(&final_credentials)),
                user_integrations::is_active.eq(true),
            ))
            .get_result::<UserIntegration>(&mut conn),
        None => diesel::insert_into(user_integrations::table)
            .values((
                user_integrations::user_id.eq(user_id),
                user_integrations::provider_name.eq(&item.provider),
                user_integrations::credentials_json.eq(credentials::encrypt(&final_credentials)),
                user_integrations::is_active.eq(true),
            ))
            .get_result::<UserIntegration>(&mut conn),
    };

    match new_integration {
        Ok(int) => HttpResponse::Ok().json(int),
        Err(e) => HttpResponse::InternalServerError().body(format!("Error: {}", e)),
    }
}

#[get("")]
pub async fn list_integrations(pool: web::Data<DbPool>, claims: Claims) -> impl Responder {
    let mut conn = pool.get().expect("Couldn't get db connection");
    let user_id = claims.sub.parse::<i32>().unwrap();

    let results = user_integrations::table
        .filter(user_integrations::user_id.eq(user_id))
        .load::<UserIntegration>(&mut conn);

    match results {
        Ok(list) => HttpResponse::Ok().json(list),
        Err(_) => HttpResponse::InternalServerError().body("Error fetching integrations"),
    }
}

/// Providers of the user's integrations that seem down (their API is not
/// official and may have changed), so the app can offer another path
#[get("/status")]
pub async fn integrations_status(pool: web::Data<DbPool>, claims: Claims) -> impl Responder {
    let Ok(user_id) = claims.sub.parse::<i32>() else {
        return HttpResponse::BadRequest().body("Invalid user ID");
    };
    let Ok(mut conn) = pool.get() else {
        return HttpResponse::InternalServerError().body("Database connection error");
    };
    let used: Vec<String> = user_integrations::table
        .filter(user_integrations::user_id.eq(user_id))
        .select(user_integrations::provider_name)
        .distinct()
        .load(&mut conn)
        .unwrap_or_default();
    let now = crate::time::now_naive();
    let zone = crate::zones::of_user(&mut conn, user_id);
    let statuses: Vec<provider_health::ProviderStatus> = provider_health::UNOFFICIAL
        .iter()
        .filter(|p| used.iter().any(|u| u == *p))
        .filter_map(|p| provider_health::status(&mut conn, p, now).ok())
        .map(|s| provider_health::ProviderStatus {
            failing_since: s.failing_since.map(|t| zone.to_local(t)),
            ..s
        })
        .collect();
    HttpResponse::Ok().json(statuses)
}

#[delete("/{id}")]
pub async fn delete_integration(
    pool: web::Data<DbPool>,
    registry: web::Data<ProviderRegistry>,
    claims: Claims,
    path: web::Path<i32>,
) -> impl Responder {
    let integration_id = path.into_inner();
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Database connection error"),
    };

    let user_id: i32 = match claims.sub.parse() {
        Ok(id) => id,
        Err(_) => return HttpResponse::BadRequest().body("Invalid user ID"),
    };

    // Verify the integration belongs to this user
    let integration_exists = user_integrations::table
        .filter(user_integrations::id.eq(integration_id))
        .filter(user_integrations::user_id.eq(user_id))
        .select(user_integrations::id)
        .first::<i32>(&mut conn)
        .is_ok();

    if !integration_exists {
        return HttpResponse::NotFound().body("Integration not found");
    }

    // Don't leave on a device that one of its rules switched on this hour
    let device_ids: Vec<i32> = devices::table
        .filter(devices::integration_id.eq(integration_id))
        .select(devices::id)
        .load(&mut conn)
        .unwrap_or_default();
    super::rules::release_devices(&pool, &registry, &device_ids).await;

    // Delete associated devices first
    let _ = diesel::delete(devices::table.filter(devices::integration_id.eq(integration_id)))
        .execute(&mut conn);

    // Delete the integration
    match diesel::delete(
        user_integrations::table
            .filter(user_integrations::id.eq(integration_id))
            .filter(user_integrations::user_id.eq(user_id)),
    )
    .execute(&mut conn)
    {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({"deleted": true})),
        Err(_) => HttpResponse::InternalServerError().body("Failed to delete integration"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_integration_request_deserialization() {
        let json = r#"{
            "provider": "meross",
            "credentials": {"email": "test@example.com", "password": "secret"}
        }"#;
        let request: AddIntegrationRequest = serde_json::from_str(json).unwrap();

        assert_eq!(request.provider, "meross");
        assert_eq!(request.credentials["email"], "test@example.com");
        assert_eq!(request.credentials["password"], "secret");
    }

    #[test]
    fn test_add_integration_request_with_complex_credentials() {
        let json = r#"{
            "provider": "tuya",
            "credentials": {
                "client_id": "abc123",
                "client_secret": "xyz789",
                "region": "eu",
                "device_ids": ["dev1", "dev2"]
            }
        }"#;
        let request: AddIntegrationRequest = serde_json::from_str(json).unwrap();

        assert_eq!(request.provider, "tuya");
        assert_eq!(request.credentials["client_id"], "abc123");
        assert_eq!(request.credentials["region"], "eu");
        assert!(request.credentials["device_ids"].is_array());
    }

    #[test]
    fn test_add_integration_request_with_empty_credentials() {
        let json = r#"{
            "provider": "test_provider",
            "credentials": {}
        }"#;
        let request: AddIntegrationRequest = serde_json::from_str(json).unwrap();

        assert_eq!(request.provider, "test_provider");
        assert!(request.credentials.is_object());
    }

    #[test]
    fn test_add_integration_request_missing_provider_fails() {
        let json = r#"{
            "credentials": {"email": "test@example.com"}
        }"#;
        let result: Result<AddIntegrationRequest, _> = serde_json::from_str(json);

        assert!(result.is_err());
    }

    #[test]
    fn test_add_integration_request_missing_credentials_fails() {
        let json = r#"{
            "provider": "meross"
        }"#;
        let result: Result<AddIntegrationRequest, _> = serde_json::from_str(json);

        assert!(result.is_err());
    }

    #[test]
    fn test_add_integration_request_with_null_credentials() {
        let json = r#"{
            "provider": "meross",
            "credentials": null
        }"#;
        let request: AddIntegrationRequest = serde_json::from_str(json).unwrap();

        assert_eq!(request.provider, "meross");
        assert!(request.credentials.is_null());
    }

    #[test]
    fn test_supported_provider_names() {
        let providers = vec!["meross", "tuya", "home_assistant", "shelly"];

        for provider in providers {
            let json = format!(r#"{{"provider": "{}", "credentials": {{}}}}"#, provider);
            let request: AddIntegrationRequest = serde_json::from_str(&json).unwrap();
            assert_eq!(request.provider, provider);
        }
    }
}
