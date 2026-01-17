use super::{
    DeviceActionResult, DeviceCapabilities, DeviceState, DiscoveredDevice, ProviderError,
    SmartHomeProvider,
};
use async_trait::async_trait;
use serde_json::Value;

pub struct MerossProvider;

#[async_trait]
impl SmartHomeProvider for MerossProvider {
    fn provider_name(&self) -> &'static str {
        "meross"
    }

    fn display_name(&self) -> &'static str {
        "Meross Smart Home"
    }

    async fn login(&self, credentials: &Value) -> Result<Value, ProviderError> {
        let email = credentials
            .get("email")
            .and_then(|v| v.as_str())
            .ok_or(ProviderError::InvalidCredentials)?;
        let password = credentials
            .get("password")
            .and_then(|v| v.as_str())
            .ok_or(ProviderError::InvalidCredentials)?;

        // TODO: Implement real Meross Login
        // The Meross API requires:
        // 1. Generate nonce and timestamp
        // 2. Sign request with MD5: MD5(secret + timestamp + nonce + data)
        // 3. POST to https://iot.meross.com/v1/Auth/Login
        // 4. Extract token and key from response

        log::info!("Meross login attempt for: {}", email);

        // For now, return a dummy token structure
        Ok(serde_json::json!({
            "token": format!("meross_token_for_{}", email),
            "key": "dummy_encryption_key",
            "user_id": "user_12345",
            "email": email
        }))
    }

    async fn list_devices(&self, credentials: &Value) -> Result<Vec<DiscoveredDevice>, ProviderError> {
        let _token = credentials
            .get("token")
            .and_then(|v| v.as_str())
            .ok_or(ProviderError::AuthenticationFailed(
                "No token provided".to_string(),
            ))?;

        // TODO: Implement real Meross device listing
        // POST to https://iot.meross.com/v1/Device/devList

        log::info!("Meross listing devices");

        // Return dummy devices for now
        Ok(vec![
            DiscoveredDevice {
                external_id: "meross_plug_001".to_string(),
                name: "Living Room Plug".to_string(),
                device_type: "switch".to_string(),
            },
            DiscoveredDevice {
                external_id: "meross_plug_002".to_string(),
                name: "Bedroom Lamp".to_string(),
                device_type: "switch".to_string(),
            },
        ])
    }

    async fn get_device_state(
        &self,
        credentials: &Value,
        external_id: &str,
    ) -> Result<DeviceState, ProviderError> {
        let _token = credentials
            .get("token")
            .and_then(|v| v.as_str())
            .ok_or(ProviderError::AuthenticationFailed(
                "No token provided".to_string(),
            ))?;

        // TODO: Query actual device state via MQTT or HTTP

        log::info!("Meross getting state for device: {}", external_id);

        Ok(DeviceState {
            is_on: false,
            brightness: None,
            temperature: None,
            power_consumption_watts: Some(0.0),
        })
    }

    async fn turn_on(
        &self,
        credentials: &Value,
        external_id: &str,
    ) -> Result<DeviceActionResult, ProviderError> {
        let _token = credentials
            .get("token")
            .and_then(|v| v.as_str())
            .ok_or(ProviderError::AuthenticationFailed(
                "No token provided".to_string(),
            ))?;

        // TODO: Send MQTT command to Meross broker
        // Topic: /appliance/{device_uuid}/publish
        // Payload: Toggle command with channel 0

        log::info!("Meross turning ON device: {}", external_id);

        Ok(DeviceActionResult {
            success: true,
            message: Some(format!("Device {} turned on", external_id)),
            new_state: Some(DeviceState {
                is_on: true,
                brightness: None,
                temperature: None,
                power_consumption_watts: None,
            }),
        })
    }

    async fn turn_off(
        &self,
        credentials: &Value,
        external_id: &str,
    ) -> Result<DeviceActionResult, ProviderError> {
        let _token = credentials
            .get("token")
            .and_then(|v| v.as_str())
            .ok_or(ProviderError::AuthenticationFailed(
                "No token provided".to_string(),
            ))?;

        log::info!("Meross turning OFF device: {}", external_id);

        Ok(DeviceActionResult {
            success: true,
            message: Some(format!("Device {} turned off", external_id)),
            new_state: Some(DeviceState {
                is_on: false,
                brightness: None,
                temperature: None,
                power_consumption_watts: None,
            }),
        })
    }

    fn get_capabilities(&self) -> DeviceCapabilities {
        DeviceCapabilities {
            can_toggle: true,
            can_dim: false,
            can_set_temperature: false,
            can_read_consumption: true, // Meross plugs can read power
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_name() {
        let provider = MerossProvider;
        assert_eq!(provider.provider_name(), "meross");
    }

    #[test]
    fn test_display_name() {
        let provider = MerossProvider;
        assert_eq!(provider.display_name(), "Meross Smart Home");
    }

    #[actix_rt::test]
    async fn test_login_with_valid_credentials() {
        let provider = MerossProvider;
        let credentials = serde_json::json!({
            "email": "test@example.com",
            "password": "test_password"
        });

        let result = provider.login(&credentials).await;

        assert!(result.is_ok());
        let token_data = result.unwrap();
        assert!(token_data.get("token").is_some());
        assert!(token_data.get("key").is_some());
    }

    #[actix_rt::test]
    async fn test_login_with_missing_email() {
        let provider = MerossProvider;
        let credentials = serde_json::json!({
            "password": "test_password"
        });

        let result = provider.login(&credentials).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ProviderError::InvalidCredentials));
    }

    #[actix_rt::test]
    async fn test_login_with_missing_password() {
        let provider = MerossProvider;
        let credentials = serde_json::json!({
            "email": "test@example.com"
        });

        let result = provider.login(&credentials).await;

        assert!(result.is_err());
    }

    #[actix_rt::test]
    async fn test_list_devices_with_token() {
        let provider = MerossProvider;
        let credentials = serde_json::json!({
            "token": "valid_token"
        });

        let result = provider.list_devices(&credentials).await;

        assert!(result.is_ok());
        let devices = result.unwrap();
        assert!(!devices.is_empty());
    }

    #[actix_rt::test]
    async fn test_list_devices_without_token() {
        let provider = MerossProvider;
        let credentials = serde_json::json!({});

        let result = provider.list_devices(&credentials).await;

        assert!(result.is_err());
    }

    #[actix_rt::test]
    async fn test_turn_on_success() {
        let provider = MerossProvider;
        let credentials = serde_json::json!({
            "token": "valid_token"
        });

        let result = provider.turn_on(&credentials, "device_123").await;

        assert!(result.is_ok());
        let action_result = result.unwrap();
        assert!(action_result.success);
        assert!(action_result.new_state.is_some());
        assert!(action_result.new_state.unwrap().is_on);
    }

    #[actix_rt::test]
    async fn test_turn_off_success() {
        let provider = MerossProvider;
        let credentials = serde_json::json!({
            "token": "valid_token"
        });

        let result = provider.turn_off(&credentials, "device_123").await;

        assert!(result.is_ok());
        let action_result = result.unwrap();
        assert!(action_result.success);
        assert!(action_result.new_state.is_some());
        assert!(!action_result.new_state.unwrap().is_on);
    }

    #[actix_rt::test]
    async fn test_get_device_state() {
        let provider = MerossProvider;
        let credentials = serde_json::json!({
            "token": "valid_token"
        });

        let result = provider.get_device_state(&credentials, "device_123").await;

        assert!(result.is_ok());
    }

    #[test]
    fn test_capabilities() {
        let provider = MerossProvider;
        let caps = provider.get_capabilities();

        assert!(caps.can_toggle);
        assert!(caps.can_read_consumption);
        assert!(!caps.can_dim);
    }
}
