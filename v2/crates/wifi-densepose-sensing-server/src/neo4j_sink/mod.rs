//! Neo4j sink — logs sensing events to a Neo4j graph database via Bolt.
//!
//! Follows the same broadcast-subscriber pattern as the MQTT publisher
//! (`mqtt/publisher.rs`). Subscribes to the main `broadcast::Sender<String>`
//! channel, deserializes each `SensingUpdate`, and writes to Neo4j.
//!
//! ## Schema
//!
//! ```cypher
//! (:SensingEvent {timestamp, source, tick, motion_level, person_count, presence, signal_quality})
//!   -[:HAS_VITALS]-> (:VitalSigns {heart_rate, breathing_rate, hr_confidence, br_confidence, signal_quality})
//!   -[:DETECTED {index}]-> (:Person {id, position_x, position_y, keypoints_json})
//!   -[:FROM_NODE]-> (:SensorNode {id, rssi_dbm, position_x, position_y})
//!
//! (:Room {name}) -[:CURRENT_EVENT]-> (:SensingEvent)
//! ```
//!
//! ## Lifecycle
//!
//! 1. Connect to Neo4j via Bolt (`neo4rs::Graph`).
//! 2. Ensure room node exists.
//! 3. Subscribe to broadcast channel.
//! 4. Per inbound message: deserialize, write nodes/relationships, update Room pointer.
//! 5. On channel close: log and exit.

use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

pub use crate::cli::Neo4jArgs;

/// Configuration for the Neo4j sink.
#[derive(Debug, Clone)]
pub struct Neo4jConfig {
    /// Bolt URL (e.g. `bolt://x1-370:7687`).
    pub url: String,
    /// Neo4j username.
    pub user: String,
    /// Neo4j password (resolved from env var at startup).
    pub password: String,
    /// Room identifier for the `:Room` node.
    pub room_name: String,
}

impl Neo4jConfig {
    /// Build config from CLI args + environment.
    pub fn from_args(args: &Neo4jArgs) -> Result<Self, String> {
        let password = std::env::var(&args.neo4j_password_env)
            .map_err(|_| format!(
                "Neo4j password env var `{}` not set",
                args.neo4j_password_env
            ))?;

        Ok(Self {
            url: args.neo4j_url.clone(),
            user: args.neo4j_user.clone(),
            password,
            room_name: args.neo4j_room_name.clone(),
        })
    }
}

/// Spawn the Neo4j sink background task.
pub fn spawn(
    cfg: Neo4jConfig,
    mut state_rx: broadcast::Receiver<String>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        if let Err(e) = run(cfg, &mut state_rx).await {
            error!("[neo4j] sink terminated: {e}");
        }
    })
}

/// Core run loop. Receives `SensingUpdate` JSON from the broadcast channel
/// and writes to Neo4j.
async fn run(cfg: Neo4jConfig, state_rx: &mut broadcast::Receiver<String>) -> Result<(), String> {
    use neo4rs::{query, Graph};

    info!(
        url = %cfg.url,
        user = %cfg.user,
        room = %cfg.room_name,
        "[neo4j] connecting",
    );

    let graph = Graph::new(&cfg.url, &cfg.user, &cfg.password)
        .map_err(|e| format!("Neo4j connection failed: {e}"))?;

    info!("[neo4j] connected");

    // Ensure room node exists.
    graph
        .run(query("MERGE (r:Room {name: $name}) SET r.last_updated = datetime()")
            .param("name", cfg.room_name.clone()))
        .await
        .map_err(|e| format!("Schema setup failed: {e}"))?;

    info!(room = %cfg.room_name, "[neo4j] schema ready");

    let mut event_count: u64 = 0;

    loop {
        match state_rx.recv().await {
            Ok(json) => {
                let v: serde_json::Value = match serde_json::from_str(&json) {
                    Ok(v) => v,
                    Err(e) => {
                        warn!("[neo4j] failed to parse SensingUpdate JSON: {e}");
                        continue;
                    }
                };

                if let Err(e) = write_event(&graph, &cfg.room_name, &v).await {
                    warn!("[neo4j] write failed: {e}");
                    continue;
                }

                event_count += 1;
                if event_count.is_multiple_of(50) {
                    info!(event_count, "[neo4j] events written");
                }
                if event_count <= 10 || event_count.is_multiple_of(100) {
                    info!(payload = %v, "[neo4j] event payload");
                }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                warn!("[neo4j] lagged behind broadcast by {n} messages — dropped");
            }
            Err(broadcast::error::RecvError::Closed) => {
                info!("[neo4j] broadcast channel closed, exiting");
                return Ok(());
            }
        }
    }
}

/// Write a single SensingUpdate (as JSON Value) as Neo4j nodes + relationships.
async fn write_event(
    graph: &neo4rs::Graph,
    room_name: &str,
    v: &serde_json::Value,
) -> Result<(), String> {
    use neo4rs::query;

    let timestamp = v["timestamp"].as_f64().unwrap_or(0.0);
    let ts_str = chrono::DateTime::from_timestamp_millis((timestamp * 1000.0) as i64)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| timestamp.to_string());

    let source = v["source"].as_str().unwrap_or("unknown");
    let tick = v["tick"].as_i64().unwrap_or(0);
    let motion_level = v["classification"]["motion_level"]
        .as_str()
        .unwrap_or("absent");
    let presence = v["classification"]["presence"].as_bool().unwrap_or(false);
    let person_count = v["estimated_persons"].as_i64().unwrap_or(0);
    let signal_quality = v["signal_quality_score"].as_f64().unwrap_or(0.0);

    // 1. Create the SensingEvent node.
    graph
        .run(query(
            "CREATE (e:SensingEvent {
                timestamp: $timestamp,
                source: $source,
                tick: $tick,
                motion_level: $motion_level,
                person_count: $person_count,
                presence: $presence,
                signal_quality: $signal_quality,
                created_at: datetime()
            })",
        )
        .param("timestamp", ts_str.clone())
        .param("source", source)
        .param("tick", tick)
        .param("motion_level", motion_level)
        .param("person_count", person_count)
        .param("presence", presence)
        .param("signal_quality", signal_quality))
        .await
        .map_err(|e| format!("Event creation failed: {e}"))?;

    // 2. Create VitalSigns node and link to event.
    if let Some(vs) = v.get("vital_signs") {
        let hr = vs["heart_rate_bpm"].as_f64().unwrap_or(0.0);
        let br = vs["breathing_rate_bpm"].as_f64().unwrap_or(0.0);
        let hr_conf = vs["heartbeat_confidence"].as_f64().unwrap_or(0.0);
        let br_conf = vs["breathing_confidence"].as_f64().unwrap_or(0.0);
        let vs_quality = vs["signal_quality"].as_f64().unwrap_or(0.0);

        graph
            .run(query(
                "MATCH (e:SensingEvent {timestamp: $timestamp})
                 CREATE (v:VitalSigns {
                    heart_rate: $hr,
                    breathing_rate: $br,
                    hr_confidence: $hr_conf,
                    br_confidence: $br_conf,
                    signal_quality: $vs_quality
                 })
                 CREATE (e)-[:HAS_VITALS]->(v)",
            )
            .param("timestamp", ts_str.clone())
            .param("hr", hr)
            .param("br", br)
            .param("hr_conf", hr_conf)
            .param("br_conf", br_conf)
            .param("vs_quality", vs_quality))
            .await
            .map_err(|e| format!("VitalSigns creation failed: {e}"))?;
    }

    // 3. Create Person nodes for each detected person.
    if let Some(persons) = v["persons"].as_array() {
        for (i, person) in persons.iter().enumerate() {
            let id = person["id"].as_i64().unwrap_or(i as i64);
            let default_pos = vec![0.0.into(), 0.0.into(), 0.0.into()];
            let pos = person["position"]
                .as_array()
                .unwrap_or(&default_pos);
            let px = pos.first().and_then(|v| v.as_f64()).unwrap_or(0.0);
            let py = pos.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0);
            let conf = person["confidence"].as_f64().unwrap_or(0.0);
            let keypoints_json = person["keypoints"].to_string();

            graph
                .run(query(
                    "MATCH (e:SensingEvent {timestamp: $timestamp})
                     CREATE (p:Person {
                        id: $id,
                        position_x: $px,
                        position_y: $py,
                        keypoints_json: $kp,
                        confidence: $conf
                     })
                     CREATE (e)-[:DETECTED {index: $idx}]->(p)",
                )
                .param("timestamp", ts_str.clone())
                .param("id", id)
                .param("px", px)
                .param("py", py)
                .param("kp", keypoints_json.as_str())
                .param("conf", conf)
                .param("idx", i as i64))
                .await
                .map_err(|e| format!("Person creation failed: {e}"))?;
        }
    }

    // 4. Link SensorNode entries.
    if let Some(nodes) = v["nodes"].as_array() {
        for node in nodes {
            let node_id = node["node_id"].as_i64().unwrap_or(0);
            let rssi = node["rssi_dbm"].as_f64().unwrap_or(0.0);
            let default_pos = vec![0.0.into(), 0.0.into(), 0.0.into()];
            let pos = node["position"]
                .as_array()
                .unwrap_or(&default_pos);
            let px = pos.first().and_then(|v| v.as_f64()).unwrap_or(0.0);
            let py = pos.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0);

            graph
                .run(query(
                    "MATCH (e:SensingEvent {timestamp: $timestamp})
                     MERGE (n:SensorNode {id: $id})
                     SET n.rssi_dbm = $rssi,
                         n.position_x = $px,
                         n.position_y = $py
                     CREATE (e)-[:FROM_NODE]->(n)",
                )
                .param("timestamp", ts_str.clone())
                .param("id", node_id)
                .param("rssi", rssi)
                .param("px", px)
                .param("py", py))
                .await
                .map_err(|e| format!("SensorNode creation failed: {e}"))?;
        }
    }

    // 5. Update the Room's CURRENT_EVENT pointer (delete old, link newest).
    graph
        .run(query(
            "MATCH (r:Room {name: $room})-[old:CURRENT_EVENT]->(:SensingEvent)
             DELETE old",
        )
        .param("room", room_name.to_string()))
        .await
        .map_err(|e| format!("Room cleanup failed: {e}"))?;

    graph
        .run(query(
            "MATCH (r:Room {name: $room}), (e:SensingEvent {timestamp: $timestamp})
             CREATE (r)-[:CURRENT_EVENT]->(e)
             SET r.last_updated = datetime()",
        )
        .param("room", room_name.to_string())
        .param("timestamp", ts_str.clone()))
        .await
        .map_err(|e| format!("Room update failed: {e}"))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neo4j_config_from_args() {
        let args = crate::cli::Neo4jArgs {
            neo4j: true,
            neo4j_url: "bolt://x1-370:7687".into(),
            neo4j_user: "neo4j".into(),
            neo4j_password_env: "NEO4J_PASSWORD".into(),
            neo4j_room_name: "main".into(),
        };
        std::env::set_var("NEO4J_PASSWORD", "testpass");
        let cfg = Neo4jConfig::from_args(&args).unwrap();
        assert_eq!(cfg.url, "bolt://x1-370:7687");
        assert_eq!(cfg.password, "testpass");
        assert_eq!(cfg.room_name, "main");
    }
}
