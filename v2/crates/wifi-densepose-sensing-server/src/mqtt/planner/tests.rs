//! Unit tests for [`super::PublishPlanner`] and [`super::ChangeGate`].

use super::*;

const NODE: &str = "base-node1";
const WARM: Duration = Duration::from_secs(61);

fn cfg(privacy_mode: bool) -> MqttConfig {
    let args = {
        use clap::Parser;
        #[derive(Parser)]
        struct W {
            #[command(flatten)]
            m: crate::cli::MqttArgs,
        }
        W::parse_from(["t"]).m
    };
    let mut c = MqttConfig::from_args(&args);
    c.privacy_mode = privacy_mode;
    c
}

fn builder() -> DiscoveryBuilder<'static> {
    DiscoveryBuilder {
        discovery_prefix: "homeassistant",
        node_id: NODE,
        node_friendly_name: None,
        sw_version: "t",
        model: "t",
        via_device: None,
    }
}

/// The live ESP32 shape with no vitals, no fall detector, no motion energy.
fn bare(presence: bool) -> VitalsSnapshot {
    VitalsSnapshot {
        node_id: NODE.into(),
        timestamp_ms: 1_779_512_400_000,
        presence,
        motion: if presence { 1.0 } else { 0.0 },
        presence_score: if presence { 0.8 } else { 0.0 },
        n_persons: u32::from(presence),
        rssi_dbm: Some(-50.0),
        ..Default::default()
    }
}

fn full(presence: bool) -> VitalsSnapshot {
    VitalsSnapshot {
        motion_energy: Some(4.2),
        breathing_rate_bpm: Some(14.0),
        heartrate_bpm: Some(62.0),
        fall_detected: Some(false),
        vital_confidence: 0.8,
        ..bare(presence)
    }
}

fn slug(topic: &str) -> &str {
    topic.rsplit('/').nth(1).unwrap()
}

fn states(msgs: &[StateMessage]) -> Vec<(String, String)> {
    msgs.iter()
        .filter(|m| m.topic.ends_with("/state"))
        .map(|m| (slug(&m.topic).to_string(), m.payload.clone()))
        .collect()
}

fn avail(msgs: &[StateMessage]) -> HashMap<String, String> {
    msgs.iter()
        .filter(|m| m.topic.ends_with("/availability"))
        .map(|m| (slug(&m.topic).to_string(), m.payload.clone()))
        .collect()
}

// ─── Entity → state mapping ─────────────────────────────────────────

#[test]
fn announced_entities_are_exactly_the_sourced_ones() {
    let p = PublishPlanner::new(&cfg(false));
    let slugs: Vec<_> = p.entities().iter().map(|e| e.topic_slug()).collect();
    assert_eq!(
        slugs,
        [
            "presence",
            "person_count",
            "breathing_rate",
            "heart_rate",
            "motion_level",
            "motion_energy",
            "fall",
            "presence_score",
            "rssi",
            "someone_sleeping",
            "possible_distress",
            "room_active",
            "elderly_inactivity_anomaly",
            "fall_risk_elevated",
            "no_movement",
        ]
    );
}

#[test]
fn every_announced_entity_publishes_state_when_its_source_is_present() {
    let mut p = PublishPlanner::new(&cfg(false));
    let b = builder();
    let mut seen: HashMap<String, String> = HashMap::new();
    let mut last_avail = HashMap::new();
    // Drive past warmup at 1 Hz; inject one fall edge.
    for s in 0..=WARM.as_secs() {
        let mut snap = full(true);
        if s == WARM.as_secs() {
            snap.fall_detected = Some(true);
        }
        let msgs = p.on_snapshot(&b, &snap, Duration::from_secs(s));
        last_avail.extend(avail(&msgs));
        seen.extend(states(&msgs));
    }
    for e in p.entities() {
        assert!(
            seen.contains_key(e.topic_slug()),
            "{e:?} never published state"
        );
        assert_eq!(last_avail[e.topic_slug()], "online", "{e:?} not online");
    }
    assert_eq!(seen["presence"], "ON");
    assert_eq!(seen["room_active"], "ON", "present + moving → room active");
    assert_eq!(seen["someone_sleeping"], "OFF");
    assert!(seen["heart_rate"].contains("\"bpm\":62.0"));
    assert!(seen["motion_energy"].contains("\"energy\":4.2"));
    assert!(seen["fall"].contains("fall_detected"));
    assert!(seen["fall_risk_elevated"].contains("\"score\""));
}

#[test]
fn unsourced_entities_are_unavailable_and_never_publish_state() {
    let mut p = PublishPlanner::new(&cfg(false));
    let b = builder();
    let mut seen = HashMap::new();
    let mut last_avail = HashMap::new();
    for s in 0..=WARM.as_secs() {
        let msgs = p.on_snapshot(&b, &bare(true), Duration::from_secs(s));
        last_avail.extend(avail(&msgs));
        seen.extend(states(&msgs));
    }
    for slug in [
        "breathing_rate",
        "heart_rate",
        "motion_energy",
        "fall",
        "someone_sleeping",
        "possible_distress",
        "fall_risk_elevated",
    ] {
        assert_eq!(last_avail[slug], "offline", "{slug} must be unavailable");
        assert!(
            !seen.contains_key(slug),
            "{slug} published a value with no source"
        );
    }
    for slug in [
        "presence",
        "person_count",
        "rssi",
        "room_active",
        "no_movement",
    ] {
        assert_eq!(last_avail[slug], "online", "{slug}");
        assert!(seen.contains_key(slug), "{slug}");
    }
}

#[test]
fn semantic_entities_unavailable_during_warmup_then_online() {
    let mut p = PublishPlanner::new(&cfg(false));
    let b = builder();
    let first = p.on_snapshot(&b, &bare(true), Duration::ZERO);
    assert_eq!(avail(&first)["room_active"], "offline");
    assert!(!states(&first).iter().any(|(s, _)| s == "room_active"));
    let warm = p.on_snapshot(&b, &bare(true), WARM);
    assert_eq!(
        avail(&warm)["room_active"],
        "online",
        "transition published"
    );
    assert!(states(&warm).contains(&("room_active".into(), "ON".into())));
}

#[test]
fn vitals_go_unavailable_when_their_source_stops() {
    let mut p = PublishPlanner::new(&cfg(false));
    let b = builder();
    assert_eq!(
        avail(&p.on_snapshot(&b, &full(true), Duration::ZERO))["heart_rate"],
        "online"
    );
    let later = p.on_snapshot(&b, &bare(true), SOURCE_STALE_AFTER);
    assert_eq!(avail(&later)["heart_rate"], "offline");
    assert!(!states(&later).iter().any(|(s, _)| s == "heart_rate"));
}

#[test]
fn stale_node_heartbeat_reports_everything_offline() {
    let mut p = PublishPlanner::new(&cfg(false));
    let b = builder();
    p.on_snapshot(&b, &bare(true), Duration::ZERO);
    let hb = p.availability(&b, NODE, false, Duration::from_secs(40), true);
    assert_eq!(hb.len(), p.entities().len());
    assert!(hb
        .iter()
        .all(|m| m.payload == "offline" && m.retain && m.qos == 1));
    assert!(p
        .availability(&b, "unknown-node", true, Duration::ZERO, true)
        .is_empty());
}

// ─── Rate limit / heartbeat (#2093) ─────────────────────────────────

#[test]
fn presence_publishes_on_change_and_heartbeat_not_per_frame() {
    let mut p = PublishPlanner::new(&cfg(false));
    let b = builder();
    let count = |p: &mut PublishPlanner, presence: bool, ms: u64| {
        states(&p.on_snapshot(&b, &bare(presence), Duration::from_millis(ms)))
            .into_iter()
            .filter(|(s, _)| s == "presence")
            .count()
    };
    // ~73 frames/s for 10 s with no change → one publish.
    let mut n = 0;
    for i in 0..730 {
        n += count(&mut p, true, i * 1000 / 73);
    }
    assert_eq!(n, 1, "unchanged presence must not publish per frame");
    assert_eq!(
        count(&mut p, false, 10_100),
        1,
        "change publishes immediately"
    );
    assert_eq!(count(&mut p, false, 10_200), 0);
    assert_eq!(
        count(&mut p, false, 10_100 + 60_000),
        1,
        "heartbeat after 60 s"
    );
}

#[test]
fn change_gate_heartbeat_and_reset() {
    let mut g = ChangeGate::default();
    let hb = Duration::from_secs(60);
    let e = EntityKind::Presence;
    assert!(g.allow("a", e, true, Duration::ZERO, hb));
    assert!(!g.allow("a", e, true, Duration::from_secs(59), hb));
    assert!(
        g.allow("b", e, true, Duration::from_secs(59), hb),
        "per node"
    );
    assert!(
        g.allow("a", e, false, Duration::from_secs(59), hb),
        "change"
    );
    assert!(
        g.allow("a", e, false, Duration::from_secs(119), hb),
        "heartbeat"
    );
    g.reset();
    assert!(g.allow("a", e, false, Duration::from_secs(120), hb));
}

#[test]
fn numeric_sensors_keep_their_configured_rate() {
    let mut p = PublishPlanner::new(&cfg(false));
    let b = builder();
    let mut rssi = 0;
    for i in 0..730u64 {
        rssi += states(&p.on_snapshot(&b, &bare(true), Duration::from_millis(i * 1000 / 73)))
            .iter()
            .filter(|(s, _)| s == "rssi")
            .count();
    }
    assert_eq!(rssi, 1, "rssi at 0.1 Hz → one sample in 10 s");
}

#[test]
fn reset_republishes_state_and_availability() {
    let mut p = PublishPlanner::new(&cfg(false));
    let b = builder();
    p.on_snapshot(&b, &bare(true), Duration::ZERO);
    let quiet = p.on_snapshot(&b, &bare(true), Duration::from_millis(10));
    assert!(avail(&quiet).is_empty() && !states(&quiet).iter().any(|(s, _)| s == "presence"));
    p.reset();
    let again = p.on_snapshot(&b, &bare(true), Duration::from_millis(20));
    assert_eq!(avail(&again)["presence"], "online");
    assert!(states(&again).iter().any(|(s, _)| s == "presence"));
}

// ─── Privacy mode ───────────────────────────────────────────────────

#[test]
fn privacy_mode_never_announces_or_publishes_biometrics() {
    let mut p = PublishPlanner::new(&cfg(true));
    assert!(p.entities().iter().all(|e| !e.is_biometric()));
    let b = builder();
    for s in 0..=WARM.as_secs() {
        let msgs = p.on_snapshot(&b, &full(true), Duration::from_secs(s));
        for m in &msgs {
            for slug in ["heart_rate", "breathing_rate", "pose"] {
                assert!(
                    !m.topic.contains(&format!("/{slug}/")),
                    "{slug} leaked: {}",
                    m.topic
                );
            }
        }
    }
    // Inferred states built on presence and motion stay (ADR-115 §3.12.3);
    // states inferred from vitals are suppressed like the vitals (#2165).
    let msgs = p.on_snapshot(&b, &full(true), WARM + Duration::from_secs(1));
    let all: Vec<_> = p.availability(&b, NODE, true, WARM, true);
    assert_eq!(avail(&all)["room_active"], "online");
    assert!(!avail(&all).contains_key("someone_sleeping"));
    assert!(!avail(&all).contains_key("possible_distress"));
    assert!(msgs.iter().all(|m| !m.payload.contains("bpm")));
    assert!(msgs
        .iter()
        .all(|m| !m.topic.contains("/someone_sleeping/") && !m.topic.contains("/possible_distress/")));
}

// ─── Semantic wiring ────────────────────────────────────────────────

#[test]
fn no_movement_turns_on_after_its_dwell() {
    let mut p = PublishPlanner::new(&cfg(false));
    let b = builder();
    let still = VitalsSnapshot {
        motion: 0.0,
        ..bare(true)
    };
    let mut first_off = None;
    let mut first_on = None;
    for s in 0..WARM.as_secs() + 30 * 60 + 2 {
        for (slug, v) in states(&p.on_snapshot(&b, &still, Duration::from_secs(s))) {
            match (slug.as_str(), v.as_str()) {
                ("no_movement", "OFF") => _ = first_off.get_or_insert(s),
                ("no_movement", "ON") => _ = first_on.get_or_insert(s),
                _ => {}
            }
        }
    }
    // OFF once warmup (60 s) ends, ON after the 30 min stillness dwell.
    assert_eq!(first_off, Some(60));
    assert_eq!(first_on, Some(60 + 30 * 60));
}

#[test]
fn semantic_bus_samples_at_one_hz_but_keeps_fall_edges() {
    let mut p = PublishPlanner::new(&cfg(false));
    let b = builder();
    // Warm up with the fall detector online.
    for s in 0..=WARM.as_secs() {
        p.on_snapshot(&b, &full(true), Duration::from_secs(s));
    }
    let base = p.nodes[NODE].bus.fall_risk_score();
    // A fall edge between two 1 Hz ticks must still reach the FSM.
    let t = WARM + Duration::from_millis(500);
    p.on_snapshot(
        &b,
        &VitalsSnapshot {
            fall_detected: Some(true),
            ..full(true)
        },
        t,
    );
    p.on_snapshot(&b, &full(true), WARM + SEMANTIC_TICK);
    assert!(p.nodes[NODE].bus.fall_risk_score() >= base + 10.0);
}
