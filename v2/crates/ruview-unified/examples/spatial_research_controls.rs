//! Run bounded SYNTHETIC controls for the spatial research tracks.
//! This is neither a paper reproduction nor a hardware accuracy benchmark.

use ruview_unified::{
    encoder::{EncoderConfig, RfEncoder},
    eval::{PartitionDim, PartitionKey, StrictSplit},
    frame::{EvidenceLevel, FrameProvenance, ProvenanceClass, SignalQuality},
    heads::NUM_JOINTS,
    predictive::{evaluate_pose, latent_example_from_tensor, EmaTeacher, LatentPredictor},
    scene_research::{
        virtual_tx_concentration, DataRole, GeometryCandidate, GeometryOrigin, LinkObservation,
        MovingScatterer, PropagationResidual, VirtualTxCandidate,
    },
    tensor::{CalibrationMeta, LinkGeometry, RfModality, RfTensor},
    transfer_calibration::{
        AmplitudeObservation, LinkTransferCalibrator, QuietState, TransferCalibrationConfig,
    },
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().skip(1).collect::<Vec<_>>() != ["--synthetic"] {
        return Err(
            "This control requires explicit --synthetic; it does not load measured data".into(),
        );
    }
    println!("SYNTHETIC experimental controls; no hardware or paper reproduction claims");
    let provenance = FrameProvenance {
        class: ProvenanceClass::Synthetic,
        evidence: EvidenceLevel::L0Simulation,
        device_id: "synthetic-gain-control".into(),
        firmware: "fixture-v1".into(),
        receipt_id: 1,
    };
    let mut calibrator = LinkTransferCalibrator::new(
        "fixture-link",
        provenance.clone(),
        TransferCalibrationConfig {
            min_quiet_samples: 4,
            min_quiet_span_ns: 3,
            ..Default::default()
        },
    )?;
    for (i, amplitude) in [9.0, 11.0, 9.0, 11.0, 14.0].iter().enumerate() {
        let features = calibrator.observe(AmplitudeObservation {
            amplitudes: &[*amplitude; 2],
            valid_mask: &[true; 2],
            timestamp_ns: i as u64,
            provenance: &provenance,
            quality: SignalQuality {
                rssi_dbm: -40.0,
                noise_floor_dbm: -90.0,
                packet_loss: 0.0,
                interference: 0.0,
            },
            quiet: if i < 4 {
                QuietState::Quiet
            } else {
                QuietState::Occupied
            },
        })?;
        if i == 4 {
            println!(
                "SYNTHETIC calibration: {:?}; original amplitude {:?}",
                features.decision, features.amplitudes
            );
        }
    }

    let tensor = RfTensor::new(
        RfModality::Synthetic,
        2.4e9,
        20e6,
        ndarray::Array3::from_shape_fn((3, 56, 8), |(link, bin, time)| {
            num_complex::Complex64::new(
                0.1 + link as f64 * 0.1 + bin as f64 * 0.003 + time as f64 * 0.02,
                0.01,
            )
        }),
        (0..3)
            .map(|link| LinkGeometry {
                tx_pos: [link as f64, 0.0, 1.0],
                rx_pos: [link as f64, 2.0, 1.0],
            })
            .collect(),
        0.0,
        1,
        "synthetic".into(),
        1.0,
        0.0,
        CalibrationMeta::default(),
    )?;
    let encoder = RfEncoder::new(
        EncoderConfig {
            d_model: 4,
            ..Default::default()
        },
        19,
    );
    let teacher = EmaTeacher::from_student(&encoder)?;
    let example = latent_example_from_tensor(&encoder, &teacher, &tensor, &[1, 2])?;
    let mut predictor = LatentPredictor::new(4, 37)?;
    let before = predictor.loss(&example)?;
    for _ in 0..100 {
        predictor.train_step(&example, 0.05)?;
    }
    println!(
        "SYNTHETIC frozen-encoder predictor-only loss: {before:.6} -> {:.6}",
        predictor.loss(&example)?
    );

    let key = |room: &str| PartitionKey {
        room: room.into(),
        day: room.into(),
        person: room.into(),
        chipset: room.into(),
        firmware: room.into(),
        layout: room.into(),
        session: room.into(),
    };
    let keys = vec![key("fixture-train"), key("fixture-test")];
    let split = StrictSplit::holdout(&keys, PartitionDim::Room, &["fixture-test"]);
    let train = std::array::from_fn::<_, NUM_JOINTS, _>(|j| [j as f64 * 0.02, 0.0, 1.0]);
    let mut heldout = train;
    for joint in &mut heldout {
        joint[0] += 2.0;
    }
    let mut prediction = heldout;
    for joint in &mut prediction {
        joint[0] += 0.2;
    }
    let pose = evaluate_pose(
        &keys,
        &split,
        &[0],
        &[train, prediction],
        &[train, heldout],
        0,
        0.02,
    )?;
    println!("SYNTHETIC constructed pose arithmetic (not a trained prediction): model {:?}; train-mean baseline {:?}", pose.model, pose.mean_pose);

    let observation = |id: &str, role, y: f64| LinkObservation {
        capture_id: id.into(),
        role,
        tx_m: [0.0, y, 1.0],
        rx_m: [2.0, y, 1.0],
        frequency_hz: 2.4e9,
        physical_db: -60.0,
        observed_db: -57.0,
        sigma_db: 0.5,
    };
    let residual = PropagationResidual::fit(
        &[
            observation("synthetic-train-a", DataRole::Train, 0.0),
            observation("synthetic-train-b", DataRole::Train, 2.0),
        ],
        5.0,
    )?;
    let gains = residual.evaluate(
        &[observation("synthetic-holdout", DataRole::Holdout, 1.0)],
        2,
    )?;
    println!(
        "SYNTHETIC gain residual: physical MAE {:.6} dB; corrected {:.6} dB",
        gains.physical_mae_db, gains.corrected_mae_db
    );
    let completion =
        GeometryCandidate::new(GeometryOrigin::Completion, [1.0, 2.0, 0.0], 0.2, 1, None)?;
    println!(
        "SYNTHETIC completion stays separate: {:?}",
        completion.disposition(2, 10, 1.0)?
    );
    let candidates = [
        VirtualTxCandidate {
            position_m: [1.0, 0.0, 0.0],
            cost: 1.0,
        },
        VirtualTxCandidate {
            position_m: [1.1, 0.0, 0.0],
            cost: 1.01,
        },
        VirtualTxCandidate {
            position_m: [10.0, 0.0, 0.0],
            cost: 1.02,
        },
    ];
    println!(
        "SYNTHETIC competing virtual-TX modes: {:?}",
        virtual_tx_concentration(&candidates, 0.1, 0.5, 2)?
    );
    let motion = MovingScatterer::new([1.0, 1.0, 1.0], [0.1, 0.0, 0.0], 1.0)?;
    println!(
        "SYNTHETIC moving-point physics control: {:?}",
        motion.render([0.0, 0.0, 1.0], [2.0, 0.0, 1.0], 0.5, 2.4e9)?
    );
    Ok(())
}
