//! Generic MQTT utilities for smart home integrations
//!
//! This module provides shared MQTT functionality that can be used by
//! different smart home providers (Meross, Tuya, Shelly, etc.)

use log::{debug, error, info, warn};
use rumqttc::{AsyncClient, Event, EventLoop, MqttOptions, Packet, QoS, Transport};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, oneshot, Mutex};
use tokio::time::timeout;
use tokio_rustls::rustls::ClientConfig;

/// Configuration for MQTT connection
#[derive(Debug, Clone)]
pub struct MqttConfig {
    pub broker_host: String,
    pub broker_port: u16,
    pub client_id: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub use_tls: bool,
    pub use_websocket: bool,
    pub keep_alive_secs: u64,
}

impl Default for MqttConfig {
    fn default() -> Self {
        Self {
            broker_host: "localhost".to_string(),
            broker_port: 1883,
            client_id: format!("pvpc-cheap-{}", uuid::Uuid::new_v4()),
            username: None,
            password: None,
            use_tls: false,
            use_websocket: false,
            keep_alive_secs: 30,
        }
    }
}

/// Error types for MQTT operations
#[derive(Debug, Clone)]
pub enum MqttError {
    ConnectionFailed(String),
    SubscribeFailed(String),
    PublishFailed(String),
    Timeout,
    Disconnected,
    InvalidResponse(String),
}

impl std::fmt::Display for MqttError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MqttError::ConnectionFailed(msg) => write!(f, "MQTT connection failed: {}", msg),
            MqttError::SubscribeFailed(msg) => write!(f, "MQTT subscribe failed: {}", msg),
            MqttError::PublishFailed(msg) => write!(f, "MQTT publish failed: {}", msg),
            MqttError::Timeout => write!(f, "MQTT operation timed out"),
            MqttError::Disconnected => write!(f, "MQTT client disconnected"),
            MqttError::InvalidResponse(msg) => write!(f, "Invalid MQTT response: {}", msg),
        }
    }
}

impl std::error::Error for MqttError {}

/// A message received from MQTT
#[derive(Debug, Clone)]
pub struct MqttMessage {
    pub topic: String,
    pub payload: Vec<u8>,
}

impl MqttMessage {
    /// Try to parse the payload as JSON
    pub fn parse_json<T: for<'de> Deserialize<'de>>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_slice(&self.payload)
    }

    /// Get payload as string
    pub fn payload_str(&self) -> Option<String> {
        String::from_utf8(self.payload.clone()).ok()
    }
}

/// Pending request waiting for a response
struct PendingRequest {
    response_tx: oneshot::Sender<MqttMessage>,
    topic_filter: String,
}

/// A managed MQTT connection with request-response support
pub struct MqttConnection {
    client: AsyncClient,
    pending_requests: Arc<Mutex<Vec<PendingRequest>>>,
    connected: Arc<Mutex<bool>>,
    _event_handle: tokio::task::JoinHandle<()>,
}

impl MqttConnection {
    /// Create a new MQTT connection
    pub async fn connect(config: MqttConfig) -> Result<Self, MqttError> {
        // Build the appropriate URL based on transport configuration
        let mut mqtt_options = if config.use_websocket {
            // Use WebSocket transport (wss:// for TLS, ws:// for plain)
            // First create MqttOptions with standard constructor, then set WebSocket transport
            let scheme = if config.use_tls { "wss" } else { "ws" };
            let url = format!(
                "{}://{}:{}",
                scheme, config.broker_host, config.broker_port
            );
            info!("Setting up WebSocket MQTT connection to: {}", url);

            // Create options with client_id, then parse URL just for transport config
            let mut opts = MqttOptions::new(&config.client_id, &config.broker_host, config.broker_port);

            // Set WebSocket transport with TLS
            if config.use_tls {
                // Load native root certificates
                let mut root_cert_store = tokio_rustls::rustls::RootCertStore::empty();
                let cert_result = rustls_native_certs::load_native_certs();
                for err in &cert_result.errors {
                    warn!("Error loading native cert: {}", err);
                }
                let (added, _ignored) = root_cert_store.add_parsable_certificates(cert_result.certs);
                info!("Loaded {} native root certificates for WebSocket TLS", added);

                let client_config = ClientConfig::builder()
                    .with_root_certificates(root_cert_store)
                    .with_no_client_auth();

                opts.set_transport(Transport::wss_with_config(client_config.into()));
            } else {
                opts.set_transport(Transport::Ws);
            }

            opts
        } else {
            // Use standard MQTT TCP transport
            MqttOptions::new(&config.client_id, &config.broker_host, config.broker_port)
        };

        mqtt_options.set_keep_alive(Duration::from_secs(config.keep_alive_secs));

        if let (Some(username), Some(password)) = (&config.username, &config.password) {
            mqtt_options.set_credentials(username, password);
        }

        // For non-websocket TLS connections, we need to set up the transport manually
        // For websocket connections (wss://), TLS is handled automatically by parse_url
        if config.use_tls && !config.use_websocket {
            info!(
                "Setting up TLS connection to {}:{}",
                config.broker_host, config.broker_port
            );

            // Load native root certificates from the operating system
            let mut root_cert_store = tokio_rustls::rustls::RootCertStore::empty();

            let cert_result = rustls_native_certs::load_native_certs();

            // Log any errors encountered while loading certs
            for err in &cert_result.errors {
                warn!("Error loading native cert: {}", err);
            }

            let (added, _ignored) = root_cert_store.add_parsable_certificates(cert_result.certs);
            info!("Loaded {} native root certificates for TLS", added);

            if added == 0 {
                error!("No TLS certificates loaded! This will likely cause connection failures.");
            }

            let client_config = ClientConfig::builder()
                .with_root_certificates(root_cert_store)
                .with_no_client_auth();

            mqtt_options.set_transport(Transport::tls_with_config(client_config.into()));
        }

        let (client, eventloop) = AsyncClient::new(mqtt_options, 100);
        let pending_requests: Arc<Mutex<Vec<PendingRequest>>> = Arc::new(Mutex::new(Vec::new()));
        let connected: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));

        // Create channel to signal when connection is established
        let (conn_tx, mut conn_rx) = broadcast::channel::<Result<(), String>>(1);

        // Spawn event loop handler
        let pending_clone = pending_requests.clone();
        let connected_clone = connected.clone();
        let event_handle = tokio::spawn(async move {
            Self::run_event_loop(eventloop, pending_clone, connected_clone, Some(conn_tx)).await;
        });

        // Wait for connection to be established (with timeout)
        let broker_host = config.broker_host.clone();
        let broker_port = config.broker_port;
        match timeout(Duration::from_secs(10), conn_rx.recv()).await {
            Ok(Ok(Ok(()))) => {
                info!("MQTT connected to {}:{}", broker_host, broker_port);
            }
            Ok(Ok(Err(e))) => {
                return Err(MqttError::ConnectionFailed(e));
            }
            Ok(Err(_)) => {
                return Err(MqttError::ConnectionFailed(
                    "Connection channel closed unexpectedly".to_string(),
                ));
            }
            Err(_) => {
                return Err(MqttError::ConnectionFailed(
                    "Connection timeout - no ConnAck received".to_string(),
                ));
            }
        }

        Ok(Self {
            client,
            pending_requests,
            connected,
            _event_handle: event_handle,
        })
    }

    /// Run the MQTT event loop
    async fn run_event_loop(
        mut eventloop: EventLoop,
        pending_requests: Arc<Mutex<Vec<PendingRequest>>>,
        connected: Arc<Mutex<bool>>,
        conn_signal: Option<broadcast::Sender<Result<(), String>>>,
    ) {
        let mut conn_signal = conn_signal;

        loop {
            match eventloop.poll().await {
                Ok(Event::Incoming(Packet::Publish(publish))) => {
                    let topic = publish.topic.clone();
                    let payload = publish.payload.to_vec();

                    debug!("MQTT received on {}: {} bytes", topic, payload.len());

                    // Check if any pending request matches
                    let mut pending = pending_requests.lock().await;
                    let mut matched_idx = None;

                    for (idx, req) in pending.iter().enumerate() {
                        if topic.contains(&req.topic_filter) || req.topic_filter == "*" {
                            matched_idx = Some(idx);
                            break;
                        }
                    }

                    if let Some(idx) = matched_idx {
                        let req = pending.remove(idx);
                        let msg = MqttMessage { topic, payload };
                        let _ = req.response_tx.send(msg);
                    }
                }
                Ok(Event::Incoming(Packet::ConnAck(connack))) => {
                    info!("MQTT connection acknowledged: {:?}", connack.code);
                    if connack.code == rumqttc::ConnectReturnCode::Success {
                        *connected.lock().await = true;
                        if let Some(tx) = conn_signal.take() {
                            let _ = tx.send(Ok(()));
                        }
                    } else {
                        if let Some(tx) = conn_signal.take() {
                            let _ = tx.send(Err(format!("Connection rejected: {:?}", connack.code)));
                        }
                        break;
                    }
                }
                Ok(Event::Incoming(Packet::SubAck(_))) => {
                    debug!("MQTT subscription acknowledged");
                }
                Ok(_) => {}
                Err(e) => {
                    error!("MQTT event loop error: {:?}", e);
                    error!("MQTT error details: {}", e);
                    *connected.lock().await = false;
                    // Signal connection failure if we haven't signaled yet
                    if let Some(tx) = conn_signal.take() {
                        let _ = tx.send(Err(format!("{:?}", e)));
                    }
                    // Clear all pending requests on error
                    pending_requests.lock().await.clear();
                    break;
                }
            }
        }
    }

    /// Subscribe to a topic
    pub async fn subscribe(&self, topic: &str) -> Result<(), MqttError> {
        self.client
            .subscribe(topic, QoS::AtLeastOnce)
            .await
            .map_err(|e| MqttError::SubscribeFailed(e.to_string()))
    }

    /// Publish a message
    pub async fn publish(&self, topic: &str, payload: &[u8]) -> Result<(), MqttError> {
        self.client
            .publish(topic, QoS::AtLeastOnce, false, payload)
            .await
            .map_err(|e| MqttError::PublishFailed(e.to_string()))
    }

    /// Publish JSON payload
    pub async fn publish_json<T: Serialize>(&self, topic: &str, payload: &T) -> Result<(), MqttError> {
        let json = serde_json::to_vec(payload)
            .map_err(|e| MqttError::PublishFailed(e.to_string()))?;
        self.publish(topic, &json).await
    }

    /// Check if the connection is still active
    pub async fn is_connected(&self) -> bool {
        *self.connected.lock().await
    }

    /// Send a request and wait for response
    pub async fn request(
        &self,
        publish_topic: &str,
        payload: &[u8],
        response_topic_filter: &str,
        timeout_secs: u64,
    ) -> Result<MqttMessage, MqttError> {
        // Check if still connected
        if !self.is_connected().await {
            return Err(MqttError::Disconnected);
        }

        // Create response channel
        let (tx, rx) = oneshot::channel();

        // Register pending request
        {
            let mut pending = self.pending_requests.lock().await;
            pending.push(PendingRequest {
                response_tx: tx,
                topic_filter: response_topic_filter.to_string(),
            });
        }

        // Publish the request
        self.publish(publish_topic, payload).await?;

        // Wait for response with timeout
        match timeout(Duration::from_secs(timeout_secs), rx).await {
            Ok(Ok(msg)) => Ok(msg),
            Ok(Err(_)) => Err(MqttError::Disconnected),
            Err(_) => {
                // Remove pending request on timeout
                let mut pending = self.pending_requests.lock().await;
                pending.retain(|r| r.topic_filter != response_topic_filter);
                Err(MqttError::Timeout)
            }
        }
    }

    /// Send a JSON request and wait for JSON response
    pub async fn request_json<Req: Serialize, Resp: for<'de> Deserialize<'de>>(
        &self,
        publish_topic: &str,
        payload: &Req,
        response_topic_filter: &str,
        timeout_secs: u64,
    ) -> Result<Resp, MqttError> {
        let json = serde_json::to_vec(payload)
            .map_err(|e| MqttError::PublishFailed(e.to_string()))?;

        let response = self.request(publish_topic, &json, response_topic_filter, timeout_secs).await?;

        response.parse_json()
            .map_err(|e| MqttError::InvalidResponse(e.to_string()))
    }

    /// Disconnect the client
    pub async fn disconnect(&self) -> Result<(), MqttError> {
        self.client
            .disconnect()
            .await
            .map_err(|e| MqttError::ConnectionFailed(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mqtt_config_default() {
        let config = MqttConfig::default();
        assert_eq!(config.broker_host, "localhost");
        assert_eq!(config.broker_port, 1883);
        assert!(!config.use_tls);
        assert!(config.client_id.starts_with("pvpc-cheap-"));
    }

    #[test]
    fn test_mqtt_error_display() {
        let err = MqttError::Timeout;
        assert!(err.to_string().contains("timed out"));

        let err = MqttError::ConnectionFailed("test".to_string());
        assert!(err.to_string().contains("connection failed"));
    }

    #[test]
    fn test_mqtt_message_payload_str() {
        let msg = MqttMessage {
            topic: "test/topic".to_string(),
            payload: b"hello world".to_vec(),
        };
        assert_eq!(msg.payload_str(), Some("hello world".to_string()));
    }

    #[test]
    fn test_mqtt_message_parse_json() {
        #[derive(Deserialize, PartialEq, Debug)]
        struct TestPayload {
            value: i32,
        }

        let msg = MqttMessage {
            topic: "test".to_string(),
            payload: br#"{"value": 42}"#.to_vec(),
        };

        let parsed: TestPayload = msg.parse_json().unwrap();
        assert_eq!(parsed.value, 42);
    }

    #[test]
    fn test_mqtt_message_parse_json_invalid() {
        #[derive(Deserialize)]
        struct TestPayload {
            value: i32,
        }

        let msg = MqttMessage {
            topic: "test".to_string(),
            payload: b"not json".to_vec(),
        };

        let result: Result<TestPayload, _> = msg.parse_json();
        assert!(result.is_err());
    }
}
