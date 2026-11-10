//! Multi-BSSID WiFi RSSI scanning and sensing task.

use std::time::Duration;
use tracing::{debug, error, info, warn};
use wifi_densepose_hardware::{Adr018Flags, PpduType};
use wifi_densepose_sensing_server::dsp::features::extract_features_from_frame;
use wifi_densepose_sensing_server::dsp::signal_field::generate_signal_field;
use wifi_densepose_sensing_server::ingest::Esp32Frame;
use wifi_densepose_sensing_server::pose::derive_pose_from_sensing;
use wifi_densepose_wifiscan::{BssidRegistry, WindowsWifiPipeline};

#[cfg(not(target_os = "macos"))]
use wifi_densepose_wifiscan::parse_netsh_output as parse_netsh_bssid_output;

use crate::{
    assess_legacy_image_pose, attach_field_positions, compute_person_score,
    observe_sensing_update, smooth_and_classify, adaptive_override, smooth_vitals,
    tracker_bridge, NodeInfo, SensingUpdate, SharedState,
    FRAME_HISTORY_CAPACITY,
};

#[cfg(not(target_os = "macos"))]
use super::fallback::windows_wifi_fallback_tick;

/// Continuous multi-BSSID WiFi RSSI collector task.
pub async fn wifi_task(state: SharedState, tick_ms: u64) {
    let mut interval = tokio::time::interval(Duration::from_millis(tick_ms));
    let mut seq: u32 = 0;

    // ADR-022 Phase 3: Multi-BSSID pipeline state (kept across ticks)
    let mut registry = BssidRegistry::new(32, 30);
    let mut pipeline = WindowsWifiPipeline::new();
    let mut reported_missing_helper = false;

    info!(
        "WiFi RSSI pipeline active (platform={}, tick={}ms, max_bssids=32)",
        std::env::consts::OS,
        tick_ms
    );

    loop {
        interval.tick().await;
        seq += 1;

        // In `auto`, a live ESP32 stream outranks host WiFi: once the UDP
        // receiver promotes the source, stop overwriting it (resumes if the
        // nodes age out to esp32:offline), mirroring the simulator.
        if state.read().await.effective_source() == "esp32" {
            continue;
        }

        // ── Step 1: Run multi-BSSID scan via spawn_blocking ──────────
        #[cfg(target_os = "macos")]
        let bssid_scan_result = tokio::task::spawn_blocking(|| {
            wifi_densepose_wifiscan::adapter::MacosCoreWlanScanner::new()
                .scan_sync()
                .map_err(|e| e.to_string())
        })
        .await;

        #[cfg(not(target_os = "macos"))]
        let bssid_scan_result = tokio::task::spawn_blocking(|| {
            let output = std::process::Command::new("netsh")
                .args(["wlan", "show", "networks", "mode=bssid"])
                .output()
                .map_err(|e| format!("netsh bssid scan failed: {e}"))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(format!(
                    "netsh exited with {}: {}",
                    output.status,
                    stderr.trim()
                ));
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            parse_netsh_bssid_output(&stdout).map_err(|e| format!("parse error: {e}"))
        })
        .await;

        let observations = match bssid_scan_result {
            Ok(Ok(obs)) if !obs.is_empty() => obs,
            Ok(Ok(_empty)) => {
                debug!("WiFi scan returned 0 observations");
                #[cfg(not(target_os = "macos"))]
                windows_wifi_fallback_tick(&state, seq).await;
                continue;
            }
            Ok(Err(e)) => {
                if cfg!(target_os = "macos") && e.contains("failed to run mac_wifi helper") {
                    if !reported_missing_helper {
                        error!(
                            "--source wifi on macOS needs the mac_wifi helper on PATH: \
                             swiftc -O archive/v1/src/sensing/mac_wifi.swift -o <dir-on-PATH>/mac_wifi ({e})"
                        );
                        reported_missing_helper = true;
                    }
                } else {
                    warn!("WiFi scan error: {e}");
                }
                #[cfg(not(target_os = "macos"))]
                windows_wifi_fallback_tick(&state, seq).await;
                continue;
            }
            Err(join_err) => {
                error!("spawn_blocking panicked: {join_err}");
                continue;
            }
        };

        let obs_count = observations.len();

        let ssid = observations
            .first()
            .map(|o| o.ssid.clone())
            .filter(|ssid| !ssid.trim().is_empty())
            .unwrap_or_else(|| "unnamed".into());

        // ── Step 2: Feed observations into registry ──────────────────
        registry.update(&observations);
        let multi_ap_frame = registry.to_multi_ap_frame();

        // ── Step 3: Run enhanced pipeline ────────────────────────────
        let enhanced = pipeline.process(&multi_ap_frame);

        // ── Step 4: Build backward-compatible Esp32Frame ─────────────
        let first_rssi = observations.first().map(|o| o.rssi_dbm).unwrap_or(-80.0);
        let _first_signal_pct = observations.first().map(|o| o.signal_pct).unwrap_or(40.0);

        let frame = Esp32Frame {
            magic: 0xC511_0001,
            node_id: 0,
            n_antennas: 1,
            n_subcarriers: obs_count.min(u16::MAX as usize) as u16,
            freq_mhz: 2437,
            sequence: seq,
            rssi: first_rssi.clamp(-128.0, 127.0) as i8,
            noise_floor: -90,
            ppdu_type: PpduType::HtLegacy,
            adr018_flags: Adr018Flags::default(),
            amplitudes: multi_ap_frame.amplitudes.clone(),
            phases: multi_ap_frame.phases.clone(),
        };

        // ── Step 4b: Update frame history and extract features ───────
        let mut s_write_pre = state.write().await;
        s_write_pre
            .frame_history
            .push_back(frame.amplitudes.clone());
        if s_write_pre.frame_history.len() > FRAME_HISTORY_CAPACITY {
            s_write_pre.frame_history.pop_front();
        }
        let sample_rate_hz = 1000.0 / tick_ms as f64;
        let (features, mut classification, breathing_rate_hz, sub_variances, raw_motion) =
            extract_features_from_frame(&frame, &s_write_pre.frame_history, sample_rate_hz);
        smooth_and_classify(&mut s_write_pre, &mut classification, raw_motion);
        adaptive_override(&s_write_pre, &features, &mut classification);
        drop(s_write_pre);

        // ── Step 5: Build enhanced fields from pipeline result ───────
        let enhanced_motion = Some(serde_json::json!({
            "score": enhanced.motion.score,
            "level": format!("{:?}", enhanced.motion.level),
            "contributing_bssids": enhanced.motion.contributing_bssids,
        }));

        let enhanced_breathing = enhanced.breathing.as_ref().map(|b| {
            serde_json::json!({
                "rate_bpm": b.rate_bpm,
                "confidence": b.confidence,
                "bssid_count": b.bssid_count,
            })
        });

        let posture_str = enhanced.posture.map(|p| format!("{p:?}"));
        let sig_quality_score = Some(enhanced.signal_quality.score);
        let verdict_str = Some(format!("{:?}", enhanced.verdict));
        let bssid_n = Some(enhanced.bssid_count);

        // ── Step 6: Update shared state ──────────────────────────────
        let mut s = state.write().await;
        s.source = format!("wifi:{ssid}");
        s.rssi_history.push_back(first_rssi);
        if s.rssi_history.len() > 60 {
            s.rssi_history.pop_front();
        }

        let tick = s.advance_tick();

        let motion_score = if classification.motion_level == "active" {
            0.8
        } else if classification.motion_level == "present_still" {
            0.3
        } else {
            0.05
        };

        let raw_vitals = s
            .vital_detector
            .process_frame(&frame.amplitudes, &frame.phases);
        let vitals = smooth_vitals(&mut s, &raw_vitals);
        s.latest_vitals = vitals.clone();

        let feat_variance = features.variance;

        s.p95_variance.push(features.variance);
        s.p95_motion_band_power.push(features.motion_band_power);
        s.p95_spectral_power.push(features.spectral_power);

        let raw_score = compute_person_score(&s, &features);
        s.smoothed_person_score = s.smoothed_person_score * 0.90 + raw_score * 0.10;
        let est_persons = if classification.presence {
            let count = s.person_count();
            s.prev_person_count = count;
            count
        } else {
            s.prev_person_count = 0;
            0
        };

        let mut update = SensingUpdate {
            msg_type: "sensing_update".to_string(),
            timestamp: chrono::Utc::now().timestamp_millis() as f64 / 1000.0,
            source: format!("wifi:{ssid}"),
            tick,
            nodes: vec![NodeInfo {
                node_id: 0,
                device_id: None,
                rssi_dbm: first_rssi,
                position: [0.0, 0.0, 0.0],
                amplitude: multi_ap_frame.amplitudes,
                subcarrier_count: obs_count,
                sync: None,
                node_inference: None,
                mediatek_diagnostics: None,
            }],
            features,
            classification,
            signal_field: generate_signal_field(
                first_rssi,
                motion_score,
                breathing_rate_hz,
                feat_variance.min(1.0),
                &sub_variances,
            ),
            vital_signs: None,
            calibrated_presence_evidence: None,
            detector_diagnostics: None,
            enhanced_motion,
            enhanced_breathing,
            posture: posture_str,
            signal_quality_score: sig_quality_score,
            quality_verdict: verdict_str,
            bssid_count: bssid_n,
            pose_keypoints: None,
            model_status: None,
            persons: None,
            estimated_persons: if est_persons > 0 {
                Some(est_persons)
            } else {
                None
            },
            node_features: None,
            room_inference: None,
            classifier: None,
            activity: None,
        };

        let raw_persons = derive_pose_from_sensing(&update);
        let mut last_tracker_instant = s.last_tracker_instant.take();
        let tracked = tracker_bridge::tracker_update(
            &mut s.pose_tracker,
            &mut last_tracker_instant,
            raw_persons,
        );
        s.last_tracker_instant = last_tracker_instant;
        if !tracked.is_empty() {
            update.persons = Some(tracked);
        }
        // #1050: attach real signal_field-peak positions to each person.
        attach_field_positions(&mut update);
        assess_legacy_image_pose(&mut s, &update);

        if let Ok(json) = serde_json::to_string(&update) {
            let _ = s.tx.send(json);
        }
        observe_sensing_update(s.latest_update.as_ref(), &update);
        s.latest_update = Some(update);

        debug!(
            "Multi-BSSID tick #{tick}: {obs_count} BSSIDs, quality={:.2}, verdict={:?}",
            enhanced.signal_quality.score, enhanced.verdict
        );
    }
}
