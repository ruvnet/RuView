#![cfg(feature = "neo4j")]

//! Live Neo4j contract tests for the sensing graph model.
//!
//! These tests are intentionally ignored by ordinary `cargo test`: they require
//! an ephemeral Neo4j instance and are run by the focused CI workflow. They
//! exercise the graph invariants that previously regressed: durable history,
//! room isolation, CURRENT_EVENT isolation, and room-scoped retention.

use neo4rs::{query, Graph};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

async fn graph() -> Graph {
    let url = std::env::var("NEO4J_TEST_URL").unwrap_or_else(|_| "127.0.0.1:7687".into());
    let user = std::env::var("NEO4J_TEST_USER").unwrap_or_else(|_| "neo4j".into());
    let password = std::env::var("NEO4J_TEST_PASSWORD").expect("NEO4J_TEST_PASSWORD must be set");
    Graph::new(&url, &user, &password).expect("connect to ephemeral Neo4j")
}

fn room(prefix: &str) -> String {
    format!("{prefix}-{}", NEXT_ID.fetch_add(1, Ordering::Relaxed))
}

async fn seed_event(graph: &Graph, room: &str, id: &str, ts: &str) {
    graph.run(
        query("MERGE (r:Room {name:$room}) CREATE (e:SensingEvent {event_id:$id,room_name:$room,timestamp:$ts}) CREATE (r)-[:HAS_EVENT]->(e) WITH r,e OPTIONAL MATCH (r)-[old:CURRENT_EVENT]->() DELETE old CREATE (r)-[:CURRENT_EVENT]->(e)")
            .param("room", room.to_owned())
            .param("id", id.to_owned())
            .param("ts", ts.to_owned()),
    ).await.expect("seed event");
}

async fn scalar_i64(graph: &Graph, cypher: &str, room: &str) -> i64 {
    let mut result = graph.execute(query(cypher).param("room", room.to_owned())).await.expect("execute assertion query");
    result.next().await.expect("read row").expect("row").get::<i64>("n").expect("integer n")
}

async fn cleanup(graph: &Graph, rooms: &[&str]) {
    for room in rooms {
        graph.run(query("MATCH (r:Room {name:$room}) OPTIONAL MATCH (r)-[:HAS_EVENT]->(e:SensingEvent) DETACH DELETE e,r").param("room", (*room).to_owned())).await.expect("cleanup");
    }
}

#[tokio::test]
#[ignore = "requires NEO4J_TEST_PASSWORD and an ephemeral Neo4j"]
async fn history_keeps_multiple_events_and_one_current_pointer() {
    let g = graph().await;
    let a = room("history");
    seed_event(&g, &a, "a1", "2026-09-15T10:00:00Z").await;
    seed_event(&g, &a, "a2", "2026-09-15T10:01:00Z").await;
    seed_event(&g, &a, "a3", "2026-09-15T10:02:00Z").await;

    assert_eq!(3, scalar_i64(&g, "MATCH (:Room {name:$room})-[:HAS_EVENT]->(e:SensingEvent) RETURN count(e) AS n", &a).await);
    assert_eq!(1, scalar_i64(&g, "MATCH (:Room {name:$room})-[:CURRENT_EVENT]->(e:SensingEvent) RETURN count(e) AS n", &a).await);
    cleanup(&g, &[&a]).await;
}

#[tokio::test]
#[ignore = "requires NEO4J_TEST_PASSWORD and an ephemeral Neo4j"]
async fn room_history_and_current_pointer_do_not_cross_rooms() {
    let g = graph().await;
    let a = room("room-a");
    let b = room("room-b");
    seed_event(&g, &a, "a1", "2026-09-15T10:00:00Z").await;
    seed_event(&g, &b, "b1", "2026-09-15T10:01:00Z").await;
    seed_event(&g, &a, "a2", "2026-09-15T10:02:00Z").await;

    assert_eq!(2, scalar_i64(&g, "MATCH (:Room {name:$room})-[:HAS_EVENT]->(e:SensingEvent) RETURN count(e) AS n", &a).await);
    assert_eq!(1, scalar_i64(&g, "MATCH (:Room {name:$room})-[:HAS_EVENT]->(e:SensingEvent) RETURN count(e) AS n", &b).await);
    assert_eq!(0, scalar_i64(&g, "MATCH (r:Room {name:$room})-[:CURRENT_EVENT]->(e:SensingEvent) WHERE e.room_name <> r.name RETURN count(e) AS n", &a).await);
    assert_eq!(0, scalar_i64(&g, "MATCH (r:Room {name:$room})-[:CURRENT_EVENT]->(e:SensingEvent) WHERE e.room_name <> r.name RETURN count(e) AS n", &b).await);
    cleanup(&g, &[&a, &b]).await;
}

#[tokio::test]
#[ignore = "requires NEO4J_TEST_PASSWORD and an ephemeral Neo4j"]
async fn retention_for_one_room_cannot_delete_or_repoint_another_room() {
    let g = graph().await;
    let a = room("retention-a");
    let b = room("retention-b");
    seed_event(&g, &a, "a-old", "2020-01-01T00:00:00Z").await;
    seed_event(&g, &a, "a-new", "2026-09-15T10:00:00Z").await;
    seed_event(&g, &b, "b-old", "2020-01-01T00:00:00Z").await;

    g.run(query("MATCH (:Room {name:$room})-[:HAS_EVENT]->(e:SensingEvent) WHERE datetime(e.timestamp) < datetime('2025-01-01T00:00:00Z') DETACH DELETE e").param("room", a.clone())).await.expect("room-scoped retention");
    g.run(query("MATCH (r:Room {name:$room}) OPTIONAL MATCH (r)-[old:CURRENT_EVENT]->() DELETE old WITH r OPTIONAL MATCH (r)-[:HAS_EVENT]->(e:SensingEvent) WITH r,e ORDER BY e.timestamp DESC WITH r,head(collect(e)) AS newest FOREACH (_ IN CASE WHEN newest IS NULL THEN [] ELSE [1] END | CREATE (r)-[:CURRENT_EVENT]->(newest))").param("room", a.clone())).await.expect("room-scoped pointer rebuild");

    assert_eq!(1, scalar_i64(&g, "MATCH (:Room {name:$room})-[:HAS_EVENT]->(e:SensingEvent) RETURN count(e) AS n", &a).await);
    assert_eq!(1, scalar_i64(&g, "MATCH (:Room {name:$room})-[:HAS_EVENT]->(e:SensingEvent) RETURN count(e) AS n", &b).await);
    assert_eq!(0, scalar_i64(&g, "MATCH (r:Room {name:$room})-[:CURRENT_EVENT]->(e:SensingEvent) WHERE e.room_name <> r.name RETURN count(e) AS n", &a).await);
    cleanup(&g, &[&a, &b]).await;
}
