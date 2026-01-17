use super::{DiscoveredDevice, SmartHomeProvider};
use async_trait::async_trait;
use serde_json::Value;

pub struct MerossProvider;

#[async_trait]
impl SmartHomeProvider for MerossProvider {
    fn provider_name(&self) -> &str {
        "meross"
    }

    async fn login(&self, _credentials: &Value) -> Result<Value, String> {
        // TODO: Implement real Meross Login (MD5 signing etc)
        // For now, return a dummy token
        Ok(serde_json::json!({ "token": "dummy_meross_token", "key": "dummy_key" }))
    }

    async fn list_devices(&self, _credentials: &Value) -> Result<Vec<DiscoveredDevice>, String> {
        // TODO: Call Meross API to list devices
        Ok(vec![DiscoveredDevice {
            external_id: "uuid_123".to_string(),
            name: "Living Room Plug".to_string(),
            device_type: "switch".to_string(),
        }])
    }

    async fn turn_on(&self, _credentials: &Value, external_id: &str) -> Result<(), String> {
        println!("Meross: Turning ON device {}", external_id);
        Ok(())
    }

    async fn turn_off(&self, _credentials: &Value, external_id: &str) -> Result<(), String> {
        println!("Meross: Turning OFF device {}", external_id);
        Ok(())
    }
}
