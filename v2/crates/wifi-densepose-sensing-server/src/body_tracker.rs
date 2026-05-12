//! Persistent body tracking on top of spatial pipeline BodyCluster data.
//!
//! Assigns stable IDs to detected bodies across frames using greedy
//! nearest-neighbor association with simplified Kalman filtering for
//! position/velocity estimation. Supports named zones with transition events.
//!
//! See ADR-094: Body Tracking Platform.

use std::collections::HashMap;
use std::time::Instant;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Input type: BodyCluster (produced by the spatial reconstruction pipeline)
// ---------------------------------------------------------------------------

/// A body-sized voxel cluster produced by the spatial reconstruction pipeline.
/// Contains the centroid, bounding box, voxel count, and a pose hint derived
/// from the vertical extent of the cluster.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BodyCluster {
    pub id: usize,
    pub centroid: [f64; 3],
    pub bbox_min: [f64; 3],
    pub bbox_max: [f64; 3],
    pub voxel_count: usize,
    /// "standing" | "sitting" | "lying" based on Z extent
    pub pose_hint: String,
}

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A tracked body with persistent ID and Kalman-filtered position.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackedBody {
    pub id: u32,
    pub centroid: [f64; 3],
    pub velocity: [f64; 3],
    pub pose_hint: String,
    pub zone: Option<String>,
    #[serde(skip)]
    pub first_seen: Option<Instant>,
    #[serde(skip)]
    pub last_seen: Option<Instant>,
    pub age_seconds: f64,
    pub track_quality: f64,
}

/// A named zone (axis-aligned box in 3D space).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Zone {
    pub name: String,
    pub min: [f64; 3],
    pub max: [f64; 3],
}

/// Result of a tracking update.
#[derive(Debug, Clone, Serialize)]
pub struct TrackingUpdate {
    pub bodies: Vec<TrackedBody>,
    pub zone_transitions: Vec<ZoneTransition>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ZoneTransition {
    pub body_id: u32,
    pub from_zone: Option<String>,
    pub to_zone: Option<String>,
}

// ---------------------------------------------------------------------------
// Internal track state
// ---------------------------------------------------------------------------

struct TrackState {
    id: u32,
    pos: [f64; 3],
    vel: [f64; 3],
    pose_hint: String,
    zone: Option<String>,
    first_seen: Instant,
    last_seen: Instant,
    quality: f64,
    matched_this_frame: bool,
}

// ---------------------------------------------------------------------------
// BodyTracker
// ---------------------------------------------------------------------------

pub struct BodyTracker {
    tracks: HashMap<u32, TrackState>,
    next_id: u32,
    zones: Vec<Zone>,
    max_association_distance: f64,
    track_timeout_secs: f64,
}

impl BodyTracker {
    pub fn new() -> Self {
        Self {
            tracks: HashMap::new(),
            next_id: 1,
            zones: Vec::new(),
            max_association_distance: 2.0,
            track_timeout_secs: 10.0,
        }
    }

    /// Run one tracking cycle: predict, associate, update, create, prune.
    pub fn update(&mut self, clusters: &[BodyCluster], dt: f64) -> TrackingUpdate {
        let now = Instant::now();

        // 1. Predict: advance each track's position by velocity * dt
        for track in self.tracks.values_mut() {
            for i in 0..3 {
                track.pos[i] += track.vel[i] * dt;
            }
            track.matched_this_frame = false;
        }

        // 2. Associate: greedy nearest-neighbor
        let mut pairs: Vec<(u32, usize, f64)> = Vec::new(); // (track_id, cluster_idx, dist)
        for track in self.tracks.values() {
            for (ci, cluster) in clusters.iter().enumerate() {
                let d = euclidean_dist(&track.pos, &cluster.centroid);
                if d <= self.max_association_distance {
                    pairs.push((track.id, ci, d));
                }
            }
        }
        pairs.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));

        let mut matched_tracks: HashMap<u32, usize> = HashMap::new();
        let mut matched_clusters: HashMap<usize, u32> = HashMap::new();
        for (tid, ci, _) in &pairs {
            if matched_tracks.contains_key(tid) || matched_clusters.contains_key(ci) {
                continue;
            }
            matched_tracks.insert(*tid, *ci);
            matched_clusters.insert(*ci, *tid);
        }

        // 3. Update matched tracks (simple Kalman)
        let alpha = 0.3;
        for (&tid, &ci) in &matched_tracks {
            if let Some(track) = self.tracks.get_mut(&tid) {
                let cluster = &clusters[ci];
                for i in 0..3 {
                    let innovation = cluster.centroid[i] - track.pos[i];
                    track.pos[i] += alpha * innovation;
                    track.vel[i] += (alpha / dt.max(0.01)) * innovation * 0.5;
                }
                track.pose_hint = cluster.pose_hint.clone();
                track.quality = 1.0;
                track.last_seen = now;
                track.matched_this_frame = true;
            }
        }

        // 4. Decay unmatched tracks
        let mut to_remove = Vec::new();
        for track in self.tracks.values_mut() {
            if !track.matched_this_frame {
                track.quality -= 0.05;
                if track.quality <= 0.0 {
                    to_remove.push(track.id);
                }
            }
        }
        for id in to_remove {
            self.tracks.remove(&id);
        }

        // 5. Create new tracks for unmatched clusters
        for (ci, cluster) in clusters.iter().enumerate() {
            if !matched_clusters.contains_key(&ci) {
                let id = self.next_id;
                self.next_id += 1;
                self.tracks.insert(id, TrackState {
                    id,
                    pos: cluster.centroid,
                    vel: [0.0; 3],
                    pose_hint: cluster.pose_hint.clone(),
                    zone: None,
                    first_seen: now,
                    last_seen: now,
                    quality: 1.0,
                    matched_this_frame: true,
                });
            }
        }

        // 6. Zone check + transitions
        let mut zone_transitions = Vec::new();
        for track in self.tracks.values_mut() {
            let new_zone = self.zones.iter().find(|z| zone_contains(z, &track.pos)).map(|z| z.name.clone());
            if new_zone != track.zone {
                zone_transitions.push(ZoneTransition {
                    body_id: track.id,
                    from_zone: track.zone.clone(),
                    to_zone: new_zone.clone(),
                });
                track.zone = new_zone;
            }
        }

        // 7. Build output
        let bodies: Vec<TrackedBody> = self.tracks.values().map(|t| TrackedBody {
            id: t.id,
            centroid: t.pos,
            velocity: t.vel,
            pose_hint: t.pose_hint.clone(),
            zone: t.zone.clone(),
            first_seen: Some(t.first_seen),
            last_seen: Some(t.last_seen),
            age_seconds: t.first_seen.elapsed().as_secs_f64(),
            track_quality: t.quality,
        }).collect();

        TrackingUpdate { bodies, zone_transitions }
    }

    pub fn get_bodies(&self) -> Vec<TrackedBody> {
        self.tracks.values().map(|t| TrackedBody {
            id: t.id,
            centroid: t.pos,
            velocity: t.vel,
            pose_hint: t.pose_hint.clone(),
            zone: t.zone.clone(),
            first_seen: Some(t.first_seen),
            last_seen: Some(t.last_seen),
            age_seconds: t.first_seen.elapsed().as_secs_f64(),
            track_quality: t.quality,
        }).collect()
    }

    pub fn get_zones(&self) -> &[Zone] {
        &self.zones
    }

    pub fn add_zone(&mut self, zone: Zone) {
        self.zones.push(zone);
    }

    pub fn remove_zone(&mut self, name: &str) {
        self.zones.retain(|z| z.name != name);
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn euclidean_dist(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

fn zone_contains(zone: &Zone, pos: &[f64; 3]) -> bool {
    (0..3).all(|i| zone.min[i] <= pos[i] && pos[i] <= zone.max[i])
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cluster(id: usize, x: f64, y: f64, z: f64) -> BodyCluster {
        BodyCluster {
            id,
            centroid: [x, y, z],
            bbox_min: [x - 0.2, y - 0.2, z],
            bbox_max: [x + 0.2, y + 0.2, z + 1.7],
            voxel_count: 10,
            pose_hint: "standing".to_string(),
        }
    }

    #[test]
    fn test_new_cluster_creates_track() {
        let mut tracker = BodyTracker::new();
        let clusters = vec![make_cluster(0, 1.0, 2.0, 0.0)];
        let result = tracker.update(&clusters, 1.0);
        assert_eq!(result.bodies.len(), 1);
        assert_eq!(result.bodies[0].id, 1);
    }

    #[test]
    fn test_persistent_id_across_frames() {
        let mut tracker = BodyTracker::new();
        let clusters = vec![make_cluster(0, 1.0, 2.0, 0.0)];
        let r1 = tracker.update(&clusters, 1.0);
        let id = r1.bodies[0].id;

        // Same position — should keep the same ID
        let clusters2 = vec![make_cluster(0, 1.05, 2.0, 0.0)];
        let r2 = tracker.update(&clusters2, 1.0);
        assert_eq!(r2.bodies.len(), 1);
        assert_eq!(r2.bodies[0].id, id);
    }

    #[test]
    fn test_track_removal_after_timeout() {
        let mut tracker = BodyTracker::new();
        let clusters = vec![make_cluster(0, 1.0, 2.0, 0.0)];
        tracker.update(&clusters, 1.0);

        // Feed empty clusters for enough frames to decay quality to 0
        // quality starts at 1.0, decays 0.05/frame → 20 frames to reach 0
        for _ in 0..20 {
            tracker.update(&[], 1.0);
        }
        let result = tracker.update(&[], 1.0);
        assert_eq!(result.bodies.len(), 0);
    }

    #[test]
    fn test_zone_containment() {
        let mut tracker = BodyTracker::new();
        tracker.add_zone(Zone {
            name: "kitchen".to_string(),
            min: [0.0, 0.0, 0.0],
            max: [3.0, 3.0, 3.0],
        });
        let clusters = vec![make_cluster(0, 1.5, 1.5, 0.5)];
        let result = tracker.update(&clusters, 1.0);
        assert_eq!(result.bodies[0].zone.as_deref(), Some("kitchen"));
    }

    #[test]
    fn test_zone_transition() {
        let mut tracker = BodyTracker::new();
        tracker.add_zone(Zone {
            name: "kitchen".to_string(),
            min: [0.0, 0.0, 0.0],
            max: [3.0, 3.0, 3.0],
        });
        tracker.add_zone(Zone {
            name: "living".to_string(),
            min: [4.0, 0.0, 0.0],
            max: [8.0, 3.0, 3.0],
        });

        // Start in kitchen
        let c1 = vec![make_cluster(0, 1.5, 1.5, 0.5)];
        let r1 = tracker.update(&c1, 1.0);
        assert_eq!(r1.zone_transitions.len(), 1);
        assert_eq!(r1.zone_transitions[0].from_zone, None);
        assert_eq!(r1.zone_transitions[0].to_zone.as_deref(), Some("kitchen"));

        // Move to living room (far enough to create new track or associate)
        let c2 = vec![make_cluster(0, 5.0, 1.5, 0.5)];
        let r2 = tracker.update(&c2, 1.0);
        // Should have a transition from kitchen to living
        let transitions: Vec<_> = r2.zone_transitions.iter()
            .filter(|zt| zt.to_zone.as_deref() == Some("living"))
            .collect();
        assert!(!transitions.is_empty());
    }
}
