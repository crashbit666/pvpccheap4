//! Shelly through its cloud (Cloud Control API v2), with the "Authorization
//! cloud key" and server shown in the Shelly app (User settings).
//!
//! The v2 API has no call that lists an account's devices, so the user types
//! their ids (shown in the app). Each one is read with the v2 API, which
//! gives its channels, names and power meter. Only v2 calls are made: v1 is
//! deprecated.
//!
//! The cloud allows one call per second per account, so calls with the same
//! key are spaced out.

use super::{DeviceActionResult, DeviceState, DiscoveredDevice, ProviderError, SmartHomeProvider};
use async_trait::async_trait;
use log::{info, warn};
use reqwest::{Client, StatusCode, redirect::Policy};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::Instant;

/// Time between two calls with the same key (the cloud allows one a second)
const SPACING: Duration = Duration::from_millis(1200);
/// Device ids per v2 status call
const IDS_PER_CALL: usize = 10;

pub struct ShellyProvider;

impl ShellyProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ShellyProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// "shelly-103-eu.shelly.cloud", with or without https:// and a trailing /.
/// Only Shelly's own servers: the server calls this host with the key.
fn parse_server(raw: &str) -> Option<String> {
    let host = raw
        .trim()
        .trim_start_matches("https://")
        .trim_end_matches('/')
        .to_ascii_lowercase();
    let name = host.strip_suffix(".shelly.cloud")?;
    let valid = name.starts_with("shelly-")
        && name.len() > "shelly-".len()
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    valid.then_some(host)
}

/// Device ids are hex (the MAC), as the app shows them
fn parse_device_id(raw: &str) -> Option<String> {
    let id = raw.trim().to_ascii_lowercase();
    ((6..=16).contains(&id.len()) && id.chars().all(|c| c.is_ascii_hexdigit())).then_some(id)
}

/// Ids typed by the user, separated by commas, spaces or new lines
fn parse_device_ids(raw: &Value) -> Result<Vec<String>, ProviderError> {
    let items: Vec<String> = match raw {
        Value::String(s) => s.split([',', ' ', '\n', ';']).map(str::to_string).collect(),
        Value::Array(list) => list
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    };
    let mut ids = Vec::new();
    for item in items.iter().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        let id = parse_device_id(item).ok_or_else(|| {
            ProviderError::AuthenticationFailed(format!("Invalid Shelly device id: {}", item))
        })?;
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    Ok(ids)
}

/// What a device exposes on one channel
#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    Switch,
    Light,
}

/// One channel of a device: "b48a0a1cd978_0" or "light:b48a0a1cd978_0"
#[derive(Debug, Clone, PartialEq)]
struct Channel {
    id: String,
    channel: u32,
    kind: Kind,
}

impl Channel {
    fn external_id(&self) -> String {
        match self.kind {
            Kind::Switch => format!("{}_{}", self.id, self.channel),
            Kind::Light => format!("light:{}_{}", self.id, self.channel),
        }
    }

    fn parse(external_id: &str) -> Option<Self> {
        let (kind, rest) = match external_id.strip_prefix("light:") {
            Some(rest) => (Kind::Light, rest),
            None => (Kind::Switch, external_id),
        };
        let (id, channel) = rest.rsplit_once('_')?;
        Some(Self {
            id: parse_device_id(id)?,
            channel: channel.parse().ok()?,
            kind,
        })
    }
}

fn is_gen1(device: &Value) -> bool {
    device.get("gen").and_then(Value::as_str) == Some("G1")
}

fn device_name(device: &Value) -> Option<String> {
    let settings = device.get("settings")?;
    let name = if is_gen1(device) {
        settings.get("name")
    } else {
        settings
            .get("sys")
            .and_then(|s| s.get("device"))
            .and_then(|d| d.get("name"))
    };
    name.and_then(Value::as_str)
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(str::to_string)
}

/// The status of one channel: (on, watts if the channel measures them)
fn channel_status(device: &Value, ch: &Channel) -> Option<(bool, Option<f32>)> {
    let status = device.get("status")?;
    if is_gen1(device) {
        let list = if ch.kind == Kind::Light {
            "lights"
        } else {
            "relays"
        };
        let on = status
            .get(list)?
            .get(ch.channel as usize)?
            .get("ison")?
            .as_bool()?;
        // A plain Shelly 1 reports a meter that is always 0
        let measures = device.get("code").and_then(Value::as_str) != Some("SHSW-1");
        let watts = status
            .get("meters")
            .and_then(|m| m.get(ch.channel as usize))
            .and_then(|m| m.get("power"))
            .and_then(Value::as_f64)
            .filter(|_| measures);
        Some((on, watts.map(|w| w as f32)))
    } else {
        let prefix = if ch.kind == Kind::Light {
            "light"
        } else {
            "switch"
        };
        let component = status.get(format!("{}:{}", prefix, ch.channel))?;
        let on = component.get("output")?.as_bool()?;
        let watts = component.get("apower").and_then(Value::as_f64);
        Some((on, watts.map(|w| w as f32)))
    }
}

/// The switchable channels of a device from a v2 status call
fn device_channels(device: &Value) -> Vec<(Channel, String)> {
    let Some(id) = device
        .get("id")
        .and_then(Value::as_str)
        .and_then(parse_device_id)
    else {
        return Vec::new();
    };
    let Some(status) = device.get("status").and_then(Value::as_object) else {
        return Vec::new();
    };
    let base = device_name(device).unwrap_or_else(|| {
        let code = device
            .get("code")
            .and_then(Value::as_str)
            .unwrap_or("Shelly");
        format!("{} {}", code, id)
    });

    let mut channels: Vec<(Channel, Option<String>)> = Vec::new();
    if is_gen1(device) {
        for (list, kind) in [("relays", Kind::Switch), ("lights", Kind::Light)] {
            let count = status
                .get(list)
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            for n in 0..count {
                let name = device
                    .get("settings")
                    .and_then(|s| s.get(list))
                    .and_then(|l| l.get(n))
                    .and_then(|c| c.get("name"))
                    .and_then(Value::as_str)
                    .map(str::to_string);
                channels.push((
                    Channel {
                        id: id.clone(),
                        channel: n as u32,
                        kind,
                    },
                    name,
                ));
            }
        }
    } else {
        for key in status.keys() {
            let Some((prefix, n)) = key.split_once(':') else {
                continue;
            };
            let kind = match prefix {
                "switch" => Kind::Switch,
                "light" => Kind::Light,
                _ => continue,
            };
            let Ok(n) = n.parse::<u32>() else { continue };
            let name = device
                .get("settings")
                .and_then(|s| s.get(key))
                .and_then(|c| c.get("name"))
                .and_then(Value::as_str)
                .map(str::to_string);
            channels.push((
                Channel {
                    id: id.clone(),
                    channel: n,
                    kind,
                },
                name,
            ));
        }
        channels.sort_by_key(|(c, _)| (c.kind == Kind::Light, c.channel));
    }

    let several = channels.len() > 1;
    channels
        .into_iter()
        .map(|(ch, name)| {
            let name = match name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()) {
                Some(n) if n != base => format!("{} · {}", base, n),
                _ if several => format!("{} · {}", base, ch.channel + 1),
                _ => base.clone(),
            };
            (ch, name)
        })
        .collect()
}

/// When each key last called the cloud
fn last_calls() -> &'static Mutex<HashMap<String, Instant>> {
    static LAST: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();
    LAST.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Waits until this key may call the cloud again
async fn wait_turn(auth_key: &str) {
    let mut last = last_calls().lock().await;
    let now = Instant::now();
    let start = match last.get(auth_key) {
        Some(prev) if *prev + SPACING > now => *prev + SPACING,
        _ => now,
    };
    last.insert(auth_key.to_string(), start);
    drop(last);
    tokio::time::sleep_until(start).await;
}

/// One Shelly cloud account
struct Api {
    client: Client,
    server: String,
    auth_key: String,
    device_ids: Vec<String>,
}

impl Api {
    fn new(credentials: &Value) -> Result<Self, ProviderError> {
        let server = credentials
            .get("server")
            .and_then(Value::as_str)
            .and_then(parse_server)
            .ok_or_else(|| {
                ProviderError::AuthenticationFailed("Invalid Shelly server".to_string())
            })?;
        let auth_key = credentials
            .get("auth_key")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|k| !k.is_empty())
            .ok_or_else(|| ProviderError::AuthenticationFailed("No Shelly key".to_string()))?;
        let device_ids = parse_device_ids(credentials.get("device_ids").unwrap_or(&Value::Null))?;
        if device_ids.is_empty() {
            return Err(ProviderError::AuthenticationFailed(
                "No Shelly device ids".to_string(),
            ));
        }
        let client = Client::builder()
            .redirect(Policy::none())
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| ProviderError::Unknown(e.to_string()))?;
        Ok(Self {
            client,
            server,
            auth_key: auth_key.to_string(),
            device_ids,
        })
    }

    fn credentials(&self) -> Value {
        json!({ "server": self.server, "auth_key": self.auth_key, "device_ids": self.device_ids })
    }

    /// Sends a request, waiting for its turn and retrying once when the cloud
    /// says it is too soon
    async fn send(
        &self,
        build: impl Fn() -> reqwest::RequestBuilder,
    ) -> Result<reqwest::Response, ProviderError> {
        for attempt in 0..2 {
            wait_turn(&self.auth_key).await;
            let response = build().send().await.map_err(|e| {
                if e.is_timeout() {
                    ProviderError::Timeout
                } else {
                    ProviderError::ConnectionError(e.without_url().to_string())
                }
            })?;
            match response.status() {
                StatusCode::TOO_MANY_REQUESTS if attempt == 0 => continue,
                StatusCode::TOO_MANY_REQUESTS => return Err(ProviderError::RateLimited),
                StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                    return Err(ProviderError::InvalidCredentials);
                }
                s if s.is_success() => return Ok(response),
                s => {
                    let body: Value = response.json().await.unwrap_or(Value::Null);
                    let error = body.get("error").and_then(Value::as_str).unwrap_or("");
                    return Err(match error {
                        "DEVICE_NOT_FOUND" | "INSTANCE_NOT_FOUND" | "DEVICE_INVALID_CHANNEL" => {
                            ProviderError::DeviceNotFound(error.to_string())
                        }
                        "DEVICE_OFFLINE" => {
                            ProviderError::ConnectionError("The device is offline".to_string())
                        }
                        "" => ProviderError::ConnectionError(format!("Shelly answered {}", s)),
                        other => {
                            ProviderError::ConnectionError(format!("Shelly answered {}", other))
                        }
                    });
                }
            }
        }
        Err(ProviderError::RateLimited)
    }

    fn v2(&self, path: &str) -> String {
        format!(
            "https://{}/v2/devices/api/{}?auth_key={}",
            self.server,
            path,
            urlencoding::encode(&self.auth_key)
        )
    }

    /// v2 state of up to ten devices
    async fn get(
        &self,
        ids: &[String],
        select: &[&str],
        pick: Option<Value>,
    ) -> Result<Vec<Value>, ProviderError> {
        let mut body = json!({ "ids": ids, "select": select });
        if let Some(pick) = pick {
            body["pick"] = pick;
        }
        let url = self.v2("get");
        self.send(|| self.client.post(&url).json(&body))
            .await?
            .json::<Vec<Value>>()
            .await
            .map_err(|e| ProviderError::Unknown(e.to_string()))
    }

    /// The status of one channel, read with the v2 API
    async fn channel(&self, ch: &Channel) -> Result<(bool, Option<f32>), ProviderError> {
        let device = self
            .get(std::slice::from_ref(&ch.id), &["status"], None)
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| ProviderError::DeviceNotFound(ch.external_id()))?;
        if device.get("online").and_then(Value::as_i64) == Some(0) {
            return Err(ProviderError::ConnectionError(
                "The device is offline".to_string(),
            ));
        }
        channel_status(&device, ch).ok_or_else(|| ProviderError::DeviceNotFound(ch.external_id()))
    }

    async fn switch(
        &self,
        external_id: &str,
        on: bool,
    ) -> Result<DeviceActionResult, ProviderError> {
        let ch = Channel::parse(external_id)
            .ok_or_else(|| ProviderError::DeviceNotFound(external_id.to_string()))?;
        let url = self.v2(if ch.kind == Kind::Light {
            "set/light"
        } else {
            "set/switch"
        });
        let body = json!({ "id": ch.id, "channel": ch.channel, "on": on });
        match self.send(|| self.client.post(&url).json(&body)).await {
            Ok(_) => Ok(DeviceActionResult {
                success: true,
                message: Some(format!("Device turned {}", if on { "on" } else { "off" })),
                new_state: Some(DeviceState {
                    is_on: on,
                    brightness: None,
                    temperature: None,
                    power_consumption_watts: None,
                }),
            }),
            Err(e @ ProviderError::InvalidCredentials) => Err(e),
            Err(e) => {
                warn!(
                    "Shelly switch {} failed for {}: {}",
                    if on { "on" } else { "off" },
                    external_id,
                    e
                );
                Ok(DeviceActionResult {
                    success: false,
                    message: Some(e.to_string()),
                    new_state: None,
                })
            }
        }
    }
}

#[async_trait]
impl SmartHomeProvider for ShellyProvider {
    fn provider_name(&self) -> &'static str {
        "shelly"
    }

    fn display_name(&self) -> &'static str {
        "Shelly"
    }

    /// Checks the key with the typed devices, so a wrong key or id is
    /// reported when the integration is added
    async fn login(&self, credentials: &Value) -> Result<Value, ProviderError> {
        let api = Api::new(credentials)?;
        let mut found = 0;
        for chunk in api.device_ids.chunks(IDS_PER_CALL) {
            found += api
                .get(chunk, &["status"], Some(json!({ "status": ["sys"] })))
                .await?
                .len();
        }
        if found < api.device_ids.len() {
            return Err(ProviderError::AuthenticationFailed(
                "Shelly does not know some device ids".to_string(),
            ));
        }
        info!("Shelly login successful");
        Ok(api.credentials())
    }

    // The key does not expire (it changes with the password): nothing to refresh
    async fn refresh_credentials(&self, _credentials: &Value) -> Result<Value, ProviderError> {
        Err(ProviderError::InvalidCredentials)
    }

    async fn list_devices(
        &self,
        credentials: &Value,
    ) -> Result<Vec<DiscoveredDevice>, ProviderError> {
        let api = Api::new(credentials)?;
        let mut devices = Vec::new();
        for chunk in api.device_ids.chunks(IDS_PER_CALL) {
            for device in api.get(chunk, &["status", "settings"], None).await? {
                for (ch, name) in device_channels(&device) {
                    devices.push(DiscoveredDevice {
                        external_id: ch.external_id(),
                        name,
                        device_type: if ch.kind == Kind::Light {
                            "light"
                        } else {
                            "switch"
                        }
                        .to_string(),
                    });
                }
            }
        }
        info!(
            "Shelly found {} channels in {} devices",
            devices.len(),
            api.device_ids.len()
        );
        Ok(devices)
    }

    async fn get_device_state(
        &self,
        credentials: &Value,
        external_id: &str,
    ) -> Result<DeviceState, ProviderError> {
        let ch = Channel::parse(external_id)
            .ok_or_else(|| ProviderError::DeviceNotFound(external_id.to_string()))?;
        let (is_on, watts) = Api::new(credentials)?.channel(&ch).await?;
        Ok(DeviceState {
            is_on,
            brightness: None,
            temperature: None,
            power_consumption_watts: watts,
        })
    }

    async fn turn_on(
        &self,
        credentials: &Value,
        external_id: &str,
    ) -> Result<DeviceActionResult, ProviderError> {
        Api::new(credentials)?.switch(external_id, true).await
    }

    async fn turn_off(
        &self,
        credentials: &Value,
        external_id: &str,
    ) -> Result<DeviceActionResult, ProviderError> {
        Api::new(credentials)?.switch(external_id, false).await
    }

    async fn supports_power_reading(
        &self,
        credentials: &Value,
        external_id: &str,
    ) -> Result<bool, ProviderError> {
        let Some(ch) = Channel::parse(external_id) else {
            return Ok(false);
        };
        Ok(Api::new(credentials)?.channel(&ch).await?.1.is_some())
    }

    async fn read_power(
        &self,
        credentials: &Value,
        external_id: &str,
    ) -> Result<f32, ProviderError> {
        let ch = Channel::parse(external_id)
            .ok_or_else(|| ProviderError::DeviceNotFound(external_id.to_string()))?;
        Api::new(credentials)?
            .channel(&ch)
            .await?
            .1
            .ok_or_else(|| ProviderError::Unknown(format!("No power reading for {}", external_id)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_is_a_shelly_host() {
        assert_eq!(
            parse_server("shelly-103-eu.shelly.cloud").as_deref(),
            Some("shelly-103-eu.shelly.cloud")
        );
        assert_eq!(
            parse_server(" https://Shelly-7-EU.shelly.cloud/ ").as_deref(),
            Some("shelly-7-eu.shelly.cloud")
        );
        assert!(parse_server("evil.com/shelly-1.shelly.cloud").is_none());
        assert!(parse_server("shelly-1.shelly.cloud.evil.com").is_none());
        assert!(parse_server("shelly-1-eu.shelly.cloud:8080").is_none());
        assert!(parse_server("user@shelly-1-eu.shelly.cloud").is_none());
        assert!(parse_server("shelly-.shelly.cloud").is_none());
        assert!(parse_server("http://shelly-1-eu.shelly.cloud").is_none());
    }

    #[test]
    fn test_device_ids() {
        assert_eq!(
            parse_device_ids(&json!("B48A0A1CD978, e8db84d2a1b3 b48a0a1cd978")).unwrap(),
            ["b48a0a1cd978", "e8db84d2a1b3"]
        );
        assert!(parse_device_ids(&json!("")).unwrap().is_empty());
        assert!(parse_device_ids(&Value::Null).unwrap().is_empty());
        assert!(parse_device_ids(&json!("b48a0a1cd978/../x")).is_err());
        assert_eq!(parse_device_ids(&json!(["abcdef"])).unwrap(), ["abcdef"]);
    }

    #[test]
    fn test_ids_are_required() {
        let creds =
            json!({ "server": "shelly-103-eu.shelly.cloud", "auth_key": "k", "device_ids": " " });
        assert!(matches!(
            Api::new(&creds),
            Err(ProviderError::AuthenticationFailed(_))
        ));
    }

    #[test]
    fn test_channel_ids() {
        let ch = Channel {
            id: "b48a0a1cd978".into(),
            channel: 1,
            kind: Kind::Switch,
        };
        assert_eq!(ch.external_id(), "b48a0a1cd978_1");
        assert_eq!(Channel::parse("b48a0a1cd978_1"), Some(ch));
        let light = Channel::parse("light:b48a0a1cd978_0").unwrap();
        assert_eq!(light.kind, Kind::Light);
        assert_eq!(light.external_id(), "light:b48a0a1cd978_0");
        assert!(Channel::parse("b48a0a1cd978").is_none());
        assert!(Channel::parse("xyz_0").is_none());
    }

    fn gen2_plug() -> Value {
        json!({
            "id": "b48a0a1cd978", "type": "relay", "code": "SNPL-00112EU", "gen": "G2", "online": 1,
            "status": {
                "sys": {"uptime": 10},
                "switch:0": {"id": 0, "output": true, "apower": 1534.2, "voltage": 231.0}
            },
            "settings": {"sys": {"device": {"name": "Termo"}}, "switch:0": {"name": null}}
        })
    }

    #[test]
    fn test_gen2_plug() {
        let device = gen2_plug();
        let channels = device_channels(&device);
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].0.external_id(), "b48a0a1cd978_0");
        assert_eq!(channels[0].1, "Termo");
        assert_eq!(
            channel_status(&device, &channels[0].0),
            Some((true, Some(1534.2)))
        );
    }

    #[test]
    fn test_gen2_two_channels_without_meter() {
        let device = json!({
            "id": "a0b1c2d3e4f5", "code": "S3SW-002P16EU", "gen": "G3", "online": 1,
            "status": {"switch:1": {"output": false}, "switch:0": {"output": true}, "input:0": {"state": false}},
            "settings": {"switch:0": {"name": "Rentadora"}}
        });
        let channels = device_channels(&device);
        let names: Vec<_> = channels.iter().map(|(_, n)| n.as_str()).collect();
        assert_eq!(
            names,
            [
                "S3SW-002P16EU a0b1c2d3e4f5 · Rentadora",
                "S3SW-002P16EU a0b1c2d3e4f5 · 2"
            ]
        );
        assert_eq!(channel_status(&device, &channels[1].0), Some((false, None)));
    }

    #[test]
    fn test_gen1_devices() {
        let plug = json!({
            "id": "e8db84d2a1b3", "code": "SHPLG-S", "gen": "G1", "online": 1,
            "status": {"relays": [{"ison": true}], "meters": [{"power": 820.5, "is_valid": true}]},
            "settings": {"name": "Assecadora", "relays": [{"name": null}]}
        });
        let channels = device_channels(&plug);
        assert_eq!(channels[0].1, "Assecadora");
        assert_eq!(
            channel_status(&plug, &channels[0].0),
            Some((true, Some(820.5)))
        );

        let shelly1 = json!({
            "id": "c45bbe6b1a2f", "code": "SHSW-1", "gen": "G1",
            "status": {"relays": [{"ison": false}], "meters": [{"power": 0, "is_valid": true}]}
        });
        let channels = device_channels(&shelly1);
        assert_eq!(channels[0].1, "SHSW-1 c45bbe6b1a2f");
        assert_eq!(
            channel_status(&shelly1, &channels[0].0),
            Some((false, None))
        );
    }

    #[test]
    fn test_missing_channel_is_none() {
        let ch = Channel {
            id: "b48a0a1cd978".into(),
            channel: 3,
            kind: Kind::Switch,
        };
        assert_eq!(channel_status(&gen2_plug(), &ch), None);
        assert!(device_channels(&json!({"id": "nothex!", "status": {}})).is_empty());
    }

    #[tokio::test]
    async fn test_calls_with_one_key_are_spaced() {
        let start = Instant::now();
        wait_turn("test-key-spacing").await;
        wait_turn("test-key-spacing").await;
        wait_turn("another-key").await;
        assert!(start.elapsed() >= SPACING);
        assert!(start.elapsed() < SPACING * 2);
    }

    /// Against a real account:
    /// SHELLY_SERVER=shelly-XX-eu.shelly.cloud SHELLY_KEY=... SHELLY_IDS=id1,id2 \
    ///   cargo test -p backend shelly -- --ignored
    #[tokio::test]
    #[ignore]
    async fn test_live_shelly() {
        let creds = json!({
            "server": std::env::var("SHELLY_SERVER").unwrap(),
            "auth_key": std::env::var("SHELLY_KEY").unwrap(),
            "device_ids": std::env::var("SHELLY_IDS").unwrap(),
        });
        let shelly = ShellyProvider::new();
        let creds = shelly.login(&creds).await.unwrap();
        let devices = shelly.list_devices(&creds).await.unwrap();
        assert!(!devices.is_empty());
        for d in &devices {
            let state = shelly
                .get_device_state(&creds, &d.external_id)
                .await
                .unwrap();
            println!(
                "{} {} on={} watts={:?}",
                d.external_id, d.name, state.is_on, state.power_consumption_watts
            );
        }
        let bad = json!({ "server": creds["server"], "auth_key": "wrong", "device_ids": creds["device_ids"] });
        assert!(shelly.login(&bad).await.is_err());
    }
}
