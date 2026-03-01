//! Robotics Integration Benchmarks
//!
//! Comprehensive benchmarks for the ruvector-robotics crate covering:
//! 1. Point cloud conversion
//! 2. Spatial search (brute-force kNN via SpatialIndex)
//! 3. Obstacle detection pipeline
//! 4. Trajectory prediction (linear, polynomial)
//! 5. Attention computation (spatial softmax)
//! 6. Behavior tree tick
//! 7. Scene graph construction
//! 8. Episodic memory recall (cosine similarity)
//! 9. Full perceive->think->act pipeline
//! 10. Swarm task assignment
//!
//! Run with: cargo bench --bench robotics_benchmarks -p ruvector-robotics

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::collections::HashMap;

use ruvector_robotics::bridge::{
    GaussianConfig, Obstacle, OccupancyGrid, Point3D, PointCloud, RobotState, SceneObject,
    SpatialIndex,
};
use ruvector_robotics::bridge::gaussian::{gaussians_from_cloud, to_viewer_json};
use ruvector_robotics::cognitive::behavior_tree::{BehaviorNode, BehaviorStatus, BehaviorTree};
use ruvector_robotics::mcp::executor::ToolExecutor;
use ruvector_robotics::mcp::ToolRequest;
use ruvector_robotics::perception::sensor_fusion::{fuse_clouds, FusionConfig};
use ruvector_robotics::planning::{astar, potential_field, PotentialFieldConfig};

// ---------------------------------------------------------------------------
// Data generators
// ---------------------------------------------------------------------------

/// Deterministic pseudo-random f32 in [0, 1).
fn pseudo_random_f32(seed: u64, index: usize) -> f32 {
    let h = seed
        .wrapping_mul(index as u64 + 1)
        .wrapping_mul(0x5DEECE66D)
        .wrapping_add(0xB);
    ((h % 10000) as f32) / 10000.0
}

/// Generate N random points in a 10x10x10 space.
fn generate_point_cloud(n: usize) -> Vec<Point3D> {
    (0..n)
        .map(|i| {
            Point3D::new(
                pseudo_random_f32(42, i * 3) * 10.0,
                pseudo_random_f32(42, i * 3 + 1) * 10.0,
                pseudo_random_f32(42, i * 3 + 2) * 10.0,
            )
        })
        .collect()
}

/// Generate N sequential robot states with smooth motion.
fn generate_robot_states(n: usize) -> Vec<RobotState> {
    (0..n)
        .map(|i| {
            let t = i as f64 * 0.01;
            RobotState {
                position: [t, (t * 0.5).sin(), 0.0],
                velocity: [1.0, (t * 0.5).cos() * 0.5, 0.0],
                acceleration: [0.0, -(t * 0.5).sin() * 0.25, 0.0],
                timestamp_us: (i as i64) * 10_000,
            }
        })
        .collect()
}

/// Generate N scene objects spread in a circle.
fn generate_scene_objects(n: usize) -> Vec<SceneObject> {
    (0..n)
        .map(|i| {
            let angle = (i as f64) * 2.0 * std::f64::consts::PI / (n as f64);
            let r = 5.0 + (i as f64) * 0.1;
            let mut obj = SceneObject::new(
                i,
                [r * angle.cos(), r * angle.sin(), 0.0],
                [0.5, 0.5, 1.8],
            );
            obj.label = if i % 3 == 0 {
                "person".to_string()
            } else if i % 3 == 1 {
                "forklift".to_string()
            } else {
                "pallet".to_string()
            };
            obj.confidence = 0.8 + pseudo_random_f32(99, i) * 0.2;
            obj.velocity = Some([
                pseudo_random_f32(77, i) as f64 - 0.5,
                pseudo_random_f32(88, i) as f64 - 0.5,
                0.0,
            ]);
            obj
        })
        .collect()
}

// ---------------------------------------------------------------------------
// 1. Point Cloud Conversion
// ---------------------------------------------------------------------------

fn bench_point_cloud_conversion(c: &mut Criterion) {
    let mut group = c.benchmark_group("PointCloud_Conversion");

    for size in [100, 1_000, 10_000, 100_000] {
        group.throughput(Throughput::Elements(size as u64));

        group.bench_with_input(
            BenchmarkId::new("to_flat_vectors", size),
            &size,
            |b, &n| {
                let points = generate_point_cloud(n);
                b.iter(|| {
                    let vectors: Vec<Vec<f32>> = points.iter().map(|p| p.to_vec()).collect();
                    black_box(vectors)
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("to_flat_array", size),
            &size,
            |b, &n| {
                let points = generate_point_cloud(n);
                b.iter(|| {
                    let flat: Vec<f32> = points
                        .iter()
                        .flat_map(|p| [p.x, p.y, p.z])
                        .collect();
                    black_box(flat)
                })
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// 2. Spatial Search (SpatialIndex kNN)
// ---------------------------------------------------------------------------

fn bench_spatial_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("Spatial_Search");

    for index_size in [100, 1_000, 10_000, 50_000] {
        group.throughput(Throughput::Elements(index_size as u64));

        group.bench_with_input(
            BenchmarkId::new("spatial_index_knn_k10", index_size),
            &index_size,
            |b, &n| {
                let points = generate_point_cloud(n);
                let cloud = PointCloud::new(points, 0);
                let mut index = SpatialIndex::new(3);
                index.insert_point_cloud(&cloud);
                let query = [5.0, 5.0, 5.0];

                b.iter(|| {
                    let result = index.search_nearest(&query, 10).unwrap();
                    black_box(result)
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("brute_range_search_r2", index_size),
            &index_size,
            |b, &n| {
                let points = generate_point_cloud(n);
                let query = Point3D::new(5.0, 5.0, 5.0);
                let radius = 2.0f32;

                b.iter(|| {
                    let results: Vec<(usize, f32)> = points
                        .iter()
                        .enumerate()
                        .filter_map(|(i, p)| {
                            let d = query.distance_to(p);
                            if d <= radius {
                                Some((i, d))
                            } else {
                                None
                            }
                        })
                        .collect();
                    black_box(results)
                })
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// 3. Obstacle Detection Pipeline
// ---------------------------------------------------------------------------

fn bench_obstacle_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("Obstacle_Detection");

    let safety_radius = 2.0f64;

    for n_points in [500, 2_000, 10_000, 50_000] {
        group.throughput(Throughput::Elements(n_points as u64));

        group.bench_with_input(
            BenchmarkId::new("detect_and_classify", n_points),
            &n_points,
            |b, &n| {
                let points = generate_point_cloud(n);
                let robot_pos = [5.0f64, 5.0, 0.0];
                let cluster_radius = 0.5f32;
                let min_cluster_size = 3usize;

                b.iter(|| {
                    let cell_size = cluster_radius;
                    let mut grid: HashMap<(i32, i32, i32), Vec<usize>> = HashMap::new();
                    for (i, p) in points.iter().enumerate() {
                        let key = (
                            (p.x / cell_size) as i32,
                            (p.y / cell_size) as i32,
                            (p.z / cell_size) as i32,
                        );
                        grid.entry(key).or_default().push(i);
                    }

                    let mut obstacles = Vec::new();
                    let mut obs_id = 0u64;
                    for indices in grid.values() {
                        if indices.len() >= min_cluster_size {
                            let cx = indices.iter().map(|&i| points[i].x as f64).sum::<f64>()
                                / indices.len() as f64;
                            let cy = indices.iter().map(|&i| points[i].y as f64).sum::<f64>()
                                / indices.len() as f64;
                            let cz = indices.iter().map(|&i| points[i].z as f64).sum::<f64>()
                                / indices.len() as f64;

                            let dx = cx - robot_pos[0];
                            let dy = cy - robot_pos[1];
                            let dz = cz - robot_pos[2];
                            let dist = (dx * dx + dy * dy + dz * dz).sqrt();

                            obstacles.push(Obstacle {
                                id: obs_id,
                                position: [cx, cy, cz],
                                distance: dist,
                                radius: 0.5,
                                label: String::new(),
                                confidence: 0.9,
                            });
                            obs_id += 1;
                        }
                    }

                    let critical_count = obstacles
                        .iter()
                        .filter(|obs| obs.distance < safety_radius)
                        .count();
                    black_box((obstacles.len(), critical_count))
                })
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// 4. Trajectory Prediction
// ---------------------------------------------------------------------------

fn bench_trajectory_prediction(c: &mut Criterion) {
    let mut group = c.benchmark_group("Trajectory_Prediction");

    for history_len in [10, 50, 200, 500] {
        group.throughput(Throughput::Elements(history_len as u64));

        group.bench_with_input(
            BenchmarkId::new("linear", history_len),
            &history_len,
            |b, &n| {
                let states = generate_robot_states(n);
                let horizon = 20usize;

                b.iter(|| {
                    let last = &states[states.len() - 1];
                    let prev = &states[states.len() - 2];
                    let dt = (last.timestamp_us - prev.timestamp_us) as f64 / 1_000_000.0;

                    let predicted: Vec<[f64; 3]> = (1..=horizon)
                        .map(|step| {
                            let t = step as f64 * dt;
                            [
                                last.position[0] + last.velocity[0] * t,
                                last.position[1] + last.velocity[1] * t,
                                last.position[2] + last.velocity[2] * t,
                            ]
                        })
                        .collect();
                    black_box(predicted)
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("polynomial_deg2", history_len),
            &history_len,
            |b, &n| {
                let states = generate_robot_states(n);
                let horizon = 20usize;

                b.iter(|| {
                    let t0 = states[0].timestamp_us as f64 / 1_000_000.0;
                    let times: Vec<f64> = states
                        .iter()
                        .map(|s| s.timestamp_us as f64 / 1_000_000.0 - t0)
                        .collect();

                    let mut coeffs = [[0.0f64; 3]; 3];
                    for axis in 0..3 {
                        let vals: Vec<f64> = states.iter().map(|s| s.position[axis]).collect();
                        let nn = times.len() as f64;
                        let s1: f64 = times.iter().sum();
                        let s2: f64 = times.iter().map(|t| t * t).sum();
                        let s3: f64 = times.iter().map(|t| t * t * t).sum();
                        let s4: f64 = times.iter().map(|t| t * t * t * t).sum();
                        let sy: f64 = vals.iter().sum();
                        let sty: f64 = times.iter().zip(vals.iter()).map(|(t, y)| t * y).sum();
                        let st2y: f64 =
                            times.iter().zip(vals.iter()).map(|(t, y)| t * t * y).sum();

                        let det = nn * (s2 * s4 - s3 * s3)
                            - s1 * (s1 * s4 - s3 * s2)
                            + s2 * (s1 * s3 - s2 * s2);
                        if det.abs() > 1e-12 {
                            coeffs[axis][0] = (sy * (s2 * s4 - s3 * s3)
                                - s1 * (sty * s4 - st2y * s3)
                                + s2 * (sty * s3 - st2y * s2))
                                / det;
                            coeffs[axis][1] = (nn * (sty * s4 - st2y * s3)
                                - sy * (s1 * s4 - s3 * s2)
                                + s2 * (s1 * st2y - sty * s2))
                                / det;
                            coeffs[axis][2] = (nn * (s2 * st2y - s3 * sty)
                                - s1 * (s1 * st2y - sty * s2)
                                + sy * (s1 * s3 - s2 * s2))
                                / det;
                        }
                    }

                    let t_last = *times.last().unwrap();
                    let predicted: Vec<[f64; 3]> = (1..=horizon)
                        .map(|step| {
                            let t = t_last + step as f64 * 0.01;
                            [
                                coeffs[0][0] + coeffs[0][1] * t + coeffs[0][2] * t * t,
                                coeffs[1][0] + coeffs[1][1] * t + coeffs[1][2] * t * t,
                                coeffs[2][0] + coeffs[2][1] * t + coeffs[2][2] * t * t,
                            ]
                        })
                        .collect();
                    black_box(predicted)
                })
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// 5. Attention Computation
// ---------------------------------------------------------------------------

fn bench_attention_computation(c: &mut Criterion) {
    let mut group = c.benchmark_group("Attention_Computation");

    for n_objects in [10, 50, 200, 500] {
        group.throughput(Throughput::Elements(n_objects as u64));

        group.bench_with_input(
            BenchmarkId::new("softmax_spatial_attn", n_objects),
            &n_objects,
            |b, &n| {
                let objects = generate_scene_objects(n);
                let robot_pos = [0.0f64, 0.0, 0.0];
                let temperature = 1.0f64;

                b.iter(|| {
                    let scores: Vec<f64> = objects
                        .iter()
                        .map(|obj| {
                            let dx = obj.center[0] - robot_pos[0];
                            let dy = obj.center[1] - robot_pos[1];
                            let dz = obj.center[2] - robot_pos[2];
                            let dist = (dx * dx + dy * dy + dz * dz).sqrt().max(0.1);
                            let vel_mag = obj
                                .velocity
                                .map(|v| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt())
                                .unwrap_or(0.0);
                            (1.0 / dist + vel_mag) / temperature
                        })
                        .collect();

                    let max_score = scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                    let exp_scores: Vec<f64> =
                        scores.iter().map(|s| (s - max_score).exp()).collect();
                    let sum: f64 = exp_scores.iter().sum();
                    let weights: Vec<f64> = exp_scores.iter().map(|e| e / sum).collect();
                    black_box(weights)
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("top_k_attn_k5", n_objects),
            &n_objects,
            |b, &n| {
                let objects = generate_scene_objects(n);
                let robot_pos = [0.0f64, 0.0, 0.0];
                let k = 5usize.min(n);

                b.iter(|| {
                    let mut scored: Vec<(usize, f64)> = objects
                        .iter()
                        .enumerate()
                        .map(|(i, obj)| {
                            let dx = obj.center[0] - robot_pos[0];
                            let dy = obj.center[1] - robot_pos[1];
                            let dist = (dx * dx + dy * dy).sqrt().max(0.1);
                            (i, 1.0 / dist)
                        })
                        .collect();
                    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
                    scored.truncate(k);
                    let sum: f64 = scored.iter().map(|(_, s)| s).sum();
                    let weights: Vec<(usize, f64)> =
                        scored.iter().map(|(i, s)| (*i, s / sum)).collect();
                    black_box(weights)
                })
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// 6. Behavior Tree Tick
// ---------------------------------------------------------------------------

fn bench_behavior_tree_tick(c: &mut Criterion) {
    let mut group = c.benchmark_group("BehaviorTree_Tick");

    for depth in [3, 5, 8, 12] {
        let n_leaves = 1usize << depth;
        group.throughput(Throughput::Elements(n_leaves as u64));

        group.bench_with_input(
            BenchmarkId::new("sequence_tree", depth),
            &depth,
            |b, &d| {
                fn build_seq(depth: usize) -> BehaviorNode {
                    if depth == 0 {
                        BehaviorNode::Action("leaf".into())
                    } else {
                        BehaviorNode::Sequence(vec![
                            build_seq(depth - 1),
                            BehaviorNode::Action("leaf".into()),
                        ])
                    }
                }
                let root = build_seq(d);
                let mut tree = BehaviorTree::new(root);
                tree.set_action_result("leaf", BehaviorStatus::Success);

                b.iter(|| {
                    let status = tree.tick();
                    black_box(status)
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("selector_tree", depth),
            &depth,
            |b, &d| {
                fn build_sel(depth: usize) -> BehaviorNode {
                    if depth == 0 {
                        BehaviorNode::Action("leaf".into())
                    } else {
                        BehaviorNode::Selector(vec![
                            BehaviorNode::Action("fail".into()),
                            build_sel(depth - 1),
                        ])
                    }
                }
                let root = build_sel(d);
                let mut tree = BehaviorTree::new(root);
                tree.set_action_result("fail", BehaviorStatus::Failure);
                tree.set_action_result("leaf", BehaviorStatus::Success);

                b.iter(|| {
                    let status = tree.tick();
                    black_box(status)
                })
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// 7. Scene Graph Construction
// ---------------------------------------------------------------------------

fn bench_scene_graph_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("SceneGraph_Construction");

    for n_objects in [10, 50, 100, 300] {
        group.throughput(Throughput::Elements(n_objects as u64));

        group.bench_with_input(
            BenchmarkId::new("build_edges_from_objects", n_objects),
            &n_objects,
            |b, &n| {
                let objects = generate_scene_objects(n);
                let edge_threshold = 5.0f64;

                b.iter(|| {
                    let mut edges = Vec::new();
                    for i in 0..objects.len() {
                        for j in (i + 1)..objects.len() {
                            let dx = objects[i].center[0] - objects[j].center[0];
                            let dy = objects[i].center[1] - objects[j].center[1];
                            let dz = objects[i].center[2] - objects[j].center[2];
                            let dist = (dx * dx + dy * dy + dz * dz).sqrt();
                            if dist <= edge_threshold {
                                edges.push((i, j, dist));
                            }
                        }
                    }
                    black_box(edges.len())
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("from_point_cloud_clustered", n_objects),
            &n_objects,
            |b, &n| {
                let pts_per_cluster = 20usize;
                let centers = generate_scene_objects(n);
                let mut all_points = Vec::with_capacity(n * pts_per_cluster);
                for center in &centers {
                    for j in 0..pts_per_cluster {
                        let offset = (j as f32) * 0.05;
                        all_points.push(Point3D::new(
                            center.center[0] as f32 + offset,
                            center.center[1] as f32 + offset * 0.5,
                            center.center[2] as f32,
                        ));
                    }
                }

                b.iter(|| {
                    let cell_size = 1.0f32;
                    let mut grid: HashMap<(i32, i32, i32), Vec<usize>> = HashMap::new();
                    for (i, p) in all_points.iter().enumerate() {
                        let key = (
                            (p.x / cell_size) as i32,
                            (p.y / cell_size) as i32,
                            (p.z / cell_size) as i32,
                        );
                        grid.entry(key).or_default().push(i);
                    }
                    let cluster_count = grid.values().filter(|v| v.len() >= 3).count();
                    black_box(cluster_count)
                })
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// 8. Episodic Memory Recall
// ---------------------------------------------------------------------------

fn bench_memory_recall(c: &mut Criterion) {
    let mut group = c.benchmark_group("Memory_Recall");

    for n_episodes in [100, 1_000, 5_000, 20_000] {
        group.throughput(Throughput::Elements(n_episodes as u64));

        group.bench_with_input(
            BenchmarkId::new("cosine_sim_recall", n_episodes),
            &n_episodes,
            |b, &n| {
                let dim = 64usize;
                let episodes: Vec<Vec<f32>> = (0..n)
                    .map(|i| {
                        (0..dim)
                            .map(|d| pseudo_random_f32(i as u64 + 1000, d))
                            .collect()
                    })
                    .collect();
                let query: Vec<f32> = (0..dim).map(|d| pseudo_random_f32(9999, d)).collect();

                b.iter(|| {
                    let q_norm: f32 =
                        query.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-10);
                    let mut sims: Vec<(usize, f32)> = episodes
                        .iter()
                        .enumerate()
                        .map(|(i, ep)| {
                            let dot: f32 =
                                query.iter().zip(ep.iter()).map(|(a, b)| a * b).sum();
                            let ep_norm: f32 =
                                ep.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-10);
                            (i, dot / (q_norm * ep_norm))
                        })
                        .collect();
                    sims.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
                    sims.truncate(5);
                    black_box(sims)
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("euclidean_recall", n_episodes),
            &n_episodes,
            |b, &n| {
                let dim = 64usize;
                let episodes: Vec<Vec<f32>> = (0..n)
                    .map(|i| {
                        (0..dim)
                            .map(|d| pseudo_random_f32(i as u64 + 2000, d))
                            .collect()
                    })
                    .collect();
                let query: Vec<f32> =
                    (0..dim).map(|d| pseudo_random_f32(8888, d)).collect();

                b.iter(|| {
                    let mut dists: Vec<(usize, f32)> = episodes
                        .iter()
                        .enumerate()
                        .map(|(i, ep)| {
                            let dist_sq: f32 = query
                                .iter()
                                .zip(ep.iter())
                                .map(|(a, b)| (a - b) * (a - b))
                                .sum();
                            (i, dist_sq)
                        })
                        .collect();
                    dists.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
                    dists.truncate(5);
                    black_box(dists)
                })
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// 9. Full Pipeline (perceive -> think -> act)
// ---------------------------------------------------------------------------

fn bench_full_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("Full_Pipeline");

    for n_points in [500, 2_000, 10_000] {
        group.throughput(Throughput::Elements(n_points as u64));

        group.bench_with_input(
            BenchmarkId::new("perceive_think_act", n_points),
            &n_points,
            |b, &n| {
                let points = generate_point_cloud(n);
                let robot = RobotState {
                    position: [5.0, 5.0, 0.0],
                    velocity: [1.0, 0.0, 0.0],
                    acceleration: [0.0; 3],
                    timestamp_us: 1_000_000,
                };

                b.iter(|| {
                    // PERCEIVE
                    let cell_size = 0.5f32;
                    let mut grid: HashMap<(i32, i32, i32), Vec<usize>> = HashMap::new();
                    for (i, p) in points.iter().enumerate() {
                        let key = (
                            (p.x / cell_size) as i32,
                            (p.y / cell_size) as i32,
                            (p.z / cell_size) as i32,
                        );
                        grid.entry(key).or_default().push(i);
                    }
                    let obstacle_positions: Vec<[f64; 3]> = grid
                        .values()
                        .filter(|indices| indices.len() >= 3)
                        .map(|indices| {
                            let cx = indices.iter().map(|&i| points[i].x as f64).sum::<f64>()
                                / indices.len() as f64;
                            let cy = indices.iter().map(|&i| points[i].y as f64).sum::<f64>()
                                / indices.len() as f64;
                            [cx, cy, 0.0]
                        })
                        .collect();

                    // THINK
                    let max_threat = obstacle_positions
                        .iter()
                        .map(|pos| {
                            let dx = pos[0] - robot.position[0];
                            let dy = pos[1] - robot.position[1];
                            1.0 / (dx * dx + dy * dy).sqrt().max(0.1)
                        })
                        .fold(0.0f64, f64::max);

                    // ACT
                    let cmd = if max_threat > 2.0 {
                        [0.0, 0.0, 0.0]
                    } else if max_threat > 0.5 {
                        [-0.5, 0.0, 0.0]
                    } else {
                        [1.0, 0.0, 0.0]
                    };

                    black_box((obstacle_positions.len(), cmd))
                })
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// 10. Swarm Task Assignment
// ---------------------------------------------------------------------------

fn bench_swarm_task_assignment(c: &mut Criterion) {
    let mut group = c.benchmark_group("Swarm_TaskAssignment");

    for (n_robots, n_tasks) in [(3, 10), (5, 20), (10, 50), (20, 100)] {
        let label = format!("{}r_{}t", n_robots, n_tasks);
        group.throughput(Throughput::Elements(n_tasks as u64));

        group.bench_with_input(
            BenchmarkId::new("greedy_assignment", &label),
            &(n_robots, n_tasks),
            |b, &(nr, nt)| {
                let robots: Vec<[f64; 3]> = (0..nr)
                    .map(|i| {
                        let angle = (i as f64) * 2.0 * std::f64::consts::PI / (nr as f64);
                        [5.0 * angle.cos(), 5.0 * angle.sin(), 0.0]
                    })
                    .collect();
                let tasks: Vec<[f64; 3]> = (0..nt)
                    .map(|i| {
                        [
                            pseudo_random_f32(300, i * 2) as f64 * 20.0 - 10.0,
                            pseudo_random_f32(300, i * 2 + 1) as f64 * 20.0 - 10.0,
                            0.0,
                        ]
                    })
                    .collect();

                b.iter(|| {
                    let mut assignments: Vec<(usize, usize)> = Vec::with_capacity(nt);
                    let mut available: Vec<bool> = vec![true; nt];
                    let mut robot_load: Vec<usize> = vec![0; nr];

                    for _ in 0..nt {
                        let mut best_cost = f64::MAX;
                        let mut best_robot = 0;
                        let mut best_task = 0;

                        for (ri, rpos) in robots.iter().enumerate() {
                            for (ti, tpos) in tasks.iter().enumerate() {
                                if !available[ti] {
                                    continue;
                                }
                                let dx = rpos[0] - tpos[0];
                                let dy = rpos[1] - tpos[1];
                                let cost =
                                    (dx * dx + dy * dy).sqrt() + (robot_load[ri] as f64) * 2.0;
                                if cost < best_cost {
                                    best_cost = cost;
                                    best_robot = ri;
                                    best_task = ti;
                                }
                            }
                        }

                        available[best_task] = false;
                        robot_load[best_robot] += 1;
                        assignments.push((best_robot, best_task));
                    }

                    black_box(assignments)
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("auction_assignment", &label),
            &(n_robots, n_tasks),
            |b, &(nr, nt)| {
                let robots: Vec<[f64; 3]> = (0..nr)
                    .map(|i| {
                        let angle = (i as f64) * 2.0 * std::f64::consts::PI / (nr as f64);
                        [5.0 * angle.cos(), 5.0 * angle.sin(), 0.0]
                    })
                    .collect();
                let tasks: Vec<[f64; 3]> = (0..nt)
                    .map(|i| {
                        [
                            pseudo_random_f32(400, i * 2) as f64 * 20.0 - 10.0,
                            pseudo_random_f32(400, i * 2 + 1) as f64 * 20.0 - 10.0,
                            0.0,
                        ]
                    })
                    .collect();

                b.iter(|| {
                    let mut prices = vec![0.0f64; nt];
                    let mut assignments: Vec<Option<usize>> = vec![None; nt];
                    let epsilon = 0.1;

                    for _round in 0..5 {
                        for ri in 0..nr {
                            let mut best_val = f64::NEG_INFINITY;
                            let mut second_val = f64::NEG_INFINITY;
                            let mut best_task = 0;

                            for ti in 0..nt {
                                let dx = robots[ri][0] - tasks[ti][0];
                                let dy = robots[ri][1] - tasks[ti][1];
                                let value =
                                    100.0 - (dx * dx + dy * dy).sqrt() - prices[ti];
                                if value > best_val {
                                    second_val = best_val;
                                    best_val = value;
                                    best_task = ti;
                                } else if value > second_val {
                                    second_val = value;
                                }
                            }

                            prices[best_task] += best_val - second_val + epsilon;
                            assignments[best_task] = Some(ri);
                        }
                    }

                    let final_assignments: Vec<(usize, usize)> = assignments
                        .iter()
                        .enumerate()
                        .filter_map(|(ti, robot)| robot.map(|ri| (ri, ti)))
                        .collect();
                    black_box(final_assignments)
                })
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// 11. Gaussian Splatting
// ---------------------------------------------------------------------------

fn bench_gaussian_splatting(c: &mut Criterion) {
    let mut group = c.benchmark_group("Gaussian_Splatting");

    for n_points in [100, 1_000, 5_000, 20_000] {
        group.throughput(Throughput::Elements(n_points as u64));

        group.bench_with_input(
            BenchmarkId::new("cloud_to_gaussians", n_points),
            &n_points,
            |b, &n| {
                let points = generate_point_cloud(n);
                let cloud = PointCloud::new(points, 1000);
                let config = GaussianConfig::default();
                b.iter(|| {
                    let gs = gaussians_from_cloud(&cloud, &config);
                    black_box(gs.len())
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("to_viewer_json", n_points),
            &n_points,
            |b, &n| {
                let points = generate_point_cloud(n);
                let cloud = PointCloud::new(points, 1000);
                let gs = gaussians_from_cloud(&cloud, &GaussianConfig::default());
                b.iter(|| {
                    let json = to_viewer_json(&gs);
                    black_box(json)
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("small_cell_high_resolution", n_points),
            &n_points,
            |b, &n| {
                let points = generate_point_cloud(n);
                let cloud = PointCloud::new(points, 1000);
                let config = GaussianConfig {
                    cell_size: 0.1,
                    min_cluster_size: 1,
                    ..Default::default()
                };
                b.iter(|| {
                    let gs = gaussians_from_cloud(&cloud, &config);
                    black_box(gs.len())
                })
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// 12. A* Pathfinding
// ---------------------------------------------------------------------------

fn bench_astar_planning(c: &mut Criterion) {
    let mut group = c.benchmark_group("AStar_Planning");

    for grid_size in [20, 50, 100, 200] {
        let cells = grid_size * grid_size;
        group.throughput(Throughput::Elements(cells as u64));

        group.bench_with_input(
            BenchmarkId::new("free_grid_corner_to_corner", grid_size),
            &grid_size,
            |b, &sz| {
                let grid = OccupancyGrid::new(sz, sz, 1.0);
                b.iter(|| {
                    let path = astar(&grid, (0, 0), (sz - 1, sz - 1)).unwrap();
                    black_box(path.cells.len())
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("sparse_obstacles", grid_size),
            &grid_size,
            |b, &sz| {
                let mut grid = OccupancyGrid::new(sz, sz, 1.0);
                // Add walls at 1/4, 1/2, 3/4 with gaps.
                for frac in [4, 2] {
                    let wall_x = sz / frac;
                    for y in 0..(sz * 3 / 4) {
                        grid.set(wall_x, y, 1.0);
                    }
                }
                b.iter(|| {
                    let path = astar(&grid, (0, sz / 2), (sz - 1, sz / 2));
                    black_box(path.map(|p| p.cells.len()).unwrap_or(0))
                })
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// 13. Potential Field Planning
// ---------------------------------------------------------------------------

fn bench_potential_field_planning(c: &mut Criterion) {
    let mut group = c.benchmark_group("PotentialField_Planning");

    for n_obstacles in [0, 10, 50, 200, 1000] {
        group.throughput(Throughput::Elements(n_obstacles as u64));

        group.bench_with_input(
            BenchmarkId::new("compute_velocity", n_obstacles),
            &n_obstacles,
            |b, &n| {
                let robot = [0.0, 0.0, 0.0];
                let goal = [10.0, 10.0, 0.0];
                let obstacles: Vec<[f64; 3]> = (0..n)
                    .map(|i| {
                        let angle = (i as f64) * 2.0 * std::f64::consts::PI / (n.max(1) as f64);
                        let r = 3.0 + (i as f64) * 0.02;
                        [r * angle.cos(), r * angle.sin(), 0.0]
                    })
                    .collect();
                let config = PotentialFieldConfig::default();

                b.iter(|| {
                    let cmd = potential_field(&robot, &goal, &obstacles, &config);
                    black_box(cmd)
                })
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// 14. Sensor Fusion
// ---------------------------------------------------------------------------

fn bench_sensor_fusion(c: &mut Criterion) {
    let mut group = c.benchmark_group("Sensor_Fusion");

    for n_sensors in [2, 4, 8] {
        let pts_per_sensor = 1000;
        group.throughput(Throughput::Elements((n_sensors * pts_per_sensor) as u64));

        group.bench_with_input(
            BenchmarkId::new("fuse_no_downsample", n_sensors),
            &n_sensors,
            |b, &ns| {
                let clouds: Vec<PointCloud> = (0..ns)
                    .map(|s| {
                        let points: Vec<Point3D> = (0..pts_per_sensor)
                            .map(|i| {
                                Point3D::new(
                                    pseudo_random_f32(s as u64 * 100 + 1, i * 3) * 10.0,
                                    pseudo_random_f32(s as u64 * 100 + 1, i * 3 + 1) * 10.0,
                                    pseudo_random_f32(s as u64 * 100 + 1, i * 3 + 2) * 10.0,
                                )
                            })
                            .collect();
                        PointCloud::new(points, s as i64 * 100) // within 50ms window
                    })
                    .collect();
                let config = FusionConfig::default();

                b.iter(|| {
                    let merged = fuse_clouds(&clouds, &config);
                    black_box(merged.len())
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("fuse_with_voxel_downsample", n_sensors),
            &n_sensors,
            |b, &ns| {
                let clouds: Vec<PointCloud> = (0..ns)
                    .map(|s| {
                        let points: Vec<Point3D> = (0..pts_per_sensor)
                            .map(|i| {
                                Point3D::new(
                                    pseudo_random_f32(s as u64 * 200 + 1, i * 3) * 10.0,
                                    pseudo_random_f32(s as u64 * 200 + 1, i * 3 + 1) * 10.0,
                                    pseudo_random_f32(s as u64 * 200 + 1, i * 3 + 2) * 10.0,
                                )
                            })
                            .collect();
                        PointCloud::new(points, s as i64 * 100)
                    })
                    .collect();
                let config = FusionConfig {
                    voxel_size: 0.5,
                    ..Default::default()
                };

                b.iter(|| {
                    let merged = fuse_clouds(&clouds, &config);
                    black_box(merged.len())
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("fuse_with_density_weighting", n_sensors),
            &n_sensors,
            |b, &ns| {
                let clouds: Vec<PointCloud> = (0..ns)
                    .map(|s| {
                        let count = pts_per_sensor * (s + 1); // varied sizes
                        let points: Vec<Point3D> = (0..count)
                            .map(|i| {
                                Point3D::new(
                                    pseudo_random_f32(s as u64 * 300 + 1, i * 3) * 10.0,
                                    pseudo_random_f32(s as u64 * 300 + 1, i * 3 + 1) * 10.0,
                                    pseudo_random_f32(s as u64 * 300 + 1, i * 3 + 2) * 10.0,
                                )
                            })
                            .collect();
                        PointCloud::new(points, s as i64 * 100)
                    })
                    .collect();
                let config = FusionConfig {
                    density_weighting: true,
                    ..Default::default()
                };

                b.iter(|| {
                    let merged = fuse_clouds(&clouds, &config);
                    black_box(merged.len())
                })
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// 15. MCP Tool Execution
// ---------------------------------------------------------------------------

fn bench_mcp_tool_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("MCP_ToolExecution");

    // Benchmark predict_trajectory (lightweight)
    group.bench_function("predict_trajectory_10steps", |b| {
        let mut exec = ToolExecutor::new();
        let args: HashMap<String, serde_json::Value> =
            serde_json::from_value(serde_json::json!({
                "position": [0.0, 0.0, 0.0],
                "velocity": [1.0, 0.5, 0.0],
                "steps": 10,
                "dt": 0.1,
            }))
            .unwrap();
        let req = ToolRequest {
            tool_name: "predict_trajectory".to_string(),
            arguments: args,
        };

        b.iter(|| {
            let resp = exec.execute(&req);
            black_box(resp)
        })
    });

    // Benchmark spatial_search (after insertion)
    for n_points in [100, 1_000, 10_000] {
        group.throughput(Throughput::Elements(n_points as u64));

        group.bench_with_input(
            BenchmarkId::new("insert_then_search", n_points),
            &n_points,
            |b, &n| {
                let mut exec = ToolExecutor::new();
                let points = generate_point_cloud(n);
                let points_json = serde_json::to_string(&points).unwrap();
                let insert_args: HashMap<String, serde_json::Value> =
                    serde_json::from_value(serde_json::json!({
                        "points_json": points_json,
                    }))
                    .unwrap();
                let insert_req = ToolRequest {
                    tool_name: "insert_points".to_string(),
                    arguments: insert_args,
                };
                exec.execute(&insert_req);

                let search_args: HashMap<String, serde_json::Value> =
                    serde_json::from_value(serde_json::json!({
                        "query": [5.0, 5.0, 5.0],
                        "k": 10,
                    }))
                    .unwrap();
                let search_req = ToolRequest {
                    tool_name: "spatial_search".to_string(),
                    arguments: search_args,
                };

                b.iter(|| {
                    let resp = exec.execute(&search_req);
                    black_box(resp)
                })
            },
        );
    }

    // Benchmark detect_obstacles
    group.bench_function("detect_obstacles_500pts", |b| {
        let mut exec = ToolExecutor::new();
        let points = generate_point_cloud(500);
        let cloud = PointCloud::new(points, 1000);
        let cloud_json = serde_json::to_string(&cloud).unwrap();
        let args: HashMap<String, serde_json::Value> =
            serde_json::from_value(serde_json::json!({
                "point_cloud_json": cloud_json,
                "robot_position": [5.0, 5.0, 0.0],
            }))
            .unwrap();
        let req = ToolRequest {
            tool_name: "detect_obstacles".to_string(),
            arguments: args,
        };

        b.iter(|| {
            let resp = exec.execute(&req);
            black_box(resp)
        })
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Criterion groups
// ---------------------------------------------------------------------------

criterion_group!(
    benches,
    bench_point_cloud_conversion,
    bench_spatial_search,
    bench_obstacle_detection,
    bench_trajectory_prediction,
    bench_attention_computation,
    bench_behavior_tree_tick,
    bench_scene_graph_construction,
    bench_memory_recall,
    bench_full_pipeline,
    bench_swarm_task_assignment,
    bench_gaussian_splatting,
    bench_astar_planning,
    bench_potential_field_planning,
    bench_sensor_fusion,
    bench_mcp_tool_execution,
);

criterion_main!(benches);
