//! Meross MQTT Client for device control
//!
//! Meross devices use MQTT for real-time control and state queries.
//! This module implements the Meross-specific MQTT protocol.

use super::mqtt::{MqttConfig, MqttConnection, MqttError};
use super::{DeviceActionResult, DeviceState, ProviderError};
use log::{debug, error, info, warn};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;

/// Meross message header
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MerossHeader {
    pub message_id: String,
    pub method: String,
    pub from: String,
    pub namespace: String,
    pub timestamp: i64,
    pub timestamp_ms: i64,
    pub sign: String,
    pub payload_version: i32,
}

/// Meross MQTT message format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerossMessage {
    pub header: MerossHeader,
    pub payload: Value,
}

/// Meross toggle payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TogglePayload {
    pub togglex: Option<ToggleX>,
    pub toggle: Option<Toggle>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToggleX {
    pub channel: i32,
    pub onoff: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Toggle {
    pub onoff: i32,
}

/// Meross device state response
#[derive(Debug, Clone, Deserialize)]
pub struct SystemAllResponse {
    pub all: Option<SystemAll>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SystemAll {
    pub digest: Option<Digest>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Digest {
    pub togglex: Option<Vec<ToggleXState>>,
    pub toggle: Option<ToggleState>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ToggleXState {
    pub channel: i32,
    pub onoff: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ToggleState {
    pub onoff: i32,
}

/// Meross MQTT client for controlling devices
pub struct MerossMqttClient {
    connection: Option<Arc<Mutex<MqttConnection>>>,
    user_id: String,
    key: String,
    mqtt_domain: String,
    /// App ID must be consistent between subscription topic and message headers
    app_id: String,
}

impl MerossMqttClient {
    /// Create a new Meross MQTT client
    pub fn new(user_id: String, key: String, mqtt_domain: String) -> Self {
        Self {
            connection: None,
            user_id,
            key,
            mqtt_domain,
            app_id: Self::generate_app_id(), // Generate once and reuse
        }
    }

    /// Create from credentials JSON
    pub fn from_credentials(credentials: &Value) -> Result<Self, ProviderError> {
        let user_id = credentials
            .get("user_id")
            .and_then(|v| v.as_str())
            .ok_or(ProviderError::AuthenticationFailed(
                "Missing user_id in credentials".to_string(),
            ))?;

        let key = credentials
            .get("key")
            .and_then(|v| v.as_str())
            .ok_or(ProviderError::AuthenticationFailed(
                "Missing key in credentials".to_string(),
            ))?;

        // mqtt_domain from API is usually like "mqtt-eu-5.meross.com"
        // The MQTT broker runs on port 2001 with TLS
        let mqtt_domain = credentials
            .get("mqtt_domain")
            .and_then(|v| v.as_str())
            .unwrap_or("mqtt-eu-5.meross.com");

        Ok(Self::new(
            user_id.to_string(),
            key.to_string(),
            mqtt_domain.to_string(),
        ))
    }

    /// Generate a unique message ID
    fn generate_message_id() -> String {
        let mut rng = rand::rng();
        let random: u128 = rng.random();
        format!("{:032x}", random)
    }

    /// Get current timestamp
    fn get_timestamp() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    }

    /// Calculate MD5 signature for Meross MQTT
    fn calculate_sign(message_id: &str, key: &str, timestamp: i64) -> String {
        let sign_string = format!("{}{}{}", message_id, key, timestamp);
        let digest = md5::compute(sign_string.as_bytes());
        format!("{:x}", digest)
    }

    /// Build the client app ID for MQTT client_id
    fn build_client_id(&self) -> String {
        format!("app:{}", self.app_id)
    }

    /// Generate app ID (called once during construction)
    fn generate_app_id() -> String {
        let mut rng = rand::rng();
        let random: u64 = rng.random();
        format!("{:016x}", random)
    }

    /// Build publish topic for a device
    fn build_device_topic(device_uuid: &str) -> String {
        format!("/appliance/{}/subscribe", device_uuid)
    }

    /// Build response topic for client - uses the stored app_id for consistency
    fn build_client_topic(&self) -> String {
        format!("/app/{}-{}/subscribe", self.user_id, self.app_id)
    }

    /// Generate MQTT password from user_id and key
    /// Meross MQTT password = MD5(user_id + key)
    /// See: https://albertogeniola.github.io/MerossIot/meross-protocol.html
    fn generate_mqtt_password(user_id: &str, key: &str) -> String {
        let password_string = format!("{}{}", user_id, key);
        let digest = md5::compute(password_string.as_bytes());
        format!("{:x}", digest)
    }

    /// Connect to MQTT broker
    pub async fn connect(&mut self) -> Result<(), ProviderError> {
        let client_id = self.build_client_id();
        let client_topic = self.build_client_topic();

        // Parse domain - remove port if present
        // Meross Cloud MQTT uses port 2001 with TLS TCP (not standard 8883)
        // See: https://github.com/albertogeniola/MerossIot/wiki/Device-pairing
        let (host, port) = if self.mqtt_domain.contains(':') {
            let parts: Vec<&str> = self.mqtt_domain.split(':').collect();
            (
                parts[0].to_string(),
                parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(2001),
            )
        } else {
            (self.mqtt_domain.clone(), 2001)
        };

        // Generate MQTT password: MD5(user_id + key)
        let mqtt_password = Self::generate_mqtt_password(&self.user_id, &self.key);

        info!("Meross MQTT connecting to {}:{} (TLS TCP)", host, port);
        debug!("Meross MQTT auth: user_id={}, password_hash={}", self.user_id, &mqtt_password[..8]);

        let config = MqttConfig {
            broker_host: host,
            broker_port: port,
            client_id,
            username: Some(self.user_id.clone()),
            password: Some(mqtt_password),
            use_tls: true,
            use_websocket: false, // Meross uses standard MQTT over TLS TCP, NOT WebSocket
            keep_alive_secs: 30,
        };

        let connection = MqttConnection::connect(config)
            .await
            .map_err(|e| ProviderError::ConnectionError(e.to_string()))?;

        // Subscribe to client topic
        connection
            .subscribe(&client_topic)
            .await
            .map_err(|e| ProviderError::ConnectionError(e.to_string()))?;

        info!("Meross MQTT connected and subscribed to {}", client_topic);
        self.connection = Some(Arc::new(Mutex::new(connection)));

        Ok(())
    }

    /// Build a Meross message
    fn build_message(&self, namespace: &str, method: &str, payload: Value) -> MerossMessage {
        let message_id = Self::generate_message_id();
        let timestamp = Self::get_timestamp();
        let sign = Self::calculate_sign(&message_id, &self.key, timestamp);

        MerossMessage {
            header: MerossHeader {
                message_id,
                method: method.to_string(),
                from: self.build_client_topic(),
                namespace: namespace.to_string(),
                timestamp,
                timestamp_ms: timestamp * 1000,
                sign,
                payload_version: 1,
            },
            payload,
        }
    }

    /// Send a command to a device
    async fn send_command(
        &self,
        device_uuid: &str,
        namespace: &str,
        method: &str,
        payload: Value,
    ) -> Result<Value, ProviderError> {
        let connection = self
            .connection
            .as_ref()
            .ok_or(ProviderError::ConnectionError(
                "Not connected to MQTT".to_string(),
            ))?;

        let message = self.build_message(namespace, method, payload);
        let topic = Self::build_device_topic(device_uuid);
        let response_filter = &message.header.message_id;

        let json = serde_json::to_vec(&message)
            .map_err(|e| ProviderError::Unknown(format!("Failed to serialize message: {}", e)))?;

        debug!("Sending MQTT command to {}: {:?}", topic, message);

        let conn = connection.lock().await;
        let response = conn
            .request(&topic, &json, response_filter, 10)
            .await
            .map_err(|e| ProviderError::ConnectionError(e.to_string()))?;

        let response_msg: MerossMessage = response
            .parse_json()
            .map_err(|e| ProviderError::Unknown(format!("Failed to parse response: {}", e)))?;

        Ok(response_msg.payload)
    }

    /// Turn device on
    pub async fn turn_on(&self, device_uuid: &str, channel: i32) -> Result<DeviceActionResult, ProviderError> {
        let payload = serde_json::json!({
            "togglex": {
                "channel": channel,
                "onoff": 1
            }
        });

        match self
            .send_command(device_uuid, "Appliance.Control.ToggleX", "SET", payload)
            .await
        {
            Ok(_) => Ok(DeviceActionResult {
                success: true,
                message: Some("Device turned on".to_string()),
                new_state: Some(DeviceState {
                    is_on: true,
                    brightness: None,
                    temperature: None,
                    power_consumption_watts: None,
                }),
            }),
            Err(e) => {
                error!("Failed to turn on device {}: {}", device_uuid, e);
                Ok(DeviceActionResult {
                    success: false,
                    message: Some(e.to_string()),
                    new_state: None,
                })
            }
        }
    }

    /// Turn device off
    pub async fn turn_off(&self, device_uuid: &str, channel: i32) -> Result<DeviceActionResult, ProviderError> {
        let payload = serde_json::json!({
            "togglex": {
                "channel": channel,
                "onoff": 0
            }
        });

        match self
            .send_command(device_uuid, "Appliance.Control.ToggleX", "SET", payload)
            .await
        {
            Ok(_) => Ok(DeviceActionResult {
                success: true,
                message: Some("Device turned off".to_string()),
                new_state: Some(DeviceState {
                    is_on: false,
                    brightness: None,
                    temperature: None,
                    power_consumption_watts: None,
                }),
            }),
            Err(e) => {
                error!("Failed to turn off device {}: {}", device_uuid, e);
                Ok(DeviceActionResult {
                    success: false,
                    message: Some(e.to_string()),
                    new_state: None,
                })
            }
        }
    }

    /// Get device state
    pub async fn get_state(&self, device_uuid: &str) -> Result<DeviceState, ProviderError> {
        let payload = serde_json::json!({});

        let response = self
            .send_command(device_uuid, "Appliance.System.All", "GET", payload)
            .await?;

        // Parse response
        let system_all: SystemAllResponse = serde_json::from_value(response)
            .map_err(|e| ProviderError::Unknown(format!("Failed to parse state: {}", e)))?;

        let is_on = if let Some(all) = system_all.all {
            if let Some(digest) = all.digest {
                if let Some(togglex) = digest.togglex {
                    togglex.first().map(|t| t.onoff == 1).unwrap_or(false)
                } else if let Some(toggle) = digest.toggle {
                    toggle.onoff == 1
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };

        Ok(DeviceState {
            is_on,
            brightness: None,
            temperature: None,
            power_consumption_watts: None,
        })
    }

    /// Disconnect from MQTT
    pub async fn disconnect(&self) -> Result<(), ProviderError> {
        if let Some(connection) = &self.connection {
            let conn = connection.lock().await;
            conn.disconnect()
                .await
                .map_err(|e| ProviderError::ConnectionError(e.to_string()))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_message_id() {
        let id1 = MerossMqttClient::generate_message_id();
        let id2 = MerossMqttClient::generate_message_id();

        assert_eq!(id1.len(), 32);
        assert_eq!(id2.len(), 32);
        // Should be different
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_calculate_sign() {
        let message_id = "test_message_id";
        let key = "test_key";
        let timestamp = 1234567890i64;

        let sign = MerossMqttClient::calculate_sign(message_id, key, timestamp);

        // Should be 32-character hex string
        assert_eq!(sign.len(), 32);
        assert!(sign.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_build_device_topic() {
        let uuid = "device-uuid-123";
        let topic = MerossMqttClient::build_device_topic(uuid);
        assert_eq!(topic, "/appliance/device-uuid-123/subscribe");
    }

    #[test]
    fn test_from_credentials_valid() {
        let credentials = serde_json::json!({
            "user_id": "user123",
            "key": "secret_key",
            "mqtt_domain": "eu-iotx.meross.com"
        });

        let client = MerossMqttClient::from_credentials(&credentials);
        assert!(client.is_ok());
        let client = client.unwrap();
        assert_eq!(client.user_id, "user123");
        assert_eq!(client.key, "secret_key");
        assert_eq!(client.mqtt_domain, "eu-iotx.meross.com");
    }

    #[test]
    fn test_from_credentials_missing_user_id() {
        let credentials = serde_json::json!({
            "key": "secret_key"
        });

        let client = MerossMqttClient::from_credentials(&credentials);
        assert!(client.is_err());
    }

    #[test]
    fn test_from_credentials_missing_key() {
        let credentials = serde_json::json!({
            "user_id": "user123"
        });

        let client = MerossMqttClient::from_credentials(&credentials);
        assert!(client.is_err());
    }

    #[test]
    fn test_from_credentials_default_mqtt_domain() {
        let credentials = serde_json::json!({
            "user_id": "user123",
            "key": "secret_key"
        });

        let client = MerossMqttClient::from_credentials(&credentials).unwrap();
        assert_eq!(client.mqtt_domain, "eu-iotx.meross.com");
    }

    #[test]
    fn test_meross_message_serialization() {
        let msg = MerossMessage {
            header: MerossHeader {
                message_id: "test123".to_string(),
                method: "SET".to_string(),
                from: "/app/test".to_string(),
                namespace: "Appliance.Control.ToggleX".to_string(),
                timestamp: 1234567890,
                timestamp_ms: 1234567890000,
                sign: "abc123".to_string(),
                payload_version: 1,
            },
            payload: serde_json::json!({"togglex": {"channel": 0, "onoff": 1}}),
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("Appliance.Control.ToggleX"));
        assert!(json.contains("togglex"));
    }
}
