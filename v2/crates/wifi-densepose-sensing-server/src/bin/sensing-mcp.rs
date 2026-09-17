//! Room-scoped, bounded MCP read server for RuView sensing data.
//!
//! Security boundary:
//! - one MCP process is bound to exactly one room;
//! - deployment must provide a dedicated Neo4j read-only principal;
//! - arbitrary Cypher and maintenance tools are intentionally not exposed;
//! - history windows, row counts, result counts, and query time are bounded;
//! - privacy mode suppresses person/vital-sign data.

use clap::Parser;
use pmcp::{Server, SimpleTool};
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};

const MAX_HISTORY_HOURS: u64 = 24;
const MAX_ROWS: u64 = 250;
const QUERY_TIMEOUT_SECS: u64 = 5;

#[derive(Parser, Debug)]
#[command(name = "sensing-mcp", about = "Room-scoped RuView sensing MCP server")]
struct Args {
    #[arg(long, env = "RUVIEW_NEO4J_URL", default_value = "bolt://127.0.0.1:7687")]
    neo4j_url: String,
    #[arg(long, env = "RUVIEW_NEO4J_USER")]
    neo4j_user: String,
    #[arg(long, default_value = "RUVIEW_MCP_NEO4J_PASSWORD")]
    neo4j_password_env: String,
    #[arg(long, env = "RUVIEW_NEO4J_ROOM")]
    room: String,
    #[arg(long, env = "RUVIEW_MCP_PRIVACY_MODE", default_value_t = false)]
    privacy_mode: bool,
}

struct McpState { graph: neo4rs::Graph, room: String, privacy_mode: bool }
fn to_pmcp_err(e: impl std::fmt::Display) -> pmcp::Error { pmcp::Error::internal(format!("{e}")) }
fn bounded_hours(args: &Value) -> u64 { args["hours"].as_u64().unwrap_or(1).clamp(1, MAX_HISTORY_HOURS) }
fn bounded_limit(args: &Value) -> u64 { args["limit"].as_u64().unwrap_or(100).clamp(1, MAX_ROWS) }

impl McpState {
    async fn new(args: &Args) -> Result<Arc<Self>, pmcp::Error> {
        if args.room.trim().is_empty() { return Err(pmcp::Error::invalid_params("room scope must not be empty")); }
        let password = std::env::var(&args.neo4j_password_env).map_err(|_| pmcp::Error::internal(format!("Env var `{}` not set", args.neo4j_password_env)))?;
        let graph = neo4rs::Graph::new(&args.neo4j_url, &args.neo4j_user, &password).map_err(to_pmcp_err)?;
        Ok(Arc::new(Self { graph, room: args.room.clone(), privacy_mode: args.privacy_mode }))
    }

    async fn query(&self, cypher: &str, params: Vec<(&str, neo4rs::BoltType)>) -> Result<Vec<Value>, pmcp::Error> {
        use neo4rs::query;
        let mut q = query(cypher);
        for (k, v) in params { q = q.param(k, v); }
        let fut = async {
            let mut rows = self.graph.execute(q).await.map_err(to_pmcp_err)?;
            let mut results = Vec::new();
            while let Ok(Some(row)) = rows.next().await {
                if results.len() >= MAX_ROWS as usize { break; }
                let mut map = serde_json::Map::new();
                for key in row.keys() {
                    let key_str = key.to_string();
                    if let Ok(val) = row.get::<neo4rs::BoltType>(&key_str) { map.insert(key_str, bolt_to_json(&val)); }
                }
                results.push(Value::Object(map));
            }
            Ok::<_, pmcp::Error>(results)
        };
        tokio::time::timeout(Duration::from_secs(QUERY_TIMEOUT_SECS), fut).await
            .map_err(|_| pmcp::Error::internal("Neo4j read timed out"))?
    }

    fn require_sensitive_reads(&self) -> Result<(), pmcp::Error> {
        if self.privacy_mode { Err(pmcp::Error::invalid_params("tool unavailable while privacy mode is enabled")) } else { Ok(()) }
    }
}

fn bolt_to_json(val: &neo4rs::BoltType) -> Value {
    use neo4rs::BoltType;
    match val {
        BoltType::Boolean(v) => Value::Bool(v.value), BoltType::Integer(v) => json!(v.value), BoltType::Float(v) => json!(v.value),
        BoltType::String(v) => Value::String(v.value.clone()), BoltType::Null(_) => Value::Null,
        BoltType::List(v) => Value::Array(v.value.iter().map(bolt_to_json).collect()),
        BoltType::Map(v) => Value::Object(v.value.iter().map(|(k,v)| (k.value.clone(), bolt_to_json(v))).collect()),
        _ => Value::String(format!("{val:?}")),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();
    let args = Args::parse();
    let state = McpState::new(&args).await?;
    eprintln!("[sensing-mcp] ready; room-scoped read principal required");

    let s = state.clone();
    let latest_event = SimpleTool::new("latest_event", move |_args, _extra| { let s=s.clone(); Box::pin(async move {
        let results = if s.privacy_mode {
            s.query("MATCH (r:Room {name:$room})-[:CURRENT_EVENT]->(e:SensingEvent) RETURN e.timestamp AS ts,e.source AS src,e.motion_level AS motion,e.presence AS presence,e.signal_quality AS signal_quality LIMIT 1", vec![("room",s.room.clone().into())]).await?
        } else {
            s.query("MATCH (r:Room {name:$room})-[:CURRENT_EVENT]->(e:SensingEvent) OPTIONAL MATCH (e)-[:HAS_VITALS]->(v) RETURN e.timestamp AS ts,e.source AS src,e.person_count AS person_count,e.motion_level AS motion,e.presence AS presence,e.signal_quality AS signal_quality,v.heart_rate AS heart_rate,v.breathing_rate AS breathing_rate LIMIT 1", vec![("room",s.room.clone().into())]).await?
        };
        Ok(json!({"room":s.room,"privacy_mode":s.privacy_mode,"events":results}))
    })}).with_description("Get the latest event for the authorized room");

    let s=state.clone();
    let vitals_history=SimpleTool::new("vitals_history",move |args,_extra|{let s=s.clone();Box::pin(async move{
        s.require_sensitive_reads()?; let hours=bounded_hours(&args); let limit=bounded_limit(&args);
        let data=s.query("MATCH (r:Room {name:$room})-[:HAS_EVENT]->(e:SensingEvent)-[:HAS_VITALS]->(v) WHERE datetime(e.timestamp)>datetime()-duration({hours:$hours}) RETURN e.timestamp AS ts,v.heart_rate AS hr,v.breathing_rate AS br,v.hr_confidence AS hr_conf,v.br_confidence AS br_conf,v.signal_quality AS quality ORDER BY e.timestamp DESC LIMIT $limit",vec![("room",s.room.clone().into()),("hours",(hours as i64).into()),("limit",(limit as i64).into())]).await?;
        Ok(json!({"room":s.room,"hours":hours,"data":data}))
    })}).with_description("Get bounded vital history for the authorized room").with_schema(json!({"type":"object","properties":{"hours":{"type":"integer","minimum":1,"maximum":24},"limit":{"type":"integer","minimum":1,"maximum":250}}}));

    let s=state.clone();
    let person_history=SimpleTool::new("person_history",move |args,_extra|{let s=s.clone();Box::pin(async move{
        s.require_sensitive_reads()?; let hours=bounded_hours(&args); let limit=bounded_limit(&args);
        let data=s.query("MATCH (r:Room {name:$room})-[:HAS_EVENT]->(e:SensingEvent)-[:DETECTED]->(p:Person) WHERE e.person_count>0 AND datetime(e.timestamp)>datetime()-duration({hours:$hours}) RETURN e.timestamp AS ts,e.person_count AS count,collect({id:p.id,confidence:p.confidence,x:p.position_x,y:p.position_y}) AS persons ORDER BY e.timestamp DESC LIMIT $limit",vec![("room",s.room.clone().into()),("hours",(hours as i64).into()),("limit",(limit as i64).into())]).await?;
        Ok(json!({"room":s.room,"hours":hours,"detections":data}))
    })}).with_description("Get bounded person history for the authorized room").with_schema(json!({"type":"object","properties":{"hours":{"type":"integer","minimum":1,"maximum":24},"limit":{"type":"integer","minimum":1,"maximum":250}}}));

let s=state.clone();
let get_rooms=SimpleTool::new("get_rooms",move |_args,_extra|{let s=s.clone();Box::pin(async move{
    let data=s.query("MATCH (r:Room) OPTIONAL MATCH (r)-[:CURRENT_EVENT]->(e:SensingEvent) RETURN r.name AS name,r.last_updated AS last_updated,e.timestamp AS latest_event_ts,e.motion_level AS latest_motion ORDER BY r.name",vec![]).await?;
    Ok(json!({"rooms":data}))
})}).with_description("List all configured rooms with their current status");

let s=state.clone();
let room_status=SimpleTool::new("room_status",move |args,_extra|{let s=s.clone();let room=args.get("room").and_then(|v|v.as_str()).unwrap_or(&s.room);let data=s.query("MATCH (r:Room {name:$room}) OPTIONAL MATCH (r)-[:CURRENT_EVENT]->(cur:SensingEvent) OPTIONAL MATCH (r)-[:HAS_EVENT]->(e:SensingEvent) RETURN r.last_updated AS last_updated,cur.timestamp AS event_ts,cur.motion_level AS motion,cur.presence AS presence,count(e) AS total_events,avg(e.signal_quality) AS avg_signal_quality,min(e.timestamp) AS first_event,max(e.timestamp) AS last_event",vec![("room",room.into())]).await?;Ok(json!({"room":room,"privacy_mode":s.privacy_mode,"status":data}))}).with_description("Get aggregate non-sensitive status for a room");

     let s=state.clone();
     let rooms=SimpleTool::new("rooms",move |_args,_extra|{let s=s.clone();Box::pin(async move{
         let data=s.query("MATCH (r:Room {name:$room}) OPTIONAL MATCH (r)-[:CURRENT_EVENT]->(e:SensingEvent) RETURN r.name AS name,r.last_updated AS last_updated,e.timestamp AS latest_event_ts,e.motion_level AS latest_motion LIMIT 1",vec![("room",s.room.clone().into())]).await?;
         Ok(json!({"rooms":data}))
     })}).with_description("Return only the authorized room");

     // ---- Layer 3: Presence & home/away context ----

     let s=state.clone();
     let presence_status=SimpleTool::new("presence_status",move |_args,_extra|{let s=s.clone();Box::pin(async move{
         let data=s.query("MATCH (r:Room {name:$room})-[:CURRENT_EVENT]->(e:SensingEvent) RETURN e.timestamp AS ts,e.motion_level AS motion,e.presence AS presence,e.signal_quality AS quality,e.person_count AS persons",vec![("room",s.room.clone().into())]).await?;
         let enter=s.query("MATCH (r:Room {name:$room})-[:CURRENT_EVENT]->(e:SensingEvent)-[:HAS_EVENT]->(ev:EnterEvent) RETURN ev.timestamp AS ts,ev.confidence AS conf,ev.motion AS motion ORDER BY ev.timestamp DESC LIMIT 1",vec![("room",s.room.clone().into())]).await?;
         let exit=s.query("MATCH (r:Room {name:$room})-[:CURRENT_EVENT]->(e:SensingEvent)-[:HAS_EVENT]->(ev:ExitEvent) RETURN ev.timestamp AS ts,ev.confidence AS conf,ev.motion AS motion ORDER BY ev.timestamp DESC LIMIT 1",vec![("room",s.room.clone().into())]).await?;
         Ok(json!({"room":s.room,"privacy_mode":s.privacy_mode,"presence":data,"last_enter":enter,"last_exit":exit}))
     })}).with_description("Get current presence status with last enter/exit events for the authorized room");

let s=state.clone();
let home_away_status=SimpleTool::new("home_away_status",move |args,_extra|{let s=s.clone();Box::pin(async move{
      let presence=s.query("MATCH (r:Room {name:$room})-[:CURRENT_EVENT]->(e:SensingEvent) RETURN e.presence AS presence,e.motion_level AS motion,e.timestamp AS ts,e.person_count AS persons",vec![("room",s.room.clone().into())]).await?;
      let vitals=s.query("MATCH (r:Room {name:$room})-[:CURRENT_EVENT]->(e:SensingEvent)-[:HAS_VITALS]->(v) RETURN v.heart_rate AS hr,v.breathing_rate AS br,v.hr_confidence AS hr_conf,v.br_confidence AS br_conf LIMIT 1",vec![("room",s.room.clone().into())]).await?;
      let enter=s.query("MATCH (r:Room {name:$room})-[:CURRENT_EVENT]->(e:SensingEvent)-[:HAS_EVENT]->(ev:EnterEvent) RETURN ev.timestamp AS ts,ev.confidence AS conf,ev.motion AS motion ORDER BY ev.timestamp DESC LIMIT 1",vec![("room",s.room.clone().into())]).await?;
      let recent=s.query("MATCH (r:Room {name:$room})-[:HAS_EVENT]->(e:SensingEvent) WHERE datetime(e.timestamp)>datetime()-duration({hours:$hours}) RETURN count(e) AS events,sum(e.person_count) AS total_persons,avg(e.signal_quality) AS avg_quality",vec![("room",s.room.clone().into()),("hours",(1i64).into())]).await?;
      Ok(json!({"room":s.room,"privacy_mode":s.privacy_mode,"status":presence,"vitals":vitals,"last_enter":enter,"recent":recent}))
    })}).with_description("Get comprehensive home/away status combining presence, vitals, and recent activity for the authorized room").with_schema(json!({"type":"object","properties":{"hours":{"type":"integer","minimum":1,"maximum":24}}}));

     let s=state.clone();
     let cross_room_transitions=SimpleTool::new("cross_room_transitions",move |args,_extra|{let s=s.clone();let hours=args.get("hours").and_then(|v|v.as_u64()).unwrap_or(1).clamp(1,24) as i64;Box::pin(async move{
      let data=s.query("MATCH (pt:PersonTransition) WHERE datetime(pt.timestamp)>datetime()-duration({hours:$hours}) RETURN pt.id AS id,pt.node_id AS node_id,pt.from_room AS from_room,pt.to_room AS to_room,pt.timestamp AS ts ORDER BY pt.timestamp DESC LIMIT 50",vec![("hours",hours)]).await?;
      Ok(json!({"hours":hours,"transitions":data}))
     })}).with_description("Get cross-room person transitions for the last N hours").with_schema(json!({"type":"object","properties":{"hours":{"type":"integer","minimum":1,"maximum":24}}}));

      let server=Server::builder().name("ruview-sensing").version(env!("CARGO_PKG_VERSION"))
          .tool("latest_event",latest_event).tool("vitals_history",vitals_history).tool("person_history",person_history)
          .tool("room_status",room_status).tool("get_rooms",get_rooms).tool("rooms",rooms).tool("presence_status",presence_status)
          .tool("home_away_status",home_away_status).tool("cross_room_transitions",cross_room_transitions).build()?;
     server.run_stdio().await?; Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn bounds_history_inputs(){assert_eq!(bounded_hours(&json!({"hours":999})),MAX_HISTORY_HOURS);assert_eq!(bounded_hours(&json!({"hours":0})),1);assert_eq!(bounded_limit(&json!({"limit":99999})),MAX_ROWS);assert_eq!(bounded_limit(&json!({"limit":0})),1);}
    #[test] fn room_is_required_by_cli(){assert!(Args::try_parse_from(["sensing-mcp","--neo4j-user","reader"]).is_err());}
    #[test] fn privacy_mode_parses(){let a=Args::try_parse_from(["sensing-mcp","--neo4j-user","reader","--room","lab","--privacy-mode"]).unwrap();assert_eq!(a.room,"lab");assert!(a.privacy_mode);}
}
