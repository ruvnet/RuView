//! Neo4j sensing sink.
//!
//! Every event is durably owned by one Room through `HAS_EVENT`; the room's
//! `CURRENT_EVENT` pointer is only a convenience pointer. Event creation,
//! subordinate nodes, ownership, and pointer replacement commit atomically.

use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

pub use crate::cli::Neo4jArgs;

#[derive(Debug, Clone)]
pub struct Neo4jConfig {
    pub url: String,
    pub user: String,
    pub password: String,
    pub room_name: String,
    pub ttl_hours: u64,
}

impl Neo4jConfig {
    pub fn from_args(args: &Neo4jArgs) -> Result<Self, String> {
        let password = std::env::var(&args.neo4j_password_env)
            .map_err(|_| format!("Neo4j password env var `{}` not set", args.neo4j_password_env))?;
        if args.neo4j_room_name.trim().is_empty() {
            return Err("Neo4j room name must not be empty".into());
        }
        Ok(Self {
            url: args.neo4j_url.clone(),
            user: args.neo4j_user.clone(),
            password,
            room_name: args.neo4j_room_name.clone(),
            ttl_hours: args.neo4j_ttl_hours,
        })
    }
}

pub fn spawn(cfg: Neo4jConfig, mut state_rx: broadcast::Receiver<String>) -> JoinHandle<()> {
    tokio::spawn(async move {
        if let Err(e) = run(cfg, &mut state_rx).await {
            error!("[neo4j] sink terminated: {e}");
        }
    })
}

async fn run(cfg: Neo4jConfig, state_rx: &mut broadcast::Receiver<String>) -> Result<(), String> {
    use neo4rs::{query, Graph};
    let graph = Graph::new(&cfg.url, &cfg.user, &cfg.password)
        .map_err(|e| format!("Neo4j connection failed: {e}"))?;

    graph.run(query("MERGE (r:Room {name:$name}) SET r.last_updated=datetime()")
        .param("name", cfg.room_name.clone())).await
        .map_err(|e| format!("Room setup failed: {e}"))?;
    graph.run(query("CREATE INDEX event_timestamp IF NOT EXISTS FOR (e:SensingEvent) ON (e.timestamp)"))
        .await.map_err(|e| format!("Index creation failed: {e}"))?;
    graph.run(query("CREATE INDEX event_room IF NOT EXISTS FOR (e:SensingEvent) ON (e.room_name)"))
        .await.map_err(|e| format!("Index creation failed: {e}"))?;

    if cfg.ttl_hours > 0 {
        let (url, user, pass, room, ttl) = (
            cfg.url.clone(), cfg.user.clone(), cfg.password.clone(), cfg.room_name.clone(), cfg.ttl_hours,
        );
        tokio::spawn(async move { compaction_loop(&url, &user, &pass, &room, ttl).await; });
    }

    let mut event_count = 0u64;
    loop {
        match state_rx.recv().await {
            Ok(raw) => {
                let value: serde_json::Value = match serde_json::from_str(&raw) {
                    Ok(v) => v,
                    Err(e) => { warn!("[neo4j] invalid update: {e}"); continue; }
                };
                if let Err(e) = write_event(&graph, &cfg.room_name, &value).await {
                    warn!("[neo4j] transactional write failed: {e}");
                    continue;
                }
                event_count += 1;
                if event_count.is_multiple_of(50) { info!(event_count, "[neo4j] events written"); }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => warn!("[neo4j] dropped {n} lagged updates"),
            Err(broadcast::error::RecvError::Closed) => return Ok(()),
        }
    }
}

async fn write_event(graph: &neo4rs::Graph, room: &str, v: &serde_json::Value) -> Result<(), String> {
    use neo4rs::query;

    let timestamp = v["timestamp"].as_f64().unwrap_or(0.0);
    let ts = chrono::DateTime::from_timestamp_millis((timestamp * 1000.0) as i64)
        .map(|dt| dt.to_rfc3339()).unwrap_or_else(|| timestamp.to_string());
    let source = v["source"].as_str().unwrap_or("unknown").to_string();
    let tick = v["tick"].as_i64().unwrap_or(0);
    let motion = v["classification"]["motion_level"].as_str().unwrap_or("absent").to_string();
    let presence = v["classification"]["presence"].as_bool().unwrap_or(false);
    let persons = v["estimated_persons"].as_i64().unwrap_or(0);
    let quality = v["signal_quality_score"].as_f64().unwrap_or(0.0);

    let mut queries = Vec::new();
    queries.push(query(
        "MATCH (r:Room {name:$room})
         CREATE (e:SensingEvent {room_name:$room,timestamp:$ts,source:$source,tick:$tick,motion_level:$motion,person_count:$persons,presence:$presence,signal_quality:$quality,created_at:datetime()})
         CREATE (r)-[:HAS_EVENT]->(e)"
    ).param("room", room.to_string()).param("ts", ts.clone()).param("source", source)
      .param("tick", tick).param("motion", motion).param("persons", persons)
      .param("presence", presence).param("quality", quality));

    if let Some(vs) = v.get("vital_signs") {
        queries.push(query(
            "MATCH (e:SensingEvent {room_name:$room,timestamp:$ts,tick:$tick})
             CREATE (x:VitalSigns {heart_rate:$hr,breathing_rate:$br,hr_confidence:$hc,br_confidence:$bc,signal_quality:$q})
             CREATE (e)-[:HAS_VITALS]->(x)"
        ).param("room", room.to_string()).param("ts", ts.clone()).param("tick", tick)
          .param("hr", vs["heart_rate_bpm"].as_f64().unwrap_or(0.0))
          .param("br", vs["breathing_rate_bpm"].as_f64().unwrap_or(0.0))
          .param("hc", vs["heartbeat_confidence"].as_f64().unwrap_or(0.0))
          .param("bc", vs["breathing_confidence"].as_f64().unwrap_or(0.0))
          .param("q", vs["signal_quality"].as_f64().unwrap_or(0.0)));
    }

    if let Some(items) = v["persons"].as_array() {
        for (i, p) in items.iter().enumerate() {
            let pos = p["position"].as_array();
            let px = pos.and_then(|x| x.first()).and_then(|x| x.as_f64()).unwrap_or(0.0);
            let py = pos.and_then(|x| x.get(1)).and_then(|x| x.as_f64()).unwrap_or(0.0);
            queries.push(query(
                "MATCH (e:SensingEvent {room_name:$room,timestamp:$ts,tick:$tick})
                 CREATE (p:Person {id:$id,position_x:$px,position_y:$py,keypoints_json:$kp,confidence:$conf})
                 CREATE (e)-[:DETECTED {index:$idx}]->(p)"
            ).param("room", room.to_string()).param("ts", ts.clone()).param("tick", tick)
              .param("id", p["id"].as_i64().unwrap_or(i as i64)).param("px", px).param("py", py)
              .param("kp", p["keypoints"].to_string()).param("conf", p["confidence"].as_f64().unwrap_or(0.0))
              .param("idx", i as i64));
        }
    }

    if let Some(nodes) = v["nodes"].as_array() {
        for n in nodes {
            let pos = n["position"].as_array();
            let px = pos.and_then(|x| x.first()).and_then(|x| x.as_f64()).unwrap_or(0.0);
            let py = pos.and_then(|x| x.get(1)).and_then(|x| x.as_f64()).unwrap_or(0.0);
            queries.push(query(
                "MATCH (e:SensingEvent {room_name:$room,timestamp:$ts,tick:$tick})
                 MERGE (n:SensorNode {id:$id}) SET n.rssi_dbm=$rssi,n.position_x=$px,n.position_y=$py
                 CREATE (e)-[:FROM_NODE]->(n)"
            ).param("room", room.to_string()).param("ts", ts.clone()).param("tick", tick)
              .param("id", n["node_id"].as_i64().unwrap_or(0)).param("rssi", n["rssi_dbm"].as_f64().unwrap_or(0.0))
              .param("px", px).param("py", py));
        }
    }

    queries.push(query(
        "MATCH (r:Room {name:$room}), (e:SensingEvent {room_name:$room,timestamp:$ts,tick:$tick})
         OPTIONAL MATCH (r)-[old:CURRENT_EVENT]->(:SensingEvent) DELETE old
         CREATE (r)-[:CURRENT_EVENT]->(e) SET r.last_updated=datetime()"
    ).param("room", room.to_string()).param("ts", ts).param("tick", tick));

    let mut txn = graph.start_txn().await.map_err(|e| format!("transaction start failed: {e}"))?;
    txn.run_queries(queries).await.map_err(|e| format!("transaction body failed: {e}"))?;
    txn.commit().await.map_err(|e| format!("transaction commit failed: {e}"))?;
    Ok(())
}

async fn compaction_loop(url: &str, user: &str, password: &str, room: &str, ttl_hours: u64) {
    use neo4rs::{query, Graph};
    let graph = match Graph::new(url, user, password) { Ok(g) => g, Err(e) => { warn!("[neo4j-compaction] connection failed: {e}"); return; } };
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(600));
    loop {
        interval.tick().await;
        let cutoff = (chrono::Utc::now() - chrono::Duration::hours(ttl_hours as i64)).to_rfc3339();
        let mut txn = match graph.start_txn().await { Ok(t) => t, Err(e) => { warn!("[neo4j-compaction] txn start failed: {e}"); continue; } };
        let queries = vec![
            query("MATCH (:Room {name:$room})-[:HAS_EVENT]->(e:SensingEvent)-[:DETECTED]->(p:Person) WHERE datetime(e.timestamp)<datetime($cutoff) DETACH DELETE p")
                .param("room", room.to_string()).param("cutoff", cutoff.clone()),
            query("MATCH (:Room {name:$room})-[:HAS_EVENT]->(e:SensingEvent)-[:HAS_VITALS]->(v:VitalSigns) WHERE datetime(e.timestamp)<datetime($cutoff) DETACH DELETE v")
                .param("room", room.to_string()).param("cutoff", cutoff.clone()),
            query("MATCH (r:Room {name:$room})-[:HAS_EVENT]->(e:SensingEvent) WHERE datetime(e.timestamp)<datetime($cutoff) DETACH DELETE e")
                .param("room", room.to_string()).param("cutoff", cutoff.clone()),
            query("MATCH (r:Room {name:$room}) OPTIONAL MATCH (r)-[old:CURRENT_EVENT]->() DELETE old WITH r OPTIONAL MATCH (r)-[:HAS_EVENT]->(e:SensingEvent) WITH r,e ORDER BY e.timestamp DESC WITH r,head(collect(e)) AS newest FOREACH (_ IN CASE WHEN newest IS NULL THEN [] ELSE [1] END | CREATE (r)-[:CURRENT_EVENT]->(newest))")
                .param("room", room.to_string()),
        ];
        if let Err(e) = txn.run_queries(queries).await { warn!("[neo4j-compaction] failed: {e}"); continue; }
        if let Err(e) = txn.commit().await { warn!("[neo4j-compaction] commit failed: {e}"); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_room_scope() {
        let args = Neo4jArgs {
            neo4j: true,
            neo4j_url: "bolt://localhost:7687".into(),
            neo4j_user: "neo4j".into(),
            neo4j_password_env: "RUVIEW_TEST_MISSING_PASSWORD".into(),
            neo4j_room_name: " ".into(),
            neo4j_ttl_hours: 1,
        };
        assert!(Neo4jConfig::from_args(&args).is_err());
    }
}
