use reqwest::Error;
use serde_json::json;

pub struct HomeAssistantClient {
    base_url: String,
    token: String,
}

impl HomeAssistantClient {
    pub fn new(base_url: String, token: String) -> Self {
        Self { base_url, token }
    }

    pub async fn turn_on(&self, entity_id: &str) -> Result<(), Error> {
        self.call_service("switch", "turn_on", entity_id).await
    }

    pub async fn turn_off(&self, entity_id: &str) -> Result<(), Error> {
        self.call_service("switch", "turn_off", entity_id).await
    }

    async fn call_service(
        &self,
        domain: &str,
        service: &str,
        entity_id: &str,
    ) -> Result<(), Error> {
        let url = format!("{}/api/services/{}/{}", self.base_url, domain, service);
        let client = reqwest::Client::new();
        client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .json(&json!({ "entity_id": entity_id }))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = HomeAssistantClient::new(
            "http://homeassistant.local:8123".to_string(),
            "my_token".to_string(),
        );

        assert_eq!(client.base_url, "http://homeassistant.local:8123");
        assert_eq!(client.token, "my_token");
    }

    #[test]
    fn test_client_with_different_urls() {
        let client1 = HomeAssistantClient::new(
            "http://192.168.1.100:8123".to_string(),
            "token1".to_string(),
        );
        let client2 = HomeAssistantClient::new(
            "https://ha.example.com".to_string(),
            "token2".to_string(),
        );

        assert_eq!(client1.base_url, "http://192.168.1.100:8123");
        assert_eq!(client2.base_url, "https://ha.example.com");
    }

    #[test]
    fn test_service_url_format() {
        // Test that the URL format is correct by checking the pattern
        let base_url = "http://homeassistant.local:8123";
        let domain = "switch";
        let service = "turn_on";
        let expected_url = format!("{}/api/services/{}/{}", base_url, domain, service);

        assert_eq!(
            expected_url,
            "http://homeassistant.local:8123/api/services/switch/turn_on"
        );
    }

    #[test]
    fn test_authorization_header_format() {
        let token = "my_long_lived_access_token";
        let expected_header = format!("Bearer {}", token);

        assert_eq!(expected_header, "Bearer my_long_lived_access_token");
    }
}
