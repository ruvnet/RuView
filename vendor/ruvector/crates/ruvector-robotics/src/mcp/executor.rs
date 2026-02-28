//! MCP tool execution engine.
//!
//! [`ToolExecutor`] wires up the perception pipeline, spatial index, and
//! memory system to actually *execute* tool requests, turning the schema-only
//! registry into a working tool backend.

use std::time::Instant;

use crate::bridge::{Point3D, PointCloud, SceneObject, SpatialIndex};
use crate::mcp::{ToolRequest, ToolResponse};
use crate::perception::PerceptionPipeline;

/// Stateful executor that handles incoming [`ToolRequest`]s by dispatching to
/// the appropriate subsystem.
pub struct ToolExecutor {
    pipeline: PerceptionPipeline,
    index: SpatialIndex,
}

impl ToolExecutor {
    /// Create a new executor with default subsystem configurations.
    pub fn new() -> Self {
        Self {
            pipeline: PerceptionPipeline::with_thresholds(0.5, 2.0),
            index: SpatialIndex::new(3),
        }
    }

    /// Execute a tool request and return a response with timing.
    pub fn execute(&mut self, request: &ToolRequest) -> ToolResponse {
        let start = Instant::now();
        let result = match request.tool_name.as_str() {
            "detect_obstacles" => self.handle_detect_obstacles(request),
            "build_scene_graph" => self.handle_build_scene_graph(request),
            "predict_trajectory" => self.handle_predict_trajectory(request),
            "focus_attention" => self.handle_focus_attention(request),
            "detect_anomalies" => self.handle_detect_anomalies(request),
            "spatial_search" => self.handle_spatial_search(request),
            "insert_points" => self.handle_insert_points(request),
            other => Err(format!("unknown tool: {other}")),
        };
        let latency_us = start.elapsed().as_micros() as u64;
        match result {
            Ok(value) => ToolResponse::ok(value, latency_us),
            Err(msg) => ToolResponse::err(msg, latency_us),
        }
    }

    /// Access the internal spatial index (e.g. for testing).
    pub fn index(&self) -> &SpatialIndex {
        &self.index
    }

    // -- handlers -----------------------------------------------------------

    fn handle_detect_obstacles(
        &self,
        req: &ToolRequest,
    ) -> std::result::Result<serde_json::Value, String> {
        let cloud = parse_point_cloud(req, "point_cloud_json")?;
        let pos = parse_position(req, "robot_position")?;
        let max_dist = req
            .arguments
            .get("max_distance")
            .and_then(|v| v.as_f64())
            .unwrap_or(20.0);

        let obstacles = self
            .pipeline
            .detect_obstacles(&cloud, pos, max_dist)
            .map_err(|e| e.to_string())?;

        serde_json::to_value(&obstacles).map_err(|e| e.to_string())
    }

    fn handle_build_scene_graph(
        &self,
        req: &ToolRequest,
    ) -> std::result::Result<serde_json::Value, String> {
        let objects: Vec<SceneObject> = parse_json_arg(req, "objects_json")?;
        let max_edge = req
            .arguments
            .get("max_edge_distance")
            .and_then(|v| v.as_f64())
            .unwrap_or(5.0);

        let graph = self
            .pipeline
            .build_scene_graph(&objects, max_edge)
            .map_err(|e| e.to_string())?;

        serde_json::to_value(&graph).map_err(|e| e.to_string())
    }

    fn handle_predict_trajectory(
        &self,
        req: &ToolRequest,
    ) -> std::result::Result<serde_json::Value, String> {
        let pos = parse_position(req, "position")?;
        let vel = parse_position(req, "velocity")?;
        let steps = req
            .arguments
            .get("steps")
            .and_then(|v| v.as_u64())
            .unwrap_or(10) as usize;
        let dt = req
            .arguments
            .get("dt")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.1);

        let traj = self
            .pipeline
            .predict_trajectory(pos, vel, steps, dt)
            .map_err(|e| e.to_string())?;

        serde_json::to_value(&traj).map_err(|e| e.to_string())
    }

    fn handle_focus_attention(
        &self,
        req: &ToolRequest,
    ) -> std::result::Result<serde_json::Value, String> {
        let cloud = parse_point_cloud(req, "point_cloud_json")?;
        let center = parse_position(req, "center")?;
        let radius = req
            .arguments
            .get("radius")
            .and_then(|v| v.as_f64())
            .ok_or("missing 'radius'")?;

        let focused = self
            .pipeline
            .focus_attention(&cloud, center, radius)
            .map_err(|e| e.to_string())?;

        serde_json::to_value(&focused).map_err(|e| e.to_string())
    }

    fn handle_detect_anomalies(
        &self,
        req: &ToolRequest,
    ) -> std::result::Result<serde_json::Value, String> {
        let cloud = parse_point_cloud(req, "point_cloud_json")?;
        let anomalies = self
            .pipeline
            .detect_anomalies(&cloud)
            .map_err(|e| e.to_string())?;
        serde_json::to_value(&anomalies).map_err(|e| e.to_string())
    }

    fn handle_spatial_search(
        &self,
        req: &ToolRequest,
    ) -> std::result::Result<serde_json::Value, String> {
        let query: Vec<f32> = req
            .arguments
            .get("query")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_f64().map(|f| f as f32)).collect())
            .ok_or("missing 'query'")?;
        let k = req
            .arguments
            .get("k")
            .and_then(|v| v.as_u64())
            .unwrap_or(5) as usize;

        let results = self
            .index
            .search_nearest(&query, k)
            .map_err(|e| e.to_string())?;

        let pairs: Vec<serde_json::Value> = results
            .iter()
            .map(|(idx, dist)| serde_json::json!({"index": idx, "distance": dist}))
            .collect();
        Ok(serde_json::json!(pairs))
    }

    fn handle_insert_points(
        &mut self,
        req: &ToolRequest,
    ) -> std::result::Result<serde_json::Value, String> {
        let points: Vec<Point3D> = parse_json_arg(req, "points_json")?;
        let cloud = PointCloud::new(points, 0);
        self.index.insert_point_cloud(&cloud);
        Ok(serde_json::json!({"inserted": cloud.len(), "total": self.index.len()}))
    }
}

impl Default for ToolExecutor {
    fn default() -> Self {
        Self::new()
    }
}

// -- argument parsers -------------------------------------------------------

fn parse_point_cloud(
    req: &ToolRequest,
    key: &str,
) -> std::result::Result<PointCloud, String> {
    let raw = req
        .arguments
        .get(key)
        .ok_or_else(|| format!("missing '{key}'"))?;

    if let Some(s) = raw.as_str() {
        serde_json::from_str(s).map_err(|e| format!("invalid point cloud JSON: {e}"))
    } else {
        serde_json::from_value(raw.clone()).map_err(|e| format!("invalid point cloud: {e}"))
    }
}

fn parse_position(
    req: &ToolRequest,
    key: &str,
) -> std::result::Result<[f64; 3], String> {
    let arr = req
        .arguments
        .get(key)
        .and_then(|v| v.as_array())
        .ok_or_else(|| format!("missing '{key}'"))?;

    if arr.len() < 3 {
        return Err(format!("'{key}' must have at least 3 elements"));
    }
    let x = arr[0].as_f64().ok_or("non-numeric")?;
    let y = arr[1].as_f64().ok_or("non-numeric")?;
    let z = arr[2].as_f64().ok_or("non-numeric")?;
    Ok([x, y, z])
}

fn parse_json_arg<T: serde::de::DeserializeOwned>(
    req: &ToolRequest,
    key: &str,
) -> std::result::Result<T, String> {
    let raw = req
        .arguments
        .get(key)
        .ok_or_else(|| format!("missing '{key}'"))?;

    if let Some(s) = raw.as_str() {
        serde_json::from_str(s).map_err(|e| format!("invalid JSON for '{key}': {e}"))
    } else {
        serde_json::from_value(raw.clone()).map_err(|e| format!("invalid '{key}': {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_request(tool: &str, args: serde_json::Value) -> ToolRequest {
        let arguments: HashMap<String, serde_json::Value> =
            serde_json::from_value(args).unwrap();
        ToolRequest { tool_name: tool.to_string(), arguments }
    }

    #[test]
    fn test_detect_obstacles() {
        let mut exec = ToolExecutor::new();
        let cloud = PointCloud::new(
            vec![
                Point3D::new(2.0, 0.0, 0.0),
                Point3D::new(2.1, 0.0, 0.0),
                Point3D::new(2.0, 0.1, 0.0),
            ],
            1000,
        );
        let cloud_json = serde_json::to_string(&cloud).unwrap();
        let req = make_request("detect_obstacles", serde_json::json!({
            "point_cloud_json": cloud_json,
            "robot_position": [0.0, 0.0, 0.0],
        }));
        let resp = exec.execute(&req);
        assert!(resp.success);
    }

    #[test]
    fn test_predict_trajectory() {
        let mut exec = ToolExecutor::new();
        let req = make_request("predict_trajectory", serde_json::json!({
            "position": [0.0, 0.0, 0.0],
            "velocity": [1.0, 0.0, 0.0],
            "steps": 5,
            "dt": 0.5,
        }));
        let resp = exec.execute(&req);
        assert!(resp.success);
        let traj = resp.result;
        assert_eq!(traj["waypoints"].as_array().unwrap().len(), 5);
    }

    #[test]
    fn test_insert_and_search() {
        let mut exec = ToolExecutor::new();

        // Insert points
        let points = vec![
            Point3D::new(1.0, 0.0, 0.0),
            Point3D::new(2.0, 0.0, 0.0),
            Point3D::new(10.0, 0.0, 0.0),
        ];
        let points_json = serde_json::to_string(&points).unwrap();
        let req = make_request("insert_points", serde_json::json!({
            "points_json": points_json,
        }));
        let resp = exec.execute(&req);
        assert!(resp.success);
        assert_eq!(resp.result["total"], 3);

        // Search
        let req = make_request("spatial_search", serde_json::json!({
            "query": [1.0, 0.0, 0.0],
            "k": 2,
        }));
        let resp = exec.execute(&req);
        assert!(resp.success);
        let results = resp.result.as_array().unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_unknown_tool() {
        let mut exec = ToolExecutor::new();
        let req = make_request("nonexistent", serde_json::json!({}));
        let resp = exec.execute(&req);
        assert!(!resp.success);
        assert!(resp.error.unwrap().contains("unknown tool"));
    }

    #[test]
    fn test_build_scene_graph() {
        let mut exec = ToolExecutor::new();
        let objects = vec![
            SceneObject::new(0, [0.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
            SceneObject::new(1, [2.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
        ];
        let objects_json = serde_json::to_string(&objects).unwrap();
        let req = make_request("build_scene_graph", serde_json::json!({
            "objects_json": objects_json,
            "max_edge_distance": 5.0,
        }));
        let resp = exec.execute(&req);
        assert!(resp.success);
        assert_eq!(resp.result["edges"].as_array().unwrap().len(), 1);
    }
}
