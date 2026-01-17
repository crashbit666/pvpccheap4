use crate::{
    db::DbPool, models::UserIntegration, schema::user_integrations, services::auth::Claims,
};
use actix_web::{HttpResponse, Responder, get, post, web};
use diesel::prelude::*;
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
pub struct AddIntegrationRequest {
    pub provider: String,
    pub credentials: Value, // JSON object
}

#[post("")]
pub async fn add_integration(
    pool: web::Data<DbPool>,
    claims: Claims, // JWT Protected
    item: web::Json<AddIntegrationRequest>,
) -> impl Responder {
    let mut conn = pool.get().expect("Couldn't get db connection");
    let user_id = claims.sub.parse::<i32>().unwrap();

    let new_integration = diesel::insert_into(user_integrations::table)
        .values((
            user_integrations::user_id.eq(user_id),
            user_integrations::provider_name.eq(&item.provider),
            user_integrations::credentials_json.eq(item.credentials.to_string()),
            user_integrations::is_active.eq(true),
        ))
        .get_result::<UserIntegration>(&mut conn);

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
            let json = format!(
                r#"{{"provider": "{}", "credentials": {{}}}}"#,
                provider
            );
            let request: AddIntegrationRequest = serde_json::from_str(&json).unwrap();
            assert_eq!(request.provider, provider);
        }
    }
}
