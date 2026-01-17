use async_trait::async_trait;
use serde_json::Value;

#[async_trait]
pub trait SmartHomeProvider {
    /// Identifier for this provider (e.g., "meross", "tuya")
    fn provider_name(&self) -> &str;

    /// Validates credentials and returns a session token/data object
    async fn login(&self, credentials: &Value) -> Result<Value, String>;

    /// Lists devices available in the user's account
    /// Returns a list of (external_id, name, type)
    async fn list_devices(&self, credentials: &Value) -> Result<Vec<DiscoveredDevice>, String>;

    /// Turns a device on
    async fn turn_on(&self, credentials: &Value, external_id: &str) -> Result<(), String>;

    /// Turns a device off
    async fn turn_off(&self, credentials: &Value, external_id: &str) -> Result<(), String>;
}

#[derive(Debug)]
pub struct DiscoveredDevice {
    pub external_id: String,
    pub name: String,
    pub device_type: String, // "switch", "light", etc.
}

pub mod meross;
