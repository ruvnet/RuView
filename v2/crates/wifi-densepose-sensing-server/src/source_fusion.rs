//! Any-source readings and late per-zone presence fusion (ADR-424 D1-D5).
//!
//! A *source reading* is one JSON line from any sensor: a CSI node's
//! classification, a radar, or a third-party program that prints readings.
//! This module validates readings at the boundary, maps them to a presence
//! probability through a per-kind descriptor, and fuses the healthy, fresh
//! sources of a zone in log-odds space.
//!
//! Rules it enforces (ADR-424 D4):
//! - a stale or unhealthy source stops contributing and lowers confidence; it
//!   is never read as "empty";
//! - a fused result that used any synthetic reading is labelled SYNTHETIC;
//! - weights are placeholders until a benchmark supplies them, and the output
//!   says so (`weights_measured`).
//!
//! Pure logic: no I/O, no clock. Callers pass `now_ms`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Largest accepted reading line, bytes.
pub const MAX_LINE_BYTES: usize = 4096;
/// Largest number of entries in `values`.
pub const MAX_VALUES: usize = 32;
/// Longest accepted id / name (source, kind, room, zone, value name).
pub const MAX_NAME_LEN: usize = 64;
/// A source with no reading for this long stops contributing.
pub const STALE_AFTER_MS: i64 = 10_000;
/// Readings stamped further in the future than this are rejected.
pub const MAX_FUTURE_SKEW_MS: i64 = 5_000;
/// Weight used for a source kind with no measured reliability.
pub const DEFAULT_WEIGHT: f64 = 0.5;
/// Probabilities are clamped to this distance from 0 and 1 before log-odds.
const P_EPS: f64 = 0.02;

/// Why a line was rejected. Rejected lines are counted, never partially applied.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReadingError {
    #[error("line exceeds {MAX_LINE_BYTES} bytes")]
    TooLong,
    #[error("invalid JSON: {0}")]
    Json(String),
    #[error("invalid {0}")]
    BadName(&'static str),
    #[error("too many values")]
    TooManyValues,
    #[error("non-finite value for {0}")]
    NonFinite(String),
    #[error("quality outside [0, 1]")]
    BadQuality,
    #[error("timestamp out of range")]
    BadTimestamp,
}

/// Source-reported health.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Health {
    #[default]
    Ok,
    Stale,
    NoSource,
    Degraded,
}

/// One validated reading (ADR-424 D1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceReading {
    pub timestamp_ms: i64,
    pub source: String,
    pub kind: String,
    #[serde(default)]
    pub room: Option<String>,
    #[serde(default)]
    pub zone: Option<String>,
    pub values: BTreeMap<String, f64>,
    #[serde(default)]
    pub quality: Option<f64>,
    #[serde(default)]
    pub health: Health,
    #[serde(default)]
    pub synthetic: bool,
    /// True only for readings the server produced itself from an
    /// authenticated or in-process source. Never read from the wire, so a
    /// sender cannot assert it: everything posted to the ingest route is a
    /// claim until the transport authenticates the sender.
    #[serde(skip)]
    pub verified: bool,
}

fn valid_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= MAX_NAME_LEN
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
}

/// Parse and validate one JSON line. `now_ms` bounds future timestamps.
pub fn parse_reading(line: &str, now_ms: i64) -> Result<SourceReading, ReadingError> {
    if line.len() > MAX_LINE_BYTES {
        return Err(ReadingError::TooLong);
    }
    let r: SourceReading =
        serde_json::from_str(line).map_err(|e| ReadingError::Json(e.to_string()))?;
    if !valid_name(&r.source) {
        return Err(ReadingError::BadName("source"));
    }
    if !valid_name(&r.kind) {
        return Err(ReadingError::BadName("kind"));
    }
    if r.room.as_deref().is_some_and(|s| !valid_name(s)) {
        return Err(ReadingError::BadName("room"));
    }
    if r.zone.as_deref().is_some_and(|s| !valid_name(s)) {
        return Err(ReadingError::BadName("zone"));
    }
    if r.values.len() > MAX_VALUES {
        return Err(ReadingError::TooManyValues);
    }
    for (k, v) in &r.values {
        if !valid_name(k) {
            return Err(ReadingError::BadName("value name"));
        }
        if !v.is_finite() {
            return Err(ReadingError::NonFinite(k.clone()));
        }
    }
    if r.quality.is_some_and(|q| !(0.0..=1.0).contains(&q)) {
        return Err(ReadingError::BadQuality);
    }
    if r.timestamp_ms <= 0 || r.timestamp_ms > now_ms + MAX_FUTURE_SKEW_MS {
        return Err(ReadingError::BadTimestamp);
    }
    Ok(r)
}

/// How a kind's values become a presence probability (ADR-424 D2).
#[derive(Debug, Clone, PartialEq)]
pub struct PresenceDescriptor {
    pub kind: &'static str,
    /// Value holding a probability in `[0, 1]`, if the source reports one.
    pub probability_value: Option<&'static str>,
    /// Value holding a 0/1 flag, used when there is no probability.
    pub flag_value: Option<&'static str>,
    /// Probability assigned to flag = 1 (and `1 - p` to flag = 0) at quality 1.
    pub flag_confidence: f64,
    /// Fusion weight. Placeholder until a benchmark measures it.
    pub weight: f64,
    pub weight_measured: bool,
}

/// Built-in descriptors. Kinds without one are context only (tier 2).
pub fn builtin_descriptors() -> Vec<PresenceDescriptor> {
    vec![
        PresenceDescriptor {
            kind: "wifi_csi",
            probability_value: Some("presence_probability"),
            flag_value: Some("presence"),
            flag_confidence: 0.75,
            weight: DEFAULT_WEIGHT,
            weight_measured: false,
        },
        PresenceDescriptor {
            kind: "mmwave_radar",
            probability_value: None,
            flag_value: Some("presence"),
            flag_confidence: 0.9,
            weight: DEFAULT_WEIGHT,
            weight_measured: false,
        },
    ]
}

/// Presence probability one reading asserts, or `None` if it carries no
/// usable presence evidence (out-of-range values are not evidence).
pub fn presence_probability(r: &SourceReading, d: &PresenceDescriptor) -> Option<f64> {
    if let Some(p) = d.probability_value.and_then(|n| r.values.get(n)) {
        return (0.0..=1.0).contains(p).then_some(*p);
    }
    let flag = *d.flag_value.and_then(|n| r.values.get(n))?;
    if flag != 0.0 && flag != 1.0 {
        return None;
    }
    // A low-quality flag is pulled toward 0.5 (no information).
    let conf = 0.5 + (d.flag_confidence - 0.5) * r.quality.unwrap_or(1.0);
    Some(if flag == 1.0 { conf } else { 1.0 - conf })
}

/// A source's part in a zone's fused result.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SourceContribution {
    pub source: String,
    pub kind: String,
    pub age_ms: i64,
    pub health: Health,
    /// `None` when the source did not contribute (stale, unhealthy, no evidence).
    pub probability: Option<f64>,
    pub weight: f64,
    pub contributing: bool,
    pub synthetic: bool,
    pub verified: bool,
}

/// Fused presence for one zone (ADR-424 D5).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ZonePresence {
    pub room: Option<String>,
    pub zone: String,
    /// `None` when no healthy, fresh tier-1 source covers the zone.
    pub presence_probability: Option<f64>,
    /// Share of the zone's known tier-1 weight that is contributing, `[0, 1]`.
    pub confidence: f64,
    /// `Some(false)` when contributing sources fall on both sides of 0.5.
    pub agreement: Option<bool>,
    /// `MEASURED` only when every contributing source is verified hardware;
    /// `CLAIMED` when any came from an unauthenticated sender; `SYNTHETIC`
    /// when any is simulated or replayed; `NONE` with no live source.
    pub evidence: &'static str,
    /// False while any contributing weight is a placeholder.
    pub weights_measured: bool,
    pub sources: Vec<SourceContribution>,
}

/// Evidence grade of a fused zone. Synthetic input dominates; otherwise any
/// contributing reading from an unauthenticated sender makes it a claim.
/// With no live source there is no measurement to grade.
fn evidence_grade(live: bool, any_synthetic: bool, any_unverified: bool) -> &'static str {
    match (live, any_synthetic, any_unverified) {
        (false, _, _) => "NONE",
        (true, true, _) => "SYNTHETIC",
        (true, false, true) => "CLAIMED",
        (true, false, false) => "MEASURED",
    }
}

fn logit(p: f64) -> f64 {
    let p = p.clamp(P_EPS, 1.0 - P_EPS);
    (p / (1.0 - p)).ln()
}

/// Latest reading per source, fused per zone.
#[derive(Debug, Default)]
pub struct SourceFusion {
    latest: BTreeMap<String, SourceReading>,
    descriptors: Vec<PresenceDescriptor>,
    rejected: u64,
}

impl SourceFusion {
    pub fn new(descriptors: Vec<PresenceDescriptor>) -> Self {
        Self { latest: BTreeMap::new(), descriptors, rejected: 0 }
    }

    /// Lines rejected at the boundary so far.
    pub fn rejected(&self) -> u64 {
        self.rejected
    }

    /// Ingest one raw line. A rejected line changes nothing but the counter.
    pub fn ingest_line(&mut self, line: &str, now_ms: i64) -> Result<(), ReadingError> {
        match parse_reading(line, now_ms) {
            Ok(r) => {
                self.ingest(r);
                Ok(())
            }
            Err(e) => {
                self.rejected += 1;
                Err(e)
            }
        }
    }

    /// Keep the newest reading per source; an older one never replaces it.
    pub fn ingest(&mut self, r: SourceReading) {
        match self.latest.get(&r.source) {
            Some(prev) if prev.timestamp_ms > r.timestamp_ms => {}
            _ => {
                self.latest.insert(r.source.clone(), r);
            }
        }
    }

    fn descriptor(&self, kind: &str) -> Option<&PresenceDescriptor> {
        self.descriptors.iter().find(|d| d.kind == kind)
    }

    /// Fused presence for every zone that has at least one tier-1 source.
    pub fn zones(&self, now_ms: i64) -> Vec<ZonePresence> {
        let mut by_zone: BTreeMap<(Option<String>, String), Vec<&SourceReading>> = BTreeMap::new();
        for r in self.latest.values() {
            if let (Some(zone), Some(_)) = (&r.zone, self.descriptor(&r.kind)) {
                by_zone.entry((r.room.clone(), zone.clone())).or_default().push(r);
            }
        }
        by_zone
            .into_iter()
            .map(|((room, zone), readings)| self.fuse_zone(room, zone, &readings, now_ms))
            .collect()
    }

    fn fuse_zone(
        &self,
        room: Option<String>,
        zone: String,
        readings: &[&SourceReading],
        now_ms: i64,
    ) -> ZonePresence {
        let mut sources = Vec::with_capacity(readings.len());
        let (mut num, mut w_live, mut w_all) = (0.0, 0.0, 0.0);
        let (mut any_synth, mut measured, mut above, mut below) = (false, true, false, false);
        let mut any_unverified = false;
        for r in readings {
            let Some(d) = self.descriptor(&r.kind) else { continue };
            let age_ms = (now_ms - r.timestamp_ms).max(0);
            let fresh = age_ms <= STALE_AFTER_MS && r.health == Health::Ok;
            let p = if fresh { presence_probability(r, d) } else { None };
            w_all += d.weight;
            if let Some(p) = p {
                // Linear age discount: a reading at the stale limit counts for nothing.
                let w = d.weight * (1.0 - age_ms as f64 / STALE_AFTER_MS as f64);
                num += w * logit(p);
                w_live += w;
                any_synth |= r.synthetic;
                any_unverified |= !r.verified;
                measured &= d.weight_measured;
                above |= p > 0.5;
                below |= p < 0.5;
            }
            sources.push(SourceContribution {
                source: r.source.clone(),
                kind: r.kind.clone(),
                age_ms,
                health: r.health,
                probability: p,
                weight: d.weight,
                contributing: p.is_some(),
                synthetic: r.synthetic,
                verified: r.verified,
            });
        }
        let live = w_live > 0.0;
        ZonePresence {
            room,
            zone,
            presence_probability: live.then(|| 1.0 / (1.0 + (-num / w_live).exp())),
            confidence: if w_all > 0.0 { (w_live / w_all).clamp(0.0, 1.0) } else { 0.0 },
            agreement: live.then_some(!(above && below)),
            evidence: evidence_grade(live, any_synth, any_unverified),
            weights_measured: live && measured,
            sources,
        }
    }
}

/// Most lines accepted in one ingest request.
pub const MAX_LINES_PER_REQUEST: usize = 256;
/// Environment variable mapping CSI node ids to zones:
/// `1=office/desk,2=office/sofa` (`room/zone`, or just `zone`).
pub const NODE_ZONES_ENV: &str = "RUVIEW_NODE_ZONES";

/// Hardware CSI sources as named by the server's `effective_source()`.
const HARDWARE_SOURCE_PREFIXES: [&str; 4] = ["esp32", "realtek", "mediatek", "qualcomm"];

/// Outcome of one ingest request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IngestReport {
    pub accepted: usize,
    pub rejected: usize,
    /// First few rejection reasons, `line N: reason`.
    pub errors: Vec<String>,
}

/// Parse `1=office/desk,2=sofa`. Malformed entries are skipped, not guessed.
pub fn parse_node_zones(spec: &str) -> BTreeMap<u8, (Option<String>, String)> {
    let mut out = BTreeMap::new();
    for item in spec.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let Some((node, place)) = item.split_once('=') else { continue };
        let Ok(node) = node.trim().parse::<u8>() else { continue };
        let (room, zone) = match place.trim().split_once('/') {
            Some((r, z)) => (Some(r), z),
            None => (None, place.trim()),
        };
        if valid_name(zone) && room.map_or(true, valid_name) {
            out.insert(node, (room.map(str::to_string), zone.to_string()));
        }
    }
    out
}

/// True for a live hardware CSI source; false for simulated, test, host-WiFi
/// and `*:offline` sources.
pub fn is_hardware_source(source: &str) -> bool {
    !source.ends_with(":offline")
        && HARDWARE_SOURCE_PREFIXES.iter().any(|p| source.starts_with(p))
}

/// Server-side fusion state: external readings plus the CSI nodes' own
/// classifications, mapped to zones by configuration.
#[derive(Debug)]
pub struct FusionState {
    fusion: SourceFusion,
    node_zones: BTreeMap<u8, (Option<String>, String)>,
}

impl Default for FusionState {
    fn default() -> Self {
        Self::new(BTreeMap::new())
    }
}

impl FusionState {
    pub fn new(node_zones: BTreeMap<u8, (Option<String>, String)>) -> Self {
        Self { fusion: SourceFusion::new(builtin_descriptors()), node_zones }
    }

    pub fn from_env() -> Self {
        Self::new(std::env::var(NODE_ZONES_ENV).map(|s| parse_node_zones(&s)).unwrap_or_default())
    }

    /// Ingest newline-separated readings. A bad line is rejected on its own;
    /// the request fails only when it carries too many lines.
    pub fn ingest_lines(&mut self, text: &str, now_ms: i64) -> Result<IngestReport, String> {
        let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
        if lines.len() > MAX_LINES_PER_REQUEST {
            return Err(format!("more than {MAX_LINES_PER_REQUEST} lines in one request"));
        }
        let mut report = IngestReport { accepted: 0, rejected: 0, errors: Vec::new() };
        for (i, line) in lines.iter().enumerate() {
            match self.fusion.ingest_line(line, now_ms) {
                Ok(()) => report.accepted += 1,
                Err(e) => {
                    report.rejected += 1;
                    if report.errors.len() < 5 {
                        report.errors.push(format!("line {}: {e}", i + 1));
                    }
                }
            }
        }
        Ok(report)
    }

    /// Turn each zone-mapped CSI node's classification into a `wifi_csi`
    /// reading. The node's confidence is used as the reading's quality, so an
    /// unsure classification is pulled toward "no information" whatever the
    /// confidence was meant to describe. A stale node entry or an offline
    /// source reports `health: stale`, which stops it contributing.
    pub fn refresh_csi(&mut self, update: Option<&crate::SensingUpdate>, source: &str) {
        let Some(update) = update else { return };
        let Some(nodes) = update.node_features.as_ref() else { return };
        let timestamp_ms = (update.timestamp * 1000.0).round() as i64;
        if timestamp_ms <= 0 {
            return;
        }
        let offline = source.ends_with(":offline");
        for nf in nodes {
            let Some((room, zone)) = self.node_zones.get(&nf.node_id) else { continue };
            let conf = nf.classification.confidence;
            let mut values = BTreeMap::new();
            values.insert("presence".to_string(), if nf.classification.presence { 1.0 } else { 0.0 });
            self.fusion.ingest(SourceReading {
                timestamp_ms,
                source: format!("csi-node-{}", nf.node_id),
                kind: "wifi_csi".to_string(),
                room: room.clone(),
                zone: Some(zone.clone()),
                values,
                quality: conf.is_finite().then(|| conf.clamp(0.0, 1.0)),
                health: if nf.stale || offline { Health::Stale } else { Health::Ok },
                synthetic: !is_hardware_source(source),
                verified: true,
            });
        }
    }

    pub fn zones(&self, now_ms: i64) -> Vec<ZonePresence> {
        self.fusion.zones(now_ms)
    }

    /// Response body of `GET /api/v1/zones`.
    pub fn zones_json(&self, now_ms: i64) -> serde_json::Value {
        let zones = self.zones(now_ms);
        serde_json::json!({
            "task": "presence",
            "configured": !zones.is_empty() || !self.node_zones.is_empty(),
            "timestamp_ms": now_ms,
            "stale_after_ms": STALE_AFTER_MS,
            "rejected_readings": self.fusion.rejected(),
            "zones": zones,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_794_250_000_000;

    fn line(source: &str, kind: &str, zone: &str, values: &str, extra: &str) -> String {
        format!(
            r#"{{"timestamp_ms":{NOW},"source":"{source}","kind":"{kind}","room":"office","zone":"{zone}","values":{values}{extra}}}"#
        )
    }

    fn fusion() -> SourceFusion {
        SourceFusion::new(builtin_descriptors())
    }

    #[test]
    fn agreeing_sources_fuse_above_either_alone_is_not_required_but_stay_present() {
        let mut f = fusion();
        f.ingest_line(&line("csi-1", "wifi_csi", "desk", r#"{"presence_probability":0.8}"#, ""), NOW).unwrap();
        f.ingest_line(&line("radar-1", "mmwave_radar", "desk", r#"{"presence":1}"#, ""), NOW).unwrap();
        let z = &f.zones(NOW)[0];
        let p = z.presence_probability.unwrap();
        assert!(p > 0.8 && p < 0.9, "weighted log-odds mean of 0.8 and 0.9, got {p}");
        assert_eq!(z.agreement, Some(true));
        assert_eq!(z.confidence, 1.0);
        assert_eq!(z.evidence, "CLAIMED", "wire readings are unverified");
        assert!(z.sources.iter().all(|s| !s.verified));
        assert!(!z.weights_measured, "built-in weights are placeholders");
    }

    #[test]
    fn disagreement_is_reported_not_hidden() {
        let mut f = fusion();
        f.ingest_line(&line("csi-1", "wifi_csi", "desk", r#"{"presence_probability":0.1}"#, ""), NOW).unwrap();
        f.ingest_line(&line("radar-1", "mmwave_radar", "desk", r#"{"presence":1}"#, ""), NOW).unwrap();
        let z = &f.zones(NOW)[0];
        assert_eq!(z.agreement, Some(false));
        let p = z.presence_probability.unwrap();
        assert!((0.4..0.6).contains(&p), "equal weights on opposite sides land near 0.5, got {p}");
    }

    #[test]
    fn stale_source_lowers_confidence_and_never_reads_as_empty() {
        let mut f = fusion();
        f.ingest_line(&line("csi-1", "wifi_csi", "desk", r#"{"presence_probability":0.9}"#, ""), NOW).unwrap();
        f.ingest_line(&line("radar-1", "mmwave_radar", "desk", r#"{"presence":0}"#, ""), NOW).unwrap();
        // Only the radar keeps reporting; CSI's last reading ages out.
        let later = NOW + STALE_AFTER_MS + 1;
        let mut fresh = parse_reading(&line("radar-1", "mmwave_radar", "desk", r#"{"presence":0}"#, ""), NOW).unwrap();
        fresh.timestamp_ms = later;
        f.ingest(fresh);
        let z = &f.zones(later)[0];
        assert_eq!(z.confidence, 0.5);
        let csi = z.sources.iter().find(|s| s.source == "csi-1").unwrap();
        assert!(!csi.contributing && csi.probability.is_none());

        // Everything stale: no probability at all, not "0".
        let z = &f.zones(later + STALE_AFTER_MS + 1)[0];
        assert_eq!(z.presence_probability, None);
        assert_eq!(z.agreement, None);
        assert_eq!(z.confidence, 0.0);
        assert_eq!(z.evidence, "NONE");
    }

    #[test]
    fn unhealthy_source_does_not_contribute() {
        let mut f = fusion();
        f.ingest_line(&line("radar-1", "mmwave_radar", "desk", r#"{"presence":0}"#, r#","health":"stale""#), NOW).unwrap();
        let z = &f.zones(NOW)[0];
        assert_eq!(z.presence_probability, None);
    }

    #[test]
    fn any_synthetic_input_makes_the_result_synthetic() {
        let mut f = fusion();
        f.ingest_line(&line("csi-1", "wifi_csi", "desk", r#"{"presence_probability":0.8}"#, ""), NOW).unwrap();
        f.ingest_line(&line("sim-1", "mmwave_radar", "desk", r#"{"presence":1}"#, r#","synthetic":true"#), NOW).unwrap();
        assert_eq!(f.zones(NOW)[0].evidence, "SYNTHETIC");
    }

    #[test]
    fn a_sender_cannot_assert_verified_and_only_server_readings_are_measured() {
        let mut f = fusion();
        f.ingest_line(&line("radar-1", "mmwave_radar", "desk", r#"{"presence":1}"#, r#","verified":true"#), NOW).unwrap();
        assert_eq!(f.zones(NOW)[0].evidence, "CLAIMED");

        let mut own = parse_reading(&line("csi-node-1", "wifi_csi", "sofa", r#"{"presence":1}"#, ""), NOW).unwrap();
        own.verified = true;
        f.ingest(own);
        let zones = f.zones(NOW);
        let sofa = zones.iter().find(|z| z.zone == "sofa").unwrap();
        assert_eq!(sofa.evidence, "MEASURED");
        assert_eq!(evidence_grade(true, true, true), "SYNTHETIC");
    }

    #[test]
    fn unknown_kind_and_zoneless_readings_are_context_only() {
        let mut f = fusion();
        f.ingest_line(&line("door-1", "door_contact", "desk", r#"{"open":1}"#, ""), NOW).unwrap();
        f.ingest_line(
            &format!(r#"{{"timestamp_ms":{NOW},"source":"csi-9","kind":"wifi_csi","values":{{"presence":1}}}}"#),
            NOW,
        )
        .unwrap();
        assert!(f.zones(NOW).is_empty());
    }

    #[test]
    fn low_quality_flag_is_pulled_toward_no_information() {
        let d = &builtin_descriptors()[1];
        let hi = parse_reading(&line("r", "mmwave_radar", "z", r#"{"presence":1}"#, r#","quality":1.0"#), NOW).unwrap();
        let lo = parse_reading(&line("r", "mmwave_radar", "z", r#"{"presence":1}"#, r#","quality":0.0"#), NOW).unwrap();
        assert_eq!(presence_probability(&hi, d), Some(0.9));
        assert_eq!(presence_probability(&lo, d), Some(0.5));
        let bad = parse_reading(&line("r", "mmwave_radar", "z", r#"{"presence":7}"#, ""), NOW).unwrap();
        assert_eq!(presence_probability(&bad, d), None);
    }

    #[test]
    fn boundary_rejects_bad_lines_and_counts_them() {
        let mut f = fusion();
        let cases = [
            "not json".to_string(),
            line("../etc", "wifi_csi", "desk", r#"{"presence":1}"#, ""),
            line("s", "wifi csi", "desk", r#"{"presence":1}"#, ""),
            line("s", "wifi_csi", "de/sk", r#"{"presence":1}"#, ""),
            line("s", "wifi_csi", "desk", r#"{"presence":1}"#, r#","quality":1.5"#),
            line("s", "wifi_csi", "desk", r#"{"bad name":1}"#, ""),
            format!(r#"{{"timestamp_ms":{},"source":"s","kind":"wifi_csi","values":{{}}}}"#, NOW + 60_000),
            format!(r#"{{"timestamp_ms":0,"source":"s","kind":"wifi_csi","values":{{}}}}"#),
            format!("{}{}", " ".repeat(MAX_LINE_BYTES), "{}"),
        ];
        for c in &cases {
            assert!(f.ingest_line(c, NOW).is_err(), "should reject: {}", &c[..c.len().min(80)]);
        }
        assert_eq!(f.rejected(), cases.len() as u64);
        assert!(f.zones(NOW).is_empty());
        let many: String = (0..=MAX_VALUES).map(|i| format!(r#""v{i}":1"#)).collect::<Vec<_>>().join(",");
        assert_eq!(
            parse_reading(&line("s", "wifi_csi", "desk", &format!("{{{many}}}"), ""), NOW),
            Err(ReadingError::TooManyValues)
        );
    }

    #[test]
    fn older_reading_never_replaces_a_newer_one() {
        let mut f = fusion();
        let mut new = parse_reading(&line("r", "mmwave_radar", "desk", r#"{"presence":1}"#, ""), NOW).unwrap();
        let mut old = new.clone();
        old.timestamp_ms -= 1000;
        old.values.insert("presence".into(), 0.0);
        new.timestamp_ms = NOW;
        f.ingest(new);
        f.ingest(old);
        assert!(f.zones(NOW)[0].presence_probability.unwrap() > 0.5);
    }
}
