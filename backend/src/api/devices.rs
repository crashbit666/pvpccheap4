use crate::{
    db::DbPool,
    integrations::{ProviderError, ProviderRegistry},
    models::{Device, UserIntegration},
    schema::{devices, user_integrations},
    services::auth::Claims,
};
use actix_web::{delete, get, post, web, HttpResponse, Responder};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct DeviceResponse {
    pub id: i32,
    pub integration_id: i32,
    pub external_id: String,
    pub name: String,
    pub device_type: String,
    pub is_managed: bool,
    pub provider_name: String,
    /// Device on/off state - defaults to false since we don't query real state on list
    /// The actual state is determined by controlling the device
    pub is_on: bool,
}

#[derive(Deserialize)]
pub struct SyncDevicesRequest {
    pub integration_id: i32,
}

#[derive(Deserialize)]
pub struct DeviceActionRequest {
    pub action: String, // "turn_on" or "turn_off"
}

#[derive(Deserialize)]
pub struct UpdateDeviceRequest {
    pub is_managed: Option<bool>,
    pub name: Option<String>,
}

/// List all devices for the authenticated user
#[get("")]
pub async fn list_devices(pool: web::Data<DbPool>, claims: Claims) -> impl Responder {
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Database connection error"),
    };

    let user_id: i32 = match claims.sub.parse() {
        Ok(id) => id,
        Err(_) => return HttpResponse::BadRequest().body("Invalid user ID"),
    };

    // Get all integrations for this user
    let user_integration_ids: Vec<i32> = match user_integrations::table
        .filter(user_integrations::user_id.eq(user_id))
        .filter(user_integrations::is_active.eq(true))
        .select(user_integrations::id)
        .load(&mut conn)
    {
        Ok(ids) => ids,
        Err(_) => return HttpResponse::InternalServerError().body("Error fetching integrations"),
    };

    if user_integration_ids.is_empty() {
        return HttpResponse::Ok().json(Vec::<DeviceResponse>::new());
    }

    // Get all devices for these integrations with provider names
    let results: Vec<(Device, String)> = match devices::table
        .inner_join(user_integrations::table)
        .filter(devices::integration_id.eq_any(&user_integration_ids))
        .select((Device::as_select(), user_integrations::provider_name))
        .load(&mut conn)
    {
        Ok(d) => d,
        Err(_) => return HttpResponse::InternalServerError().body("Error fetching devices"),
    };

    let response: Vec<DeviceResponse> = results
        .into_iter()
        .map(|(device, provider_name)| DeviceResponse {
            id: device.id,
            integration_id: device.integration_id,
            external_id: device.external_id,
            name: device.name,
            device_type: device.device_type,
            is_managed: device.is_managed,
            provider_name,
            is_on: device.is_on, // Use cached state from database
        })
        .collect();

    HttpResponse::Ok().json(response)
}

/// Sync devices from a provider integration
#[post("/sync")]
pub async fn sync_devices(
    pool: web::Data<DbPool>,
    registry: web::Data<ProviderRegistry>,
    claims: Claims,
    body: web::Json<SyncDevicesRequest>,
) -> impl Responder {
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Database connection error"),
    };

    let user_id: i32 = match claims.sub.parse() {
        Ok(id) => id,
        Err(_) => return HttpResponse::BadRequest().body("Invalid user ID"),
    };

    // Get the integration and verify it belongs to this user
    let integration: UserIntegration = match user_integrations::table
        .filter(user_integrations::id.eq(body.integration_id))
        .filter(user_integrations::user_id.eq(user_id))
        .first(&mut conn)
    {
        Ok(i) => i,
        Err(_) => return HttpResponse::NotFound().body("Integration not found"),
    };

    // Get the provider
    let provider = match registry.get(&integration.provider_name) {
        Some(p) => p,
        None => {
            return HttpResponse::BadRequest()
                .body(format!("Unknown provider: {}", integration.provider_name))
        }
    };

    // Parse stored credentials
    let credentials: serde_json::Value = match serde_json::from_str(&integration.credentials_json) {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Invalid stored credentials"),
    };

    // First, login to get a fresh token
    let session = match provider.login(&credentials).await {
        Ok(s) => s,
        Err(e) => {
            return HttpResponse::BadRequest().body(format!("Failed to authenticate: {}", e))
        }
    };

    // Update stored credentials with session data (contains user_id, key, mqtt_domain needed for MQTT)
    let updated_credentials = session.to_string();
    log::info!(
        "Updating credentials for integration {}: has user_id={}, has key={}, has mqtt_domain={}",
        integration.id,
        session.get("user_id").is_some(),
        session.get("key").is_some(),
        session.get("mqtt_domain").is_some()
    );
    match diesel::update(
        user_integrations::table.filter(user_integrations::id.eq(integration.id)),
    )
    .set(user_integrations::credentials_json.eq(&updated_credentials))
    .execute(&mut conn)
    {
        Ok(rows) => log::info!("Updated {} rows with new credentials", rows),
        Err(e) => log::error!("Failed to update credentials with session data: {}", e),
    }

    // List devices from the provider
    let discovered = match provider.list_devices(&session).await {
        Ok(d) => d,
        Err(e) => {
            return HttpResponse::InternalServerError()
                .body(format!("Failed to list devices: {}", e))
        }
    };

    // Sync to database
    let mut synced_count = 0;
    let mut new_count = 0;

    for device in discovered {
        // Check if device already exists
        let existing: Option<Device> = devices::table
            .filter(devices::integration_id.eq(integration.id))
            .filter(devices::external_id.eq(&device.external_id))
            .first(&mut conn)
            .optional()
            .unwrap_or(None);

        if existing.is_some() {
            // Update existing device name if changed
            diesel::update(
                devices::table
                    .filter(devices::integration_id.eq(integration.id))
                    .filter(devices::external_id.eq(&device.external_id)),
            )
            .set(devices::name.eq(&device.name))
            .execute(&mut conn)
            .ok();
            synced_count += 1;
        } else {
            // Insert new device
            diesel::insert_into(devices::table)
                .values((
                    devices::integration_id.eq(integration.id),
                    devices::external_id.eq(&device.external_id),
                    devices::name.eq(&device.name),
                    devices::device_type.eq(&device.device_type),
                    devices::is_managed.eq(false), // Default to not managed
                ))
                .execute(&mut conn)
                .ok();
            new_count += 1;
            synced_count += 1;
        }
    }

    HttpResponse::Ok().json(serde_json::json!({
        "synced": synced_count,
        "new": new_count,
        "message": format!("Synced {} devices ({} new)", synced_count, new_count)
    }))
}

/// Control a device (turn on/off)
#[post("/{device_id}/control")]
pub async fn control_device(
    pool: web::Data<DbPool>,
    registry: web::Data<ProviderRegistry>,
    claims: Claims,
    path: web::Path<i32>,
    body: web::Json<DeviceActionRequest>,
) -> impl Responder {
    let device_id = path.into_inner();
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Database connection error"),
    };

    let user_id: i32 = match claims.sub.parse() {
        Ok(id) => id,
        Err(_) => return HttpResponse::BadRequest().body("Invalid user ID"),
    };

    // Get device with integration info, verify ownership
    let (device, integration): (Device, UserIntegration) = match devices::table
        .inner_join(user_integrations::table)
        .filter(devices::id.eq(device_id))
        .filter(user_integrations::user_id.eq(user_id))
        .select((Device::as_select(), UserIntegration::as_select()))
        .first(&mut conn)
    {
        Ok(d) => d,
        Err(_) => return HttpResponse::NotFound().body("Device not found"),
    };

    // Get provider
    let provider = match registry.get(&integration.provider_name) {
        Some(p) => p,
        None => return HttpResponse::InternalServerError().body("Provider not available"),
    };

    // Parse credentials and login
    let credentials: serde_json::Value = match serde_json::from_str(&integration.credentials_json) {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Invalid stored credentials"),
    };

    let session = match provider.login(&credentials).await {
        Ok(s) => s,
        Err(e) => {
            return HttpResponse::BadRequest().body(format!("Authentication failed: {}", e))
        }
    };

    // Execute action
    let result = match body.action.as_str() {
        "turn_on" => provider.turn_on(&session, &device.external_id).await,
        "turn_off" => provider.turn_off(&session, &device.external_id).await,
        _ => return HttpResponse::BadRequest().body("Invalid action. Use 'turn_on' or 'turn_off'"),
    };

    match result {
        Ok(action_result) => {
            // Update cached is_on state in database if action was successful
            if action_result.success {
                if let Some(ref new_state) = action_result.new_state {
                    let _ = diesel::update(devices::table.filter(devices::id.eq(device_id)))
                        .set(devices::is_on.eq(new_state.is_on))
                        .execute(&mut conn);
                    log::info!("Updated device {} is_on state to {}", device_id, new_state.is_on);
                }
            }
            HttpResponse::Ok().json(action_result)
        }
        Err(e) => HttpResponse::InternalServerError().body(format!("Action failed: {}", e)),
    }
}

/// Get device state
#[get("/{device_id}/state")]
pub async fn get_device_state(
    pool: web::Data<DbPool>,
    registry: web::Data<ProviderRegistry>,
    claims: Claims,
    path: web::Path<i32>,
) -> impl Responder {
    let device_id = path.into_inner();
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Database connection error"),
    };

    let user_id: i32 = match claims.sub.parse() {
        Ok(id) => id,
        Err(_) => return HttpResponse::BadRequest().body("Invalid user ID"),
    };

    // Get device with integration
    let (device, integration): (Device, UserIntegration) = match devices::table
        .inner_join(user_integrations::table)
        .filter(devices::id.eq(device_id))
        .filter(user_integrations::user_id.eq(user_id))
        .select((Device::as_select(), UserIntegration::as_select()))
        .first(&mut conn)
    {
        Ok(d) => d,
        Err(_) => return HttpResponse::NotFound().body("Device not found"),
    };

    let provider = match registry.get(&integration.provider_name) {
        Some(p) => p,
        None => return HttpResponse::InternalServerError().body("Provider not available"),
    };

    let credentials: serde_json::Value = match serde_json::from_str(&integration.credentials_json) {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Invalid stored credentials"),
    };

    let session = match provider.login(&credentials).await {
        Ok(s) => s,
        Err(e) => {
            return HttpResponse::BadRequest().body(format!("Authentication failed: {}", e))
        }
    };

    match provider.get_device_state(&session, &device.external_id).await {
        Ok(state) => HttpResponse::Ok().json(state),
        Err(e) => HttpResponse::InternalServerError().body(format!("Failed to get state: {}", e)),
    }
}

/// Update device settings (e.g., is_managed flag)
#[post("/{device_id}")]
pub async fn update_device(
    pool: web::Data<DbPool>,
    claims: Claims,
    path: web::Path<i32>,
    body: web::Json<UpdateDeviceRequest>,
) -> impl Responder {
    let device_id = path.into_inner();
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Database connection error"),
    };

    let user_id: i32 = match claims.sub.parse() {
        Ok(id) => id,
        Err(_) => return HttpResponse::BadRequest().body("Invalid user ID"),
    };

    // Verify device belongs to user
    let device_exists = devices::table
        .inner_join(user_integrations::table)
        .filter(devices::id.eq(device_id))
        .filter(user_integrations::user_id.eq(user_id))
        .select(devices::id)
        .first::<i32>(&mut conn)
        .is_ok();

    if !device_exists {
        return HttpResponse::NotFound().body("Device not found");
    }

    // Update fields
    if let Some(is_managed) = body.is_managed {
        diesel::update(devices::table.filter(devices::id.eq(device_id)))
            .set(devices::is_managed.eq(is_managed))
            .execute(&mut conn)
            .ok();
    }

    if let Some(ref name) = body.name {
        diesel::update(devices::table.filter(devices::id.eq(device_id)))
            .set(devices::name.eq(name))
            .execute(&mut conn)
            .ok();
    }

    // Return updated device
    let updated: Device = match devices::table.find(device_id).first(&mut conn) {
        Ok(d) => d,
        Err(_) => return HttpResponse::InternalServerError().body("Error fetching updated device"),
    };

    HttpResponse::Ok().json(updated)
}

/// Delete a device
#[delete("/{device_id}")]
pub async fn delete_device(
    pool: web::Data<DbPool>,
    claims: Claims,
    path: web::Path<i32>,
) -> impl Responder {
    let device_id = path.into_inner();
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Database connection error"),
    };

    let user_id: i32 = match claims.sub.parse() {
        Ok(id) => id,
        Err(_) => return HttpResponse::BadRequest().body("Invalid user ID"),
    };

    // Verify device belongs to user
    let device_exists = devices::table
        .inner_join(user_integrations::table)
        .filter(devices::id.eq(device_id))
        .filter(user_integrations::user_id.eq(user_id))
        .select(devices::id)
        .first::<i32>(&mut conn)
        .is_ok();

    if !device_exists {
        return HttpResponse::NotFound().body("Device not found");
    }

    // Delete device
    match diesel::delete(devices::table.filter(devices::id.eq(device_id))).execute(&mut conn) {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({"deleted": true})),
        Err(_) => HttpResponse::InternalServerError().body("Failed to delete device"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_action_request_deserialization() {
        let json = r#"{"action": "turn_on"}"#;
        let request: DeviceActionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.action, "turn_on");
    }

    #[test]
    fn test_sync_devices_request_deserialization() {
        let json = r#"{"integration_id": 5}"#;
        let request: SyncDevicesRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.integration_id, 5);
    }

    #[test]
    fn test_update_device_request_deserialization() {
        let json = r#"{"is_managed": true, "name": "New Name"}"#;
        let request: UpdateDeviceRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.is_managed, Some(true));
        assert_eq!(request.name, Some("New Name".to_string()));
    }

    #[test]
    fn test_update_device_request_partial() {
        let json = r#"{"is_managed": false}"#;
        let request: UpdateDeviceRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.is_managed, Some(false));
        assert!(request.name.is_none());
    }
}
