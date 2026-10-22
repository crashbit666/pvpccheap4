//! Home Assistant through its REST API, with a long-lived access token.
//! Any switch, light or input_boolean in HA can be scheduled, so every
//! brand HA supports (Shelly, Tuya, Zigbee...) works through it.
//!
//! The URL is typed by the user and the server calls it, so it must not
//! reach the server's own network: only https to public addresses, unless
//! HA_ALLOW_PRIVATE_URLS=1 (self-hosted backend in the same home network).

use super::{DeviceActionResult, DeviceState, DiscoveredDevice, ProviderError, SmartHomeProvider};
use async_trait::async_trait;
use log::{info, warn};
use reqwest::{Client, StatusCode, Url, redirect::Policy};
use serde_json::{Value, json};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;

/// Entity domains that can be switched on and off
const SWITCHABLE: [&str; 3] = ["switch", "light", "input_boolean"];

pub struct HomeAssistantProvider;

impl HomeAssistantProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for HomeAssistantProvider {
    fn default() -> Self {
        Self::new()
    }
}

fn allow_private() -> bool {
    std::env::var("HA_ALLOW_PRIVATE_URLS").is_ok_and(|v| v == "1")
}

fn is_public_v4(ip: Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    !(ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_unspecified()
        || ip.is_documentation()
        || ip.is_multicast()
        || a == 0
        || (a == 100 && (64..128).contains(&b)) // carrier-grade NAT
        || a >= 240)
}

fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_public_v4(v4),
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_public_v4(v4);
            }
            let first = v6.segments()[0];
            !(v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (first & 0xfe00) == 0xfc00 // unique local
                || (first & 0xffc0) == 0xfe80 // link local
                || v6 == Ipv6Addr::UNSPECIFIED)
        }
    }
}

/// Parses the URL the user typed: https, no user or path, a host
fn parse_base_url(raw: &str, allow_private: bool) -> Result<Url, ProviderError> {
    let invalid = |why: &str| ProviderError::AuthenticationFailed(format!("Invalid Home Assistant URL: {}", why));
    let mut url = Url::parse(raw.trim()).map_err(|_| invalid("not a URL"))?;
    match url.scheme() {
        "https" => {}
        "http" if allow_private => {}
        _ => return Err(invalid("it must start with https://")),
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(invalid("it must not contain a user or password"));
    }
    if url.host_str().is_none_or(str::is_empty) {
        return Err(invalid("no host"));
    }
    url.set_path("");
    url.set_query(None);
    url.set_fragment(None);
    Ok(url)
}

/// Resolves the host once and checks the address, so the request goes to
/// the address that was checked
async fn resolve(url: &Url, allow_private: bool) -> Result<SocketAddr, ProviderError> {
    let host = url.host_str().unwrap_or_default().trim_start_matches('[').trim_end_matches(']');
    let port = url.port_or_known_default().unwrap_or(443);
    let addrs: Vec<SocketAddr> = tokio::net::lookup_host((host, port))
        .await
        .map_err(|_| ProviderError::ConnectionError(format!("Cannot resolve {}", host)))?
        .collect();
    if addrs.is_empty() {
        return Err(ProviderError::ConnectionError(format!("Cannot resolve {}", host)));
    }
    if !allow_private && addrs.iter().any(|a| !is_public(a.ip())) {
        return Err(ProviderError::AuthenticationFailed(
            "Invalid Home Assistant URL: it points to a private address".to_string(),
        ));
    }
    Ok(addrs[0])
}

/// Entity ids go into URLs and templates, so only the HA format is allowed
fn valid_entity_id(entity_id: &str) -> bool {
    let Some((domain, object)) = entity_id.split_once('.') else { return false };
    !domain.is_empty()
        && !object.is_empty()
        && domain.chars().all(|c| c.is_ascii_lowercase() || c == '_')
        && object.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// Devices from GET /api/states: the switchable entities
fn switchable_entities(states: &Value) -> Vec<DiscoveredDevice> {
    states
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|s| {
                    let entity_id = s.get("entity_id")?.as_str()?;
                    let domain = entity_id.split('.').next()?;
                    if !SWITCHABLE.contains(&domain) || !valid_entity_id(entity_id) {
                        return None;
                    }
                    let name = s
                        .get("attributes")
                        .and_then(|a| a.get("friendly_name"))
                        .and_then(Value::as_str)
                        .unwrap_or(entity_id);
                    Some(DiscoveredDevice {
                        external_id: entity_id.to_string(),
                        name: name.to_string(),
                        device_type: if domain == "light" { "light" } else { "switch" }.to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Template that prints the power sensor of the entity's device as
/// "value|unit", or nothing when the device has none
fn power_template(entity_id: &str) -> String {
    format!(
        "{{% set d = device_id('{entity_id}') %}}\
         {{% set s = (device_entities(d) if d else []) | select('is_state_attr', 'device_class', 'power') | first | default('') %}}\
         {{{{ (states(s) ~ '|' ~ (state_attr(s, 'unit_of_measurement') or '')) if s else '' }}}}"
    )
}

/// "1500|W" or "1.5|kW" to watts
fn parse_power(rendered: &str) -> Option<f32> {
    let (value, unit) = rendered.trim().split_once('|')?;
    let value: f32 = value.trim().parse().ok()?;
    match unit.trim() {
        "kW" => Some(value * 1000.0),
        "W" | "" => Some(value),
        _ => None,
    }
}

/// A checked connection to one Home Assistant
struct Api {
    client: Client,
    base: Url,
    token: String,
}

impl Api {
    async fn new(credentials: &Value) -> Result<Self, ProviderError> {
        let allow = allow_private();
        let raw = credentials.get("url").and_then(Value::as_str).unwrap_or_default();
        let token = credentials
            .get("token")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .ok_or_else(|| ProviderError::AuthenticationFailed("No access token".to_string()))?;
        let base = parse_base_url(raw, allow)?;
        let addr = resolve(&base, allow).await?;
        let client = Client::builder()
            .redirect(Policy::none())
            .timeout(Duration::from_secs(10))
            .resolve(base.host_str().unwrap_or_default(), addr)
            .build()
            .map_err(|e| ProviderError::Unknown(e.to_string()))?;
        Ok(Self { client, base, token: token.to_string() })
    }

    fn url(&self, path: &str) -> Result<Url, ProviderError> {
        self.base.join(path).map_err(|e| ProviderError::Unknown(e.to_string()))
    }

    async fn send(&self, request: reqwest::RequestBuilder) -> Result<reqwest::Response, ProviderError> {
        let response = request
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|e| if e.is_timeout() { ProviderError::Timeout } else { ProviderError::ConnectionError(e.without_url().to_string()) })?;
        match response.status() {
            s if s.is_success() => Ok(response),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(ProviderError::InvalidCredentials),
            StatusCode::NOT_FOUND => Err(ProviderError::DeviceNotFound("Not found in Home Assistant".to_string())),
            s => Err(ProviderError::ConnectionError(format!("Home Assistant answered {}", s))),
        }
    }

    async fn get(&self, path: &str) -> Result<Value, ProviderError> {
        self.send(self.client.get(self.url(path)?))
            .await?
            .json()
            .await
            .map_err(|e| ProviderError::Unknown(e.to_string()))
    }

    async fn post(&self, path: &str, body: &Value) -> Result<reqwest::Response, ProviderError> {
        self.send(self.client.post(self.url(path)?).json(body)).await
    }

    async fn render(&self, template: &str) -> Result<String, ProviderError> {
        self.post("/api/template", &json!({ "template": template }))
            .await?
            .text()
            .await
            .map_err(|e| ProviderError::Unknown(e.to_string()))
    }

    async fn switch(&self, entity_id: &str, on: bool) -> Result<DeviceActionResult, ProviderError> {
        if !valid_entity_id(entity_id) {
            return Err(ProviderError::DeviceNotFound(entity_id.to_string()));
        }
        let domain = entity_id.split('.').next().unwrap_or_default();
        let service = if on { "turn_on" } else { "turn_off" };
        match self.post(&format!("/api/services/{domain}/{service}"), &json!({ "entity_id": entity_id })).await {
            Ok(_) => Ok(DeviceActionResult {
                success: true,
                message: Some(format!("Device turned {}", if on { "on" } else { "off" })),
                new_state: Some(DeviceState { is_on: on, brightness: None, temperature: None, power_consumption_watts: None }),
            }),
            Err(e @ ProviderError::InvalidCredentials) => Err(e),
            Err(e) => {
                warn!("Home Assistant {} failed for {}: {}", service, entity_id, e);
                Ok(DeviceActionResult { success: false, message: Some(e.to_string()), new_state: None })
            }
        }
    }
}

#[async_trait]
impl SmartHomeProvider for HomeAssistantProvider {
    fn provider_name(&self) -> &'static str {
        "home_assistant"
    }

    fn display_name(&self) -> &'static str {
        "Home Assistant"
    }

    async fn login(&self, credentials: &Value) -> Result<Value, ProviderError> {
        let api = Api::new(credentials).await?;
        let answer = api.get("/api/").await?;
        if answer.get("message").is_none() {
            return Err(ProviderError::ConnectionError("It does not look like Home Assistant".to_string()));
        }
        info!("Home Assistant login successful");
        Ok(json!({ "url": api.base.as_str().trim_end_matches('/'), "token": api.token }))
    }

    // A long-lived token does not expire: nothing to refresh
    async fn refresh_credentials(&self, _credentials: &Value) -> Result<Value, ProviderError> {
        Err(ProviderError::InvalidCredentials)
    }

    async fn list_devices(&self, credentials: &Value) -> Result<Vec<DiscoveredDevice>, ProviderError> {
        let api = Api::new(credentials).await?;
        let devices = switchable_entities(&api.get("/api/states").await?);
        info!("Home Assistant found {} switchable entities", devices.len());
        Ok(devices)
    }

    async fn get_device_state(&self, credentials: &Value, external_id: &str) -> Result<DeviceState, ProviderError> {
        if !valid_entity_id(external_id) {
            return Err(ProviderError::DeviceNotFound(external_id.to_string()));
        }
        let api = Api::new(credentials).await?;
        let state = api.get(&format!("/api/states/{external_id}")).await?;
        match state.get("state").and_then(Value::as_str) {
            Some("on") => Ok(DeviceState { is_on: true, brightness: None, temperature: None, power_consumption_watts: None }),
            Some("off") => Ok(DeviceState { is_on: false, brightness: None, temperature: None, power_consumption_watts: None }),
            // unavailable / unknown: not "off", callers keep the cached state
            other => Err(ProviderError::ConnectionError(format!("State is {}", other.unwrap_or("missing")))),
        }
    }

    async fn turn_on(&self, credentials: &Value, external_id: &str) -> Result<DeviceActionResult, ProviderError> {
        Api::new(credentials).await?.switch(external_id, true).await
    }

    async fn turn_off(&self, credentials: &Value, external_id: &str) -> Result<DeviceActionResult, ProviderError> {
        Api::new(credentials).await?.switch(external_id, false).await
    }

    async fn supports_power_reading(&self, credentials: &Value, external_id: &str) -> Result<bool, ProviderError> {
        if !valid_entity_id(external_id) {
            return Ok(false);
        }
        let rendered = Api::new(credentials).await?.render(&power_template(external_id)).await?;
        Ok(!rendered.trim().is_empty())
    }

    async fn read_power(&self, credentials: &Value, external_id: &str) -> Result<f32, ProviderError> {
        if !valid_entity_id(external_id) {
            return Err(ProviderError::DeviceNotFound(external_id.to_string()));
        }
        let rendered = Api::new(credentials).await?.render(&power_template(external_id)).await?;
        parse_power(&rendered).ok_or_else(|| ProviderError::Unknown(format!("No power reading for {}", external_id)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_url_rules() {
        assert_eq!(parse_base_url("https://casa.ui.nabu.casa/lovelace/0?x=1", false).unwrap().as_str(), "https://casa.ui.nabu.casa/");
        assert!(parse_base_url("http://casa.example.com", false).is_err());
        assert!(parse_base_url("http://192.168.1.10:8123", true).is_ok());
        assert!(parse_base_url("https://user:pw@casa.example.com", false).is_err());
        assert!(parse_base_url("file:///etc/passwd", true).is_err());
        assert!(parse_base_url("not a url", false).is_err());
    }

    #[test]
    fn test_private_addresses_are_not_public() {
        for ip in ["127.0.0.1", "10.1.2.3", "172.16.0.1", "192.168.1.1", "169.254.169.254", "100.64.0.1", "0.0.0.0", "::1", "fd00::1", "fe80::1", "::ffff:127.0.0.1"] {
            assert!(!is_public(ip.parse().unwrap()), "{ip} should be private");
        }
        for ip in ["8.8.8.8", "135.181.255.121", "2a01:4f9::1"] {
            assert!(is_public(ip.parse().unwrap()), "{ip} should be public");
        }
    }

    #[tokio::test]
    async fn test_localhost_is_refused() {
        let url = parse_base_url("https://localhost:8123", false).unwrap();
        assert!(resolve(&url, false).await.is_err());
    }

    #[test]
    fn test_entity_ids() {
        assert!(valid_entity_id("switch.termo_2"));
        assert!(!valid_entity_id("switch.termo'); {{ x"));
        assert!(!valid_entity_id("switch"));
        assert!(!valid_entity_id("../api/states"));
    }

    #[test]
    fn test_switchable_entities() {
        let states = json!([
            {"entity_id": "switch.termo", "state": "off", "attributes": {"friendly_name": "Termo"}},
            {"entity_id": "light.sala", "state": "on", "attributes": {}},
            {"entity_id": "sensor.termo_power", "state": "0", "attributes": {}},
            {"entity_id": "input_boolean.carregar", "state": "off", "attributes": {"friendly_name": "Carregar"}}
        ]);
        let devices = switchable_entities(&states);
        let ids: Vec<_> = devices.iter().map(|d| d.external_id.as_str()).collect();
        assert_eq!(ids, ["switch.termo", "light.sala", "input_boolean.carregar"]);
        assert_eq!(devices[0].name, "Termo");
        assert_eq!(devices[1].name, "light.sala");
        assert_eq!(devices[1].device_type, "light");
    }

    #[test]
    fn test_parse_power() {
        assert_eq!(parse_power("1500.5|W"), Some(1500.5));
        assert_eq!(parse_power(" 1.5|kW\n"), Some(1500.0));
        assert_eq!(parse_power("unavailable|W"), None);
        assert_eq!(parse_power(""), None);
    }

    /// Against a real Home Assistant (e.g. a test container with `demo:`):
    /// HA_ALLOW_PRIVATE_URLS=1 HA_TEST_URL=http://127.0.0.1:8123 HA_TEST_TOKEN=... \
    ///   cargo test -p backend home_assistant -- --ignored
    #[tokio::test]
    #[ignore]
    async fn test_live_home_assistant() {
        let creds = json!({
            "url": std::env::var("HA_TEST_URL").unwrap(),
            "token": std::env::var("HA_TEST_TOKEN").unwrap(),
        });
        let ha = HomeAssistantProvider::new();
        let creds = ha.login(&creds).await.unwrap();
        let devices = ha.list_devices(&creds).await.unwrap();
        assert!(devices.iter().any(|d| d.external_id == "switch.ac"));

        assert!(ha.turn_on(&creds, "switch.ac").await.unwrap().success);
        assert!(ha.get_device_state(&creds, "switch.ac").await.unwrap().is_on);
        assert!(ha.turn_off(&creds, "switch.ac").await.unwrap().success);
        assert!(!ha.get_device_state(&creds, "switch.ac").await.unwrap().is_on);

        // The demo AC device has no power sensor of its own
        assert!(!ha.supports_power_reading(&creds, "switch.ac").await.unwrap());
        assert!(matches!(
            ha.get_device_state(&creds, "switch.does_not_exist").await,
            Err(ProviderError::DeviceNotFound(_))
        ));
        let bad = json!({ "url": creds["url"], "token": "wrong" });
        assert!(matches!(ha.login(&bad).await, Err(ProviderError::InvalidCredentials)));
    }

    #[test]
    fn test_power_template_names_the_entity() {
        let t = power_template("switch.termo");
        assert!(t.contains("device_id('switch.termo')"));
        assert!(t.starts_with("{% set d"));
        assert!(t.contains("{{ (states(s)"));
    }
}
