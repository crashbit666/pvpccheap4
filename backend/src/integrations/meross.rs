use super::{
    DeviceActionResult, DeviceCapabilities, DeviceState, DiscoveredDevice, ProviderError,
    SmartHomeProvider,
};
use async_trait::async_trait;
use log::{debug, error, info, warn};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

// Meross API constants
const MEROSS_SECRET: &str = "23x17ahWarFH6w29";
const MEROSS_API_EU: &str = "https://iotx-eu.meross.com";
const MEROSS_API_US: &str = "https://iotx-us.meross.com";
const MEROSS_API_AP: &str = "https://iotx-ap.meross.com";

// Request/Response structures for Meross API
#[derive(Serialize)]
struct MerossRequest {
    params: String,
    sign: String,
    timestamp: i64,
    nonce: String,
}

#[derive(Deserialize, Debug)]
struct MerossResponse<T> {
    #[serde(rename = "apiStatus")]
    api_status: i32,
    #[serde(rename = "sysStatus")]
    sys_status: Option<i32>,
    data: Option<T>,
    info: Option<String>,
    timestamp: Option<i64>,
}

#[derive(Deserialize, Debug, Clone)]
struct MerossLoginData {
    userid: String,
    email: String,
    token: String,
    key: String,
    #[serde(rename = "mqttDomain", default)]
    mqtt_domain: Option<String>,
    #[serde(flatten)]
    _extra: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Deserialize, Debug, Clone)]
struct MerossDeviceData {
    uuid: String,
    #[serde(rename = "devName")]
    dev_name: String,
    #[serde(rename = "deviceType")]
    device_type: String,
    // onlineStatus can be int (0/1/2) or string ("ONLINE"/"OFFLINE") depending on API version
    #[serde(default, rename = "onlineStatus")]
    online_status: serde_json::Value,
    #[serde(default)]
    channels: Option<Vec<MerossChannel>>,
    #[serde(default)]
    domain: Option<String>,
    #[serde(default)]
    region: Option<String>,
    // Allow any other fields we don't care about
    #[serde(flatten)]
    _extra: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Deserialize, Debug, Clone)]
struct MerossChannel {
    #[serde(default)]
    channel: Option<i32>,
    #[serde(rename = "devName", default)]
    dev_name: Option<String>,
    #[serde(rename = "type", default)]
    channel_type: Option<String>,
    #[serde(flatten)]
    _extra: std::collections::HashMap<String, serde_json::Value>,
}

pub struct MerossProvider {
    client: Client,
}

impl Default for MerossProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MerossProvider {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    /// Generate a random nonce (16 alphanumeric characters)
    fn generate_nonce() -> String {
        use std::collections::hash_map::RandomState;
        use std::hash::{BuildHasher, Hasher};

        let hasher = RandomState::new().build_hasher();
        let seed = hasher.finish();

        const CHARS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
        let mut nonce = String::with_capacity(16);
        let mut value = seed;

        for _ in 0..16 {
            nonce.push(CHARS[(value % 36) as usize] as char);
            value /= 36;
            if value == 0 {
                value = seed.wrapping_mul(31);
            }
        }

        nonce
    }

    /// Get current timestamp in milliseconds
    fn get_timestamp() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
    }

    /// Calculate MD5 signature for Meross API
    fn calculate_sign(timestamp: i64, nonce: &str, params_base64: &str) -> String {
        let sign_string = format!("{}{}{}{}", MEROSS_SECRET, timestamp, nonce, params_base64);
        let digest = md5::compute(sign_string.as_bytes());
        // Must be lowercase hex
        format!("{:x}", digest)
    }

    /// Build a Meross API request
    fn build_request<T: Serialize>(params: &T) -> MerossRequest {
        let params_json = serde_json::to_string(params).unwrap_or_else(|_| "{}".to_string());
        let params_base64 = base64_encode(&params_json);
        let timestamp = Self::get_timestamp();
        let nonce = Self::generate_nonce();
        let sign = Self::calculate_sign(timestamp, &nonce, &params_base64);

        MerossRequest {
            params: params_base64,
            sign,
            timestamp,
            nonce,
        }
    }

    /// Try login against a specific regional endpoint
    async fn try_login_endpoint(
        &self,
        base_url: &str,
        email: &str,
        password: &str,
    ) -> Result<MerossLoginData, ProviderError> {
        let url = format!("{}/v1/Auth/signIn", base_url);

        #[derive(Serialize)]
        struct LoginParams {
            email: String,
            password: String,
        }

        let params = LoginParams {
            email: email.to_string(),
            password: password.to_string(),
        };

        let request = Self::build_request(&params);

        debug!("Attempting Meross login to: {}", base_url);

        let response = self.client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("vender", "Meross")
            .header("AppVersion", "1.3.0")
            .header("AppLanguage", "EN")
            .header("User-Agent", "okhttp/3.6.0")
            .json(&request)
            .send()
            .await
            .map_err(|e| ProviderError::ConnectionError(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            return Err(ProviderError::ConnectionError(format!(
                "HTTP error: {}",
                status
            )));
        }

        // Get raw response text for debugging
        let response_text = response
            .text()
            .await
            .map_err(|e| ProviderError::Unknown(format!("Failed to read response: {}", e)))?;

        debug!("Meross login response: {}", response_text);

        let meross_response: MerossResponse<MerossLoginData> = serde_json::from_str(&response_text)
            .map_err(|e| ProviderError::Unknown(format!("Failed to parse response: {}. Body: {}", e, response_text)))?;

        // Meross uses api_status 0 for success
        if meross_response.api_status != 0 {
            let error_msg = meross_response.info.unwrap_or_else(|| "Unknown error".to_string());
            return Err(ProviderError::AuthenticationFailed(format!(
                "API error {}: {}",
                meross_response.api_status, error_msg
            )));
        }

        meross_response.data.ok_or_else(|| {
            ProviderError::AuthenticationFailed("No data in response".to_string())
        })
    }

    /// Get device list from Meross API
    async fn get_device_list(
        &self,
        base_url: &str,
        token: &str,
    ) -> Result<Vec<MerossDeviceData>, ProviderError> {
        let url = format!("{}/v1/Device/devList", base_url);

        // Empty params for device list
        let params: serde_json::Map<String, Value> = serde_json::Map::new();
        let request = Self::build_request(&params);

        let response = self.client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Basic {}", token))
            .header("vender", "Meross")
            .header("AppVersion", "1.3.0")
            .header("AppLanguage", "EN")
            .header("User-Agent", "okhttp/3.6.0")
            .json(&request)
            .send()
            .await
            .map_err(|e| ProviderError::ConnectionError(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            return Err(ProviderError::ConnectionError(format!(
                "HTTP error: {}",
                status
            )));
        }

        // Get raw response text for debugging
        let response_text = response
            .text()
            .await
            .map_err(|e| ProviderError::Unknown(format!("Failed to read response: {}", e)))?;

        debug!("Meross devList response: {}", response_text);

        let meross_response: MerossResponse<Vec<MerossDeviceData>> = serde_json::from_str(&response_text)
            .map_err(|e| ProviderError::Unknown(format!("Failed to parse response: {}. Body: {}", e, response_text)))?;

        if meross_response.api_status != 0 {
            let error_msg = meross_response.info.unwrap_or_else(|| "Unknown error".to_string());
            return Err(ProviderError::AuthenticationFailed(format!(
                "API error {}: {}",
                meross_response.api_status, error_msg
            )));
        }

        Ok(meross_response.data.unwrap_or_default())
    }

    /// Extract API base URL from credentials
    fn get_api_url(credentials: &Value) -> String {
        credentials
            .get("api_base_url")
            .and_then(|v| v.as_str())
            .unwrap_or(MEROSS_API_EU)
            .to_string()
    }
}

/// Simple base64 encoding without external dependency
fn base64_encode(input: &str) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let bytes = input.as_bytes();
    let mut result = String::new();

    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);

        result.push(ALPHABET[(b0 >> 2) as usize] as char);
        result.push(ALPHABET[((b0 & 0x03) << 4 | b1 >> 4) as usize] as char);

        if chunk.len() > 1 {
            result.push(ALPHABET[((b1 & 0x0F) << 2 | b2 >> 6) as usize] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(ALPHABET[(b2 & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }

    result
}

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

        info!("Meross login attempt for: {}", email);

        // Try each regional endpoint until one works
        let endpoints = [MEROSS_API_EU, MEROSS_API_US, MEROSS_API_AP];
        let mut last_error = ProviderError::Unknown("No endpoints tried".to_string());

        for endpoint in endpoints {
            match self.try_login_endpoint(endpoint, email, password).await {
                Ok(login_data) => {
                    info!("Meross login successful via {}", endpoint);
                    return Ok(serde_json::json!({
                        "token": login_data.token,
                        "key": login_data.key,
                        "user_id": login_data.userid,
                        "email": login_data.email,
                        "api_base_url": endpoint,
                        "mqtt_domain": login_data.mqtt_domain
                    }));
                }
                Err(e) => {
                    debug!("Meross login failed at {}: {}", endpoint, e);
                    last_error = e;
                }
            }
        }

        error!("Meross login failed on all endpoints");
        Err(last_error)
    }

    async fn list_devices(&self, credentials: &Value) -> Result<Vec<DiscoveredDevice>, ProviderError> {
        let token = credentials
            .get("token")
            .and_then(|v| v.as_str())
            .ok_or(ProviderError::AuthenticationFailed(
                "No token provided".to_string(),
            ))?;

        let api_url = Self::get_api_url(credentials);

        info!("Meross listing devices from {}", api_url);

        let devices = self.get_device_list(&api_url, token).await?;

        let discovered: Vec<DiscoveredDevice> = devices
            .into_iter()
            .map(|d| {
                // Determine device type from Meross deviceType
                let device_type = match d.device_type.as_str() {
                    t if t.starts_with("mss") => "switch", // Smart plugs
                    t if t.starts_with("msl") => "light",  // Smart lights
                    t if t.starts_with("msg") => "garage", // Garage door
                    t if t.starts_with("mts") => "thermostat",
                    _ => "unknown",
                };

                DiscoveredDevice {
                    external_id: d.uuid,
                    name: d.dev_name,
                    device_type: device_type.to_string(),
                }
            })
            .collect();

        info!("Meross found {} devices", discovered.len());
        Ok(discovered)
    }

    async fn get_device_state(
        &self,
        credentials: &Value,
        external_id: &str,
    ) -> Result<DeviceState, ProviderError> {
        // Validate we have required credentials
        credentials
            .get("token")
            .and_then(|v| v.as_str())
            .ok_or(ProviderError::AuthenticationFailed(
                "No token provided".to_string(),
            ))?;

        // Create MQTT client and connect
        let mut mqtt_client = super::meross_mqtt::MerossMqttClient::from_credentials(credentials)?;

        match mqtt_client.connect().await {
            Ok(()) => {
                let result = mqtt_client.get_state(external_id).await;
                let _ = mqtt_client.disconnect().await;
                result
            }
            Err(e) => {
                warn!("Meross MQTT connection failed for get_device_state: {}", e);
                // Return default state on connection failure
                Ok(DeviceState {
                    is_on: false,
                    brightness: None,
                    temperature: None,
                    power_consumption_watts: None,
                })
            }
        }
    }

    async fn turn_on(
        &self,
        credentials: &Value,
        external_id: &str,
    ) -> Result<DeviceActionResult, ProviderError> {
        // Validate we have required credentials
        credentials
            .get("token")
            .and_then(|v| v.as_str())
            .ok_or(ProviderError::AuthenticationFailed(
                "No token provided".to_string(),
            ))?;

        // Create MQTT client and connect
        let mut mqtt_client = super::meross_mqtt::MerossMqttClient::from_credentials(credentials)?;

        match mqtt_client.connect().await {
            Ok(()) => {
                let result = mqtt_client.turn_on(external_id, 0).await;
                let _ = mqtt_client.disconnect().await;
                result
            }
            Err(e) => {
                error!("Meross MQTT connection failed for turn_on: {}", e);
                Ok(DeviceActionResult {
                    success: false,
                    message: Some(format!("MQTT connection failed: {}", e)),
                    new_state: None,
                })
            }
        }
    }

    async fn turn_off(
        &self,
        credentials: &Value,
        external_id: &str,
    ) -> Result<DeviceActionResult, ProviderError> {
        // Validate we have required credentials
        credentials
            .get("token")
            .and_then(|v| v.as_str())
            .ok_or(ProviderError::AuthenticationFailed(
                "No token provided".to_string(),
            ))?;

        // Create MQTT client and connect
        let mut mqtt_client = super::meross_mqtt::MerossMqttClient::from_credentials(credentials)?;

        match mqtt_client.connect().await {
            Ok(()) => {
                let result = mqtt_client.turn_off(external_id, 0).await;
                let _ = mqtt_client.disconnect().await;
                result
            }
            Err(e) => {
                error!("Meross MQTT connection failed for turn_off: {}", e);
                Ok(DeviceActionResult {
                    success: false,
                    message: Some(format!("MQTT connection failed: {}", e)),
                    new_state: None,
                })
            }
        }
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
        let provider = MerossProvider::new();
        assert_eq!(provider.provider_name(), "meross");
    }

    #[test]
    fn test_display_name() {
        let provider = MerossProvider::new();
        assert_eq!(provider.display_name(), "Meross Smart Home");
    }

    #[test]
    fn test_capabilities() {
        let provider = MerossProvider::new();
        let caps = provider.get_capabilities();

        assert!(caps.can_toggle);
        assert!(caps.can_read_consumption);
        assert!(!caps.can_dim);
        assert!(!caps.can_set_temperature);
    }

    #[test]
    fn test_base64_encode() {
        // Test base64 encoding
        assert_eq!(base64_encode("hello"), "aGVsbG8=");
        assert_eq!(base64_encode(""), "");
        assert_eq!(base64_encode("a"), "YQ==");
        assert_eq!(base64_encode("ab"), "YWI=");
        assert_eq!(base64_encode("abc"), "YWJj");
    }

    #[test]
    fn test_generate_nonce() {
        let nonce1 = MerossProvider::generate_nonce();
        let nonce2 = MerossProvider::generate_nonce();

        // Nonce should be 16 characters
        assert_eq!(nonce1.len(), 16);
        assert_eq!(nonce2.len(), 16);

        // All characters should be alphanumeric
        assert!(nonce1.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn test_calculate_sign() {
        // Test MD5 signature calculation
        let timestamp = 1234567890i64;
        let nonce = "ABCDEFGHIJKLMNOP";
        let params = "eyJ0ZXN0IjoidmFsdWUifQ=="; // {"test":"value"} in base64

        let sign = MerossProvider::calculate_sign(timestamp, nonce, params);

        // Sign should be a 32-character hex string
        assert_eq!(sign.len(), 32);
        assert!(sign.chars().all(|c| c.is_ascii_hexdigit()));
        // Should be lowercase
        assert_eq!(sign, sign.to_lowercase());
    }

    #[test]
    fn test_build_request() {
        #[derive(Serialize)]
        struct TestParams {
            email: String,
        }

        let params = TestParams {
            email: "test@example.com".to_string(),
        };

        let request = MerossProvider::build_request(&params);

        // Request should have all required fields
        assert!(!request.params.is_empty());
        assert_eq!(request.sign.len(), 32);
        assert_eq!(request.nonce.len(), 16);
        assert!(request.timestamp > 0);
    }

    #[test]
    fn test_get_api_url_default() {
        let credentials = serde_json::json!({});
        let url = MerossProvider::get_api_url(&credentials);
        assert_eq!(url, MEROSS_API_EU);
    }

    #[test]
    fn test_get_api_url_custom() {
        let credentials = serde_json::json!({
            "api_base_url": "https://custom.meross.com"
        });
        let url = MerossProvider::get_api_url(&credentials);
        assert_eq!(url, "https://custom.meross.com");
    }

    #[actix_rt::test]
    async fn test_login_missing_email() {
        let provider = MerossProvider::new();
        let credentials = serde_json::json!({
            "password": "test_password"
        });

        let result = provider.login(&credentials).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ProviderError::InvalidCredentials));
    }

    #[actix_rt::test]
    async fn test_login_missing_password() {
        let provider = MerossProvider::new();
        let credentials = serde_json::json!({
            "email": "test@example.com"
        });

        let result = provider.login(&credentials).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ProviderError::InvalidCredentials));
    }

    #[actix_rt::test]
    async fn test_list_devices_missing_token() {
        let provider = MerossProvider::new();
        let credentials = serde_json::json!({});

        let result = provider.list_devices(&credentials).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ProviderError::AuthenticationFailed(_)));
    }

    #[actix_rt::test]
    async fn test_turn_on_missing_token() {
        let provider = MerossProvider::new();
        let credentials = serde_json::json!({});

        let result = provider.turn_on(&credentials, "device_123").await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ProviderError::AuthenticationFailed(_)));
    }

    #[actix_rt::test]
    async fn test_turn_off_missing_token() {
        let provider = MerossProvider::new();
        let credentials = serde_json::json!({});

        let result = provider.turn_off(&credentials, "device_123").await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ProviderError::AuthenticationFailed(_)));
    }

    #[actix_rt::test]
    async fn test_get_device_state_missing_token() {
        let provider = MerossProvider::new();
        let credentials = serde_json::json!({});

        let result = provider.get_device_state(&credentials, "device_123").await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ProviderError::AuthenticationFailed(_)));
    }

    #[actix_rt::test]
    async fn test_turn_on_requires_mqtt_credentials() {
        let provider = MerossProvider::new();
        // Missing user_id and key required for MQTT
        let credentials = serde_json::json!({
            "token": "some_token"
        });

        let result = provider.turn_on(&credentials, "device_123").await;

        // Should return Err because MQTT credentials (user_id, key) are missing
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ProviderError::AuthenticationFailed(_)));
    }

    #[actix_rt::test]
    async fn test_turn_off_requires_mqtt_credentials() {
        let provider = MerossProvider::new();
        // Missing user_id and key required for MQTT
        let credentials = serde_json::json!({
            "token": "some_token"
        });

        let result = provider.turn_off(&credentials, "device_123").await;

        // Should return Err because MQTT credentials (user_id, key) are missing
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ProviderError::AuthenticationFailed(_)));
    }

    #[actix_rt::test]
    async fn test_get_device_state_fallback_on_connection_failure() {
        let provider = MerossProvider::new();
        // Credentials with MQTT info but will fail to connect
        let credentials = serde_json::json!({
            "token": "some_token",
            "user_id": "test_user",
            "key": "test_key",
            "mqtt_domain": "invalid.domain.test"
        });

        let result = provider.get_device_state(&credentials, "device_123").await;

        // Should return Ok with default state on connection failure
        assert!(result.is_ok());
        let state = result.unwrap();
        assert!(!state.is_on); // Default state
    }

    #[test]
    fn test_default_trait() {
        let provider = MerossProvider::default();
        assert_eq!(provider.provider_name(), "meross");
    }
}
