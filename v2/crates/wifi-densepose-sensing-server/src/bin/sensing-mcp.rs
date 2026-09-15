//! MCP server for RuView sensing data (hermes agent integration).
//!
//! Exposes sensing events, vital signs, person detections, and room status
//! from Neo4j as MCP tools that hermes can call on-demand.
//!
//! Usage:
//! ```bash
//! NEO4J_PASSWORD=<your-password> \
//!   cargo run --bin sensing-mcp --features "neo4j,mcp" --no-default-features
//! ```

use clap::Parser;
use pmcp::{Server, SimpleTool};
use serde_json::{json, Value};
use std::sync::Arc;

/// MCP server CLI args.
#[derive(Parser, Debug)]
#[command(name = "sensing-mcp", about = "RuView Sensing Data MCP Server")]
struct Args {
    /// Neo4j Bolt URL
    #[arg(long, env = "RUVIEW_NEO4J_URL", default_value = "bolt://x1-370:7687")]
    neo4j_url: String,

    /// Neo4j username
    #[arg(long, env = "RUVIEW_NEO4J_USER", default_value = "neo4j")]
    neo4j_user: String,

    /// Environment variable holding the Neo4j password
    #[arg(long, default_value = "NEO4J_PASSWORD")]
    neo4j_password_env: String,

    /// Room name to query (default: all rooms)
    #[arg(long, env = "RUVIEW_NEO4J_ROOM")]
    room: Option<String>,
}

/// Shared state for MCP tools.
struct McpState {
    graph: neo4rs::Graph,
    room: Option<String>,
}

fn to_pmcp_err(e: impl std::fmt::Display) -> pmcp::Error {
    pmcp::Error::internal(format!("{e}"))
}

impl McpState {
    async fn new(args: &Args) -> Result<Arc<Self>, pmcp::Error> {
        let password = std::env::var(&args.neo4j_password_env)
            .map_err(|_| pmcp::Error::internal(format!(
                "Env var `{}` not set", args.neo4j_password_env
            )))?;

        let graph = neo4rs::Graph::new(&args.neo4j_url, &args.neo4j_user, &password)
            .map_err(to_pmcp_err)?;

        Ok(Arc::new(Self {
            graph,
            room: args.room.clone(),
        }))
    }

    async fn query(&self, cypher: &str, params: Vec<(&str, neo4rs::BoltType)>) -> Result<Vec<Value>, pmcp::Error> {
        use neo4rs::query;

        let mut q = query(cypher);
        for (k, v) in params {
            q = q.param(k, v);
        }

        let mut rows = self.graph.execute(q).await.map_err(to_pmcp_err)?;

        let mut results = Vec::new();
        while let Ok(Some(row)) = rows.next().await {
            let keys: Vec<String> = row.keys().iter().map(|k| k.to_string()).collect();
            let mut map = serde_json::Map::new();
            for key in &keys {
                if let Ok(val) = row.get::<neo4rs::BoltType>(key) {
                    map.insert(key.clone(), bolt_to_json(&val));
                }
            }
            results.push(Value::Object(map));
        }
        Ok(results)
    }
}

/// Convert a neo4rs BoltType to serde_json::Value.
fn bolt_to_json(val: &neo4rs::BoltType) -> Value {
    use neo4rs::BoltType;
    match val {
        BoltType::Boolean(b) => Value::Bool(b.value),
        BoltType::Integer(i) => json!(i.value),
        BoltType::Float(f) => json!(f.value),
        BoltType::String(s) => Value::String(s.value.clone()),
        BoltType::Null(_) => Value::Null,
        BoltType::List(list) => {
            let arr: Vec<Value> = list.value.iter().map(bolt_to_json).collect();
            Value::Array(arr)
        }
        BoltType::Map(map) => {
            let m: serde_json::Map<String, Value> = map.value.iter()
                .map(|(k, v)| (k.value.clone(), bolt_to_json(v)))
                .collect();
            Value::Object(m)
        }
        BoltType::Node(node) => {
            let mut m = serde_json::Map::new();
            m.insert("_type".into(), Value::String("Node".into()));
            m.insert("_id".into(), json!(node.id.value));
            m.insert("_labels".into(), Value::Array(
                node.labels.value.iter().map(bolt_to_json).collect()
            ));
            for (k, v) in &node.properties.value {
                m.insert(k.value.clone(), bolt_to_json(v));
            }
            Value::Object(m)
        }
        BoltType::Relation(rel) => {
            let mut m = serde_json::Map::new();
            m.insert("_type".into(), Value::String("Relation".into()));
            m.insert("_id".into(), json!(rel.id.value));
            m.insert("_start".into(), json!(rel.start_node_id.value));
            m.insert("_end".into(), json!(rel.end_node_id.value));
            for (k, v) in &rel.properties.value {
                m.insert(k.value.clone(), bolt_to_json(v));
            }
            Value::Object(m)
        }
        BoltType::DateTime(_) => {
            use serde::de::IntoDeserializer;
            use serde::Deserialize;
            if let Ok(dt) = chrono::DateTime::<chrono::Utc>::deserialize(
                val.into_deserializer(),
            ) {
                Value::String(dt.to_rfc3339())
            } else {
                Value::String(format!("{val:?}"))
            }
        }
        BoltType::LocalDateTime(_) => {
            use serde::de::IntoDeserializer;
            use serde::Deserialize;
            if let Ok(ldt) = chrono::NaiveDateTime::deserialize(
                val.into_deserializer(),
            ) {
                Value::String(ldt.format("%Y-%m-%dT%H:%M:%S%.f").to_string())
            } else {
                Value::String(format!("{val:?}"))
            }
        }
        _ => Value::String(format!("{val:?}")),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let args = Args::parse();
    let state = McpState::new(&args).await?;

    eprintln!("[sensing-mcp] connected to Neo4j: {}", args.neo4j_url);

    // Tool: latest_event
    let state_clone = state.clone();
    let latest_event = SimpleTool::new("latest_event", move |args, _extra| {
        let state = state_clone.clone();
        Box::pin(async move {
            let room = args["room"].as_str()
                .or(state.room.as_deref())
                .unwrap_or("main");

            let results = state.query(
                "MATCH (r:Room {name: $room})-[:CURRENT_EVENT]->(e:SensingEvent)
                 OPTIONAL MATCH (e)-[:HAS_VITALS]->(v)
                 OPTIONAL MATCH (e)-[:DETECTED]->(p)
                 RETURN e.timestamp AS ts, e.source AS src, e.person_count AS person_count,
                        e.motion_level AS motion, e.presence AS presence,
                        e.signal_quality AS signal_quality,
                        v.heart_rate AS heart_rate, v.breathing_rate AS breathing_rate,
                        v.hr_confidence AS hr_confidence, v.br_confidence AS br_confidence,
                        collect(p.id) AS person_ids,
                        collect(p.confidence) AS person_confidences",
                vec![("room", room.into())],
            ).await?;

            Ok(json!({"room": room, "events": results}))
        })
    })
    .with_description("Get the latest sensing event for a room, including vital signs and detected persons")
    .with_schema(json!({
        "type": "object",
        "properties": {
            "room": {
                "type": "string",
                "description": "Room name (e.g. 'living-room'). Uses default room if not specified."
            }
        }
    }));

    // Tool: vitals_history
    let state_clone = state.clone();
    let vitals_history = SimpleTool::new("vitals_history", move |args, _extra| {
        let state = state_clone.clone();
        Box::pin(async move {
            let room = args["room"].as_str()
                .or(state.room.as_deref())
                .unwrap_or("main");
            let hours = args["hours"].as_u64().unwrap_or(1);
            let limit = args["limit"].as_u64().unwrap_or(100);

            let results = state.query(
                "MATCH (r:Room {name: $room})-[:CURRENT_EVENT]->(e:SensingEvent)
                 MATCH (e)-[:HAS_VITALS]->(v)
                 WHERE datetime(e.timestamp) > datetime() - duration({hours: $hours})
                 RETURN e.timestamp AS ts, v.heart_rate AS hr, v.breathing_rate AS br,
                        v.hr_confidence AS hr_conf, v.br_confidence AS br_conf,
                        v.signal_quality AS quality
                 ORDER BY e.timestamp DESC
                 LIMIT $limit",
                vec![
                    ("room", room.into()),
                    ("hours", (hours as i64).into()),
                    ("limit", (limit as i64).into()),
                ],
            ).await?;

            Ok(json!({"room": room, "hours": hours, "data": results}))
        })
    })
    .with_description("Get heart rate and breathing rate time series for a room")
    .with_schema(json!({
        "type": "object",
        "properties": {
            "room": { "type": "string", "description": "Room name" },
            "hours": { "type": "integer", "description": "Hours to look back (default: 1)" },
            "limit": { "type": "integer", "description": "Max data points (default: 100)" }
        }
    }));

    // Tool: person_history
    let state_clone = state.clone();
    let person_history = SimpleTool::new("person_history", move |args, _extra| {
        let state = state_clone.clone();
        Box::pin(async move {
            let room = args["room"].as_str()
                .or(state.room.as_deref())
                .unwrap_or("main");
            let hours = args["hours"].as_u64().unwrap_or(1);

            let results = state.query(
                "MATCH (r:Room {name: $room})-[:CURRENT_EVENT]->(e:SensingEvent)
                 WHERE e.person_count > 0
                 AND datetime(e.timestamp) > datetime() - duration({hours: $hours})
                 MATCH (e)-[:DETECTED]->(p:Person)
                 RETURN e.timestamp AS ts, e.person_count AS count,
                        collect({id: p.id, confidence: p.confidence, x: p.position_x, y: p.position_y}) AS persons
                 ORDER BY e.timestamp DESC",
                vec![
                    ("room", room.into()),
                    ("hours", (hours as i64).into()),
                ],
            ).await?;

            Ok(json!({"room": room, "hours": hours, "detections": results}))
        })
    })
    .with_description("Get person detection history for a room")
    .with_schema(json!({
        "type": "object",
        "properties": {
            "room": { "type": "string", "description": "Room name" },
            "hours": { "type": "integer", "description": "Hours to look back (default: 1)" }
        }
    }));

    // Tool: room_status
    let state_clone = state.clone();
    let room_status = SimpleTool::new("room_status", move |args, _extra| {
        let state = state_clone.clone();
        Box::pin(async move {
            let room = args["room"].as_str()
                .or(state.room.as_deref())
                .unwrap_or("main");

            let current = state.query(
                "MATCH (r:Room {name: $room})-[:CURRENT_EVENT]->(e:SensingEvent)
                 OPTIONAL MATCH (e)-[:HAS_VITALS]->(v)
                 OPTIONAL MATCH (e)-[:DETECTED]->(p)
                 RETURN r.last_updated AS last_updated,
                        e.timestamp AS event_ts,
                        e.source AS source,
                        e.person_count AS person_count,
                        e.motion_level AS motion,
                        e.presence AS presence,
                        v.heart_rate AS heart_rate,
                        v.breathing_rate AS breathing_rate,
                        size(collect(p)) AS detected_persons",
                vec![("room", room.into())],
            ).await?;

            let summary = state.query(
                "MATCH (r:Room {name: $room})-[:CURRENT_EVENT]->(e:SensingEvent)
                 RETURN count(e) AS total_events,
                        avg(e.signal_quality) AS avg_signal_quality,
                        min(e.timestamp) AS first_event,
                        max(e.timestamp) AS last_event",
                vec![("room", room.into())],
            ).await?;

            Ok(json!({"room": room, "current": current, "summary": summary}))
        })
    })
    .with_description("Get current status and aggregate stats for a room")
    .with_schema(json!({
        "type": "object",
        "properties": {
            "room": { "type": "string", "description": "Room name" }
        }
    }));

    // Tool: query_cypher (read-only)
    let state_clone = state.clone();
    let query_cypher = SimpleTool::new("query_cypher", move |args, _extra| {
        let state = state_clone.clone();
        Box::pin(async move {
            let cypher = args["cypher"].as_str()
                .ok_or_else(|| pmcp::Error::invalid_params("Missing 'cypher' parameter"))?;

            let upper = cypher.to_uppercase();
            if upper.contains("CREATE") || upper.contains("DELETE") || upper.contains("DETACH")
                || upper.contains("SET ") || upper.contains("REMOVE ") || upper.contains("MERGE")
            {
                return Err(pmcp::Error::invalid_params(
                    "Only READ queries allowed (MATCH, RETURN, WHERE, WITH, ORDER BY, LIMIT)"
                ));
            }

            let results = state.query(cypher, vec![]).await?;
            Ok(json!({"results": results, "count": results.len()}))
        })
    })
    .with_description("Run a read-only Cypher query against the sensing graph")
    .with_schema(json!({
        "type": "object",
        "properties": {
            "cypher": {
                "type": "string",
                "description": "Cypher query (read-only: MATCH, RETURN, WHERE, WITH, ORDER BY, LIMIT)"
            }
        },
        "required": ["cypher"]
    }));

    // Tool: rooms
    let state_clone = state.clone();
    let rooms = SimpleTool::new("rooms", move |_args, _extra| {
        let state = state_clone.clone();
        Box::pin(async move {
            let results = state.query(
                "MATCH (r:Room)
                 OPTIONAL MATCH (r)-[:CURRENT_EVENT]->(e:SensingEvent)
                 RETURN r.name AS name, r.last_updated AS last_updated,
                        e.timestamp AS latest_event_ts,
                        e.person_count AS latest_person_count,
                        e.motion_level AS latest_motion
                 ORDER BY r.name",
                vec![],
            ).await?;

            Ok(json!({"rooms": results}))
        })
    })
    .with_description("List all rooms with sensing data");

    // Build and run
    let server = Server::builder()
        .name("ruview-sensing-mcp")
        .version(env!("CARGO_PKG_VERSION"))
        .tool("latest_event", latest_event)
        .tool("vitals_history", vitals_history)
        .tool("person_history", person_history)
        .tool("room_status", room_status)
        .tool("query_cypher", query_cypher)
        .tool("rooms", rooms)
        .build()?;

    eprintln!("[sensing-mcp] server ready on stdin/stdout");
    server.run_stdio().await?;

    Ok(())
}
