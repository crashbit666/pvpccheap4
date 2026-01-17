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
