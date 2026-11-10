//! Legacy image pose assessment.
//!
//! Feeds legacy renderer coordinates into the canonical audit boundary without
//! promoting them to metric 3D or calibrated evidence.
//!
//! Complies with ELMAS.md: <= 350 lines per file.

use wifi_densepose_core::{
    CalibrationId, Coco17Joint, JointObservation, JointVisibility, ModelRef,
    PoseDimensionality, PoseObservationV2, PoseTrustState, Probability, SourceProvenance,
    SpatialFrameRef, SymmetricCovariance3, TrackId,
};
use wifi_densepose_sensing_server::domain::SensingUpdate;

use super::types::AppStateInner;

impl AppStateInner {
    /// Feed legacy renderer coordinates into the canonical audit boundary without
    /// promoting them to metric 3D or calibrated evidence.
    pub fn assess_legacy_image_pose(&mut self, update: &SensingUpdate) {
        if !update.timestamp.is_finite() || update.timestamp < 0.0 {
            return;
        }
        let timestamp_ns = (update.timestamp * 1_000_000_000.0) as u64;
        let Some(persons) = update.persons.as_ref() else {
            return;
        };
        for person in persons {
            if person.keypoints.len() != Coco17Joint::ALL.len() || !person.confidence.is_finite() {
                continue;
            }
            let Some(joints) = person
                .keypoints
                .iter()
                .zip(Coco17Joint::ALL)
                .map(|(keypoint, kind)| {
                    let position = [keypoint.x as f32, keypoint.y as f32, 0.0];
                    if !position.iter().all(|coordinate| coordinate.is_finite())
                        || !keypoint.confidence.is_finite()
                    {
                        return None;
                    }
                    let confidence =
                        Probability::new((keypoint.confidence as f32).clamp(0.0, 1.0)).ok()?;
                    Some(JointObservation {
                        kind,
                        position_m: position,
                        covariance_m2: SymmetricCovariance3 {
                            xx: 0.0,
                            xy: 0.0,
                            xz: 0.0,
                            yy: 0.0,
                            yz: 0.0,
                            zz: 0.0,
                        },
                        confidence,
                        visibility: if confidence.get() > 0.0 {
                            JointVisibility::Visible
                        } else {
                            JointVisibility::Unknown
                        },
                    })
                })
                .collect::<Option<Vec<_>>>()
                .and_then(|joints| joints.try_into().ok())
            else {
                continue;
            };
            let observer_confidence =
                match Probability::new((person.confidence as f32).clamp(0.0, 1.0)) {
                    Ok(confidence) => confidence,
                    Err(_) => continue,
                };
            let mut raw = PoseObservationV2 {
                schema_version: 2,
                timestamp_ns,
                sensor_epoch: 1,
                sequence: update.tick,
                track_id: TrackId(format!("local:{}", person.id)),
                frame: SpatialFrameRef {
                    name: "legacy-renderer-image".into(),
                    version: 1,
                    metric: false,
                    right_handed: false,
                    z_up: false,
                },
                calibration_id: CalibrationId("uncalibrated-image".into()),
                floor_plane: None,
                model: ModelRef {
                    id: if self.model_loaded {
                        "sensing-server-model-artifact-unknown".into()
                    } else {
                        "sensing-server-signal-derived".into()
                    },
                    artifact_hash: [0; 32],
                },
                source: SourceProvenance {
                    sensor_id: "sensing-server-local".into(),
                    authenticated: false,
                    replay_protected: false,
                },
                trust_state: PoseTrustState::Degraded,
                dimensionality: PoseDimensionality::Image2d,
                uncertainty_calibrated: false,
                joints,
                observer_confidence,
                canonical_hash: [0; 32],
            };
            raw.seal();
            let _ = self.pose_physics.process(&raw, timestamp_ns);
        }
    }
}

pub fn assess_legacy_image_pose(state: &mut AppStateInner, update: &SensingUpdate) {
    state.assess_legacy_image_pose(update);
}

