use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

pub mod meross;
pub mod meross_mqtt;
pub mod mqtt;

// Re-export providers
pub use meross::MerossProvider;

/// Device discovered from a provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredDevice {
    pub external_id: String,
    pub name: String,
    pub device_type: String,
}

/// Device capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCapabilities {
    pub can_toggle: bool,
    pub can_dim: bool,
    pub can_set_temperature: bool,
    pub can_read_consumption: bool,
}

impl Default for DeviceCapabilities {
    fn default() -> Self {
        Self {
            can_toggle: true,
            can_dim: false,
            can_set_temperature: false,
            can_read_consumption: false,
        }
    }
}

/// Result of a device action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceActionResult {
    pub success: bool,
    pub message: Option<String>,
    pub new_state: Option<DeviceState>,
}

/// Current state of a device
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceState {
    pub is_on: bool,
    pub brightness: Option<u8>,
    pub temperature: Option<f32>,
    pub power_consumption_watts: Option<f32>,
}

/// Error types for provider operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProviderError {
    AuthenticationFailed(String),
    DeviceNotFound(String),
    ConnectionError(String),
    RateLimited,
    InvalidCredentials,
    Timeout,
    Unknown(String),
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderError::AuthenticationFailed(msg) => write!(f, "Authentication failed: {}", msg),
            ProviderError::DeviceNotFound(id) => write!(f, "Device not found: {}", id),
            ProviderError::ConnectionError(msg) => write!(f, "Connection error: {}", msg),
            ProviderError::RateLimited => write!(f, "Rate limited by provider"),
            ProviderError::InvalidCredentials => write!(f, "Invalid credentials"),
            ProviderError::Timeout => write!(f, "Request timeout"),
            ProviderError::Unknown(msg) => write!(f, "Unknown error: {}", msg),
        }
    }
}

impl std::error::Error for ProviderError {}

/// Main trait for smart home providers
#[async_trait]
pub trait SmartHomeProvider: Send + Sync {
    /// Unique identifier for this provider (e.g., "meross", "tuya")
    fn provider_name(&self) -> &'static str;

    /// Human-readable display name
    fn display_name(&self) -> &'static str;

    /// Validates credentials and returns session data
    async fn login(&self, credentials: &Value) -> Result<Value, ProviderError>;

    /// Lists all devices available in the user's account
    async fn list_devices(&self, credentials: &Value) -> Result<Vec<DiscoveredDevice>, ProviderError>;

    /// Gets the current state of a device
    async fn get_device_state(
        &self,
        credentials: &Value,
        external_id: &str,
    ) -> Result<DeviceState, ProviderError>;

    /// Turns a device on
    async fn turn_on(
        &self,
        credentials: &Value,
        external_id: &str,
    ) -> Result<DeviceActionResult, ProviderError>;

    /// Turns a device off
    async fn turn_off(
        &self,
        credentials: &Value,
        external_id: &str,
    ) -> Result<DeviceActionResult, ProviderError>;

    /// Get device capabilities
    fn get_capabilities(&self) -> DeviceCapabilities {
        DeviceCapabilities::default()
    }

    /// Refresh credentials (re-login to get new token)
    /// Returns updated credentials with new token if successful
    async fn refresh_credentials(&self, credentials: &Value) -> Result<Value, ProviderError> {
        // Default implementation: just call login which will re-authenticate
        self.login(credentials).await
    }
}

/// Registry for managing multiple providers
#[derive(Clone)]
pub struct ProviderRegistry {
    providers: HashMap<String, Arc<dyn SmartHomeProvider>>,
}

impl ProviderRegistry {
    /// Create a new registry with all available providers
    pub fn new() -> Self {
        let mut registry = Self {
            providers: HashMap::new(),
        };

        // Register all available providers
        registry.register(Arc::new(MerossProvider::new()));
        // Future: registry.register(Arc::new(TuyaProvider::new()));
        // Future: registry.register(Arc::new(ShellyProvider::new()));

        registry
    }

    /// Register a new provider
    pub fn register(&mut self, provider: Arc<dyn SmartHomeProvider>) {
        self.providers
            .insert(provider.provider_name().to_string(), provider);
    }

    /// Get a provider by name
    pub fn get(&self, name: &str) -> Option<Arc<dyn SmartHomeProvider>> {
        self.providers.get(name).cloned()
    }

    /// List all available provider names
    pub fn available_providers(&self) -> Vec<&str> {
        self.providers.keys().map(|s| s.as_str()).collect()
    }

    /// Check if a provider exists
    pub fn has_provider(&self, name: &str) -> bool {
        self.providers.contains_key(name)
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_registry_creation() {
        let registry = ProviderRegistry::new();
        assert!(registry.has_provider("meross"));
    }

    #[test]
    fn test_provider_registry_get() {
        let registry = ProviderRegistry::new();
        let provider = registry.get("meross");
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().provider_name(), "meross");
    }

    #[test]
    fn test_provider_registry_unknown() {
        let registry = ProviderRegistry::new();
        assert!(registry.get("unknown_provider").is_none());
    }

    #[test]
    fn test_available_providers() {
        let registry = ProviderRegistry::new();
        let providers = registry.available_providers();
        assert!(providers.contains(&"meross"));
    }

    #[test]
    fn test_device_capabilities_default() {
        let caps = DeviceCapabilities::default();
        assert!(caps.can_toggle);
        assert!(!caps.can_dim);
    }

    #[test]
    fn test_provider_error_display() {
        let err = ProviderError::AuthenticationFailed("bad token".to_string());
        assert!(err.to_string().contains("Authentication failed"));
    }
}
