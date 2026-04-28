//! MQTT bridge for Home Assistant integration.
//!
//! Subscribes to the event bus and publishes events to MQTT.
//! Topics:
//!   ruview/events/{event_type} -- event JSON
//!   homeassistant/sensor/ruview_*/config -- HA MQTT discovery
//!
//! Opt-in via `--mqtt-url` (or `SENSING_MQTT_URL` env var).
//! If no broker is available, logs a warning and retries automatically.

use std::sync::Arc;
use std::time::Duration;

use rumqttc::{AsyncClient, MqttOptions, QoS};
use tracing::{info, warn, debug};

use crate::event_stream::{EventBus, EventData, Event};

/// MQTT bridge configuration.
#[derive(Debug, Clone)]
pub struct MqttConfig {
    pub broker_url: String,
    pub client_id: String,
    pub topic_prefix: String,
    pub ha_discovery: bool,
}

impl Default for MqttConfig {
    fn default() -> Self {
        Self {
            broker_url: "mqtt://localhost:1883".to_string(),
            client_id: "ruview-sensing".to_string(),
            topic_prefix: "ruview".to_string(),
            ha_discovery: true,
        }
    }
}

/// Parse a `mqtt://host:port` URL into `(host, port)`.
fn parse_mqtt_url(url: &str) -> (String, u16) {
    let stripped = url.strip_prefix("mqtt://").unwrap_or(url);
    let parts: Vec<&str> = stripped.split(':').collect();
    let host = parts.first().unwrap_or(&"localhost").to_string();
    let port = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(1883);
    (host, port)
}

/// Start the MQTT bridge as a background task.
///
/// Returns immediately. If the broker is unreachable the bridge logs a
/// warning and retries every 30 s — it never crashes the server.
pub fn start_mqtt_bridge(
    config: MqttConfig,
    event_bus: Arc<EventBus>,
) {
    tokio::spawn(async move {
        let (host, port) = parse_mqtt_url(&config.broker_url);

        let mut mqttoptions = MqttOptions::new(&config.client_id, &host, port);
        mqttoptions.set_keep_alive(Duration::from_secs(30));
        mqttoptions.set_clean_session(true);

        let (client, mut eventloop) = AsyncClient::new(mqttoptions, 100);

        info!("MQTT bridge connecting to {}:{}", host, port);

        // Subscribe to event bus
        let mut rx = event_bus.subscribe();
        let prefix = config.topic_prefix.clone();

        // Publish HA discovery configs if enabled
        if config.ha_discovery {
            let discovery_client = client.clone();
            let discovery_prefix = prefix.clone();
            tokio::spawn(async move {
                // Wait for connection before publishing discovery
                tokio::time::sleep(Duration::from_secs(5)).await;
                publish_ha_discovery(&discovery_client, &discovery_prefix).await;
            });
        }

        // Event forwarding loop
        let event_client = client.clone();
        let event_prefix = prefix.clone();
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        publish_event(&event_client, &event_prefix, &event).await;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        debug!("MQTT bridge lagged by {n} events");
                        continue;
                    }
                    Err(_) => {
                        warn!("MQTT bridge: event bus closed");
                        break;
                    }
                }
            }
        });

        // Drive the MQTT event loop with auto-reconnect
        loop {
            match eventloop.poll().await {
                Ok(_notification) => {}
                Err(e) => {
                    warn!("MQTT connection error: {e} — retrying in 30s");
                    tokio::time::sleep(Duration::from_secs(30)).await;
                }
            }
        }
    });
}

/// Publish a single event to MQTT.
async fn publish_event(client: &AsyncClient, prefix: &str, event: &Event) {
    let event_type = match &event.data {
        EventData::PresenceChanged { .. } => "presence_changed",
        EventData::PostureChanged { .. } => "posture_changed",
        EventData::ActivityChanged { .. } => "activity_changed",
        EventData::VitalsAlert { .. } => "vitals_alert",
        EventData::LyingStillAlert { .. } => "lying_still_alert",
        EventData::NodeOnline { .. } => "node_online",
        EventData::NodeOffline { .. } => "node_offline",
    };

    let topic = format!("{prefix}/events/{event_type}");
    if let Ok(payload) = serde_json::to_string(event) {
        if let Err(e) = client.publish(&topic, QoS::AtMostOnce, false, payload).await {
            debug!("MQTT publish failed: {e}");
        }
    }
}

/// Publish Home Assistant MQTT discovery messages.
async fn publish_ha_discovery(client: &AsyncClient, prefix: &str) {
    // Person count sensor
    let config = serde_json::json!({
        "name": "RuView Person Count",
        "unique_id": "ruview_person_count",
        "state_topic": format!("{prefix}/events/presence_changed"),
        "value_template": "{{ value_json.data.body_count }}",
        "device": {
            "identifiers": ["ruview_sensing"],
            "name": "RuView Sensing",
            "manufacturer": "RuView",
            "model": "WiFi CSI Sensing Server"
        },
        "icon": "mdi:account-group"
    });

    let topic = "homeassistant/sensor/ruview_person_count/config";
    if let Ok(payload) = serde_json::to_string(&config) {
        let _ = client.publish(topic, QoS::AtLeastOnce, true, payload).await;
    }

    // Activity sensor
    let activity_config = serde_json::json!({
        "name": "RuView Activity",
        "unique_id": "ruview_activity",
        "state_topic": format!("{prefix}/events/activity_changed"),
        "value_template": "{{ value_json.data.to }}",
        "device": {
            "identifiers": ["ruview_sensing"],
            "name": "RuView Sensing",
        },
        "icon": "mdi:run"
    });

    let topic = "homeassistant/sensor/ruview_activity/config";
    if let Ok(payload) = serde_json::to_string(&activity_config) {
        let _ = client.publish(topic, QoS::AtLeastOnce, true, payload).await;
    }

    info!("MQTT HA discovery configs published");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_mqtt_url() {
        let (host, port) = parse_mqtt_url("mqtt://test.example.com:1883");
        assert_eq!(host, "test.example.com");
        assert_eq!(port, 1883);
    }

    #[test]
    fn test_parse_mqtt_url_default_port() {
        let (host, port) = parse_mqtt_url("mqtt://broker.local");
        assert_eq!(host, "broker.local");
        assert_eq!(port, 1883);
    }

    #[test]
    fn test_parse_mqtt_url_no_scheme() {
        let (host, port) = parse_mqtt_url("10.0.0.5:1884");
        assert_eq!(host, "10.0.0.5");
        assert_eq!(port, 1884);
    }

    #[test]
    fn test_default_config() {
        let config = MqttConfig::default();
        assert_eq!(config.broker_url, "mqtt://localhost:1883");
        assert_eq!(config.client_id, "ruview-sensing");
        assert_eq!(config.topic_prefix, "ruview");
        assert!(config.ha_discovery);
    }
}
