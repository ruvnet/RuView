//! BSSID Registry aggregate root.
//!
//! The `BssidRegistry` is the aggregate root of the BSSID Acquisition bounded
//! context. It tracks all visible access points across scans, maintains
//! identity stability as BSSIDs appear and disappear, and provides a
//! consistent subcarrier mapping for pseudo-CSI frame construction.

use std::collections::HashMap;
use std::collections::VecDeque;
use std::time::Instant;

use crate::domain::bssid::{BandType, BssidId, BssidObservation, RadioType};
use crate::domain::frame::MultiApFrame;

// ---------------------------------------------------------------------------
// RunningStats -- Welford online statistics
// ---------------------------------------------------------------------------

/// Welford online algorithm for computing running mean and variance.
///
/// This allows us to compute per-BSSID statistics incrementally without
/// storing the entire history, which is essential for detecting which BSSIDs
/// show body-correlated variance versus static background.
#[derive(Debug, Clone)]
pub struct RunningStats {
    /// Number of samples seen.
    count: u64,
    /// Running mean.
    mean: f64,
    /// Running M2 accumulator (sum of squared differences from the mean).
    m2: f64,
}

impl RunningStats {
    /// Create a new empty `RunningStats`.
    pub fn new() -> Self {
        Self {
            count: 0,
            mean: 0.0,
            m2: 0.0,
        }
    }

    /// Push a new sample into the running statistics.
    pub fn push(&mut self, value: f64) {
        self.count += 1;
        let delta = value - self.mean;
        self.mean += delta / self.count as f64;
        let delta2 = value - self.mean;
        self.m2 += delta * delta2;
    }

    /// The number of samples observed.
    pub fn count(&self) -> u64 {
        self.count
    }

    /// The running mean. Returns 0.0 if no samples have been pushed.
    pub fn mean(&self) -> f64 {
        self.mean
    }

    /// The population variance. Returns 0.0 if fewer than 2 samples.
    pub fn variance(&self) -> f64 {
        if self.count < 2 {
            0.0
        } else {
            self.m2 / self.count as f64
        }
    }

    /// The sample variance (Bessel-corrected). Returns 0.0 if fewer than 2 samples.
    pub fn sample_variance(&self) -> f64 {
        if self.count < 2 {
            0.0
        } else {
            self.m2 / (self.count - 1) as f64
        }
    }

    /// The population standard deviation.
    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    /// Reset all statistics to zero.
    pub fn reset(&mut self) {
        self.count = 0;
        self.mean = 0.0;
        self.m2 = 0.0;
    }
}

impl Default for RunningStats {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// BssidMeta -- metadata about a tracked BSSID
// ---------------------------------------------------------------------------

/// Static metadata about a tracked BSSID, captured on first observation.
#[derive(Debug, Clone)]
pub struct BssidMeta {
    /// The SSID (network name). May be empty for hidden networks.
    pub ssid: String,
    /// The 802.11 channel number.
    pub channel: u8,
    /// The frequency band.
    pub band: BandType,
    /// The radio standard.
    pub radio_type: RadioType,
    /// When this BSSID was first observed.
    pub first_seen: Instant,
}

// ---------------------------------------------------------------------------
// BssidEntry -- Entity
// ---------------------------------------------------------------------------

/// A tracked BSSID with observation history and running statistics.
///
/// Each entry corresponds to one physical access point. The ring buffer
/// stores recent RSSI values (in dBm) for temporal analysis, while the
/// `RunningStats` provides efficient online mean/variance without needing
/// the full history.
#[derive(Debug, Clone)]
pub struct BssidEntry {
    /// The unique identifier for this BSSID.
    pub id: BssidId,
    /// Static metadata (SSID, channel, band, radio type).
    pub meta: BssidMeta,
    /// Ring buffer of recent RSSI observations (dBm).
    pub history: VecDeque<f64>,
    /// Welford online statistics over the full observation lifetime.
    pub stats: RunningStats,
    /// When this BSSID was last observed.
    pub last_seen: Instant,
    /// Index in the subcarrier map, or `None` if not yet assigned.
    pub subcarrier_idx: Option<usize>,
}

impl BssidEntry {
    /// Maximum number of RSSI samples kept in the ring buffer history.
    pub const DEFAULT_HISTORY_CAPACITY: usize = 128;

    /// Create a new entry from a first observation.
    fn new(obs: &BssidObservation) -> Self {
        let mut stats = RunningStats::new();
        stats.push(obs.rssi_dbm);

        let mut history = VecDeque::with_capacity(Self::DEFAULT_HISTORY_CAPACITY);
        history.push_back(obs.rssi_dbm);

        Self {
            id: obs.bssid,
            meta: BssidMeta {
                ssid: obs.ssid.clone(),
                channel: obs.channel,
                band: obs.band,
                radio_type: obs.radio_type,
                first_seen: obs.timestamp,
            },
            history,
            stats,
            last_seen: obs.timestamp,
            subcarrier_idx: None,
        }
    }

    /// Record a new observation for this BSSID.
    fn record(&mut self, obs: &BssidObservation) {
        self.stats.push(obs.rssi_dbm);

        if self.history.len() >= Self::DEFAULT_HISTORY_CAPACITY {
            self.history.pop_front();
        }
        self.history.push_back(obs.rssi_dbm);

        self.last_seen = obs.timestamp;

        // Update mutable metadata in case the AP changed channel/band
        self.meta.channel = obs.channel;
        self.meta.band = obs.band;
        self.meta.radio_type = obs.radio_type;
        if !obs.ssid.is_empty() {
            self.meta.ssid = obs.ssid.clone();
        }
    }

    /// The RSSI variance over the observation lifetime (Welford).
    pub fn variance(&self) -> f64 {
        self.stats.variance()
    }

    /// The most recent RSSI observation in dBm.
    pub fn latest_rssi(&self) -> Option<f64> {
        self.history.back().copied()
    }
}

// ---------------------------------------------------------------------------
// BssidRegistry -- Aggregate Root
// ---------------------------------------------------------------------------

/// Aggregate root that tracks all visible BSSIDs across scans.
///
/// The registry maintains:
/// - A map of known BSSIDs with per-BSSID history and statistics.
/// - An ordered subcarrier map that assigns each BSSID a stable index,
///   sorted by first-seen time so that the mapping is deterministic.
/// - Expiry logic to remove BSSIDs that have not been observed recently.
#[derive(Debug, Clone)]
pub struct BssidRegistry {
    /// Known BSSIDs with sliding window of observations.
    entries: HashMap<BssidId, BssidEntry>,
    /// Ordered list of BSSID IDs for consistent subcarrier mapping.
    /// Sorted by first-seen time for stability.
    subcarrier_map: Vec<BssidId>,
    /// Maximum number of tracked BSSIDs (maps to max pseudo-subcarriers).
    max_bssids: usize,
    /// How long a BSSID can go unseen before being expired (in seconds).
    expiry_secs: u64,
}

impl BssidRegistry {
    /// Default maximum number of tracked BSSIDs.
    pub const DEFAULT_MAX_BSSIDS: usize = 32;

    /// Default expiry time in seconds.
    pub const DEFAULT_EXPIRY_SECS: u64 = 30;

    /// Create a new registry with the given capacity and expiry settings.
    pub fn new(max_bssids: usize, expiry_secs: u64) -> Self {
        Self {
            entries: HashMap::with_capacity(max_bssids),
            subcarrier_map: Vec::with_capacity(max_bssids),
            max_bssids,
            expiry_secs,
        }
    }

    /// Update the registry with a batch of observations from a single scan.
    ///
    /// New BSSIDs are registered and assigned subcarrier indices. Existing
    /// BSSIDs have their history and statistics updated. BSSIDs that have
    /// not been seen within the expiry window are removed.
    pub fn update(&mut self, observations: &[BssidObservation]) {
        let now = if let Some(obs) = observations.first() {
            obs.timestamp
        } else {
            return;
        };

        // Update or insert each observed BSSID
        for obs in observations {
            if let Some(entry) = self.entries.get_mut(&obs.bssid) {
                entry.record(obs);
            } else if self.subcarrier_map.len() < self.max_bssids {
                // New BSSID: register it
                let mut entry = BssidEntry::new(obs);
                let idx = self.subcarrier_map.len();
                entry.subcarrier_idx = Some(idx);
                self.subcarrier_map.push(obs.bssid);
                self.entries.insert(obs.bssid, entry);
            }
            // If we are at capacity, silently ignore new BSSIDs.
            // A smarter policy (evict lowest-variance) can be added later.
        }

        // Expire stale BSSIDs
        self.expire(now);
    }

    /// Remove BSSIDs that have not been observed within the expiry window.
    fn expire(&mut self, now: Instant) {
        let expiry = std::time::Duration::from_secs(self.expiry_secs);
        let stale: Vec<BssidId> = self
            .entries
            .iter()
            .filter(|(_, entry)| now.duration_since(entry.last_seen) > expiry)
            .map(|(id, _)| *id)
            .collect();

        for id in &stale {
            self.entries.remove(id);
        }

        if !stale.is_empty() {
            // Rebuild the subcarrier map without the stale entries,
            // preserving relative ordering.
            self.subcarrier_map.retain(|id| !stale.contains(id));
            // Re-index remaining entries
            for (idx, id) in self.subcarrier_map.iter().enumerate() {
                if let Some(entry) = self.entries.get_mut(id) {
                    entry.subcarrier_idx = Some(idx);
                }
            }
        }
    }

    /// Look up the subcarrier index assigned to a BSSID.
    pub fn subcarrier_index(&self, bssid: &BssidId) -> Option<usize> {
        self.entries
            .get(bssid)
            .and_then(|entry| entry.subcarrier_idx)
    }

    /// Return the ordered subcarrier map (list of BSSID IDs).
    pub fn subcarrier_map(&self) -> &[BssidId] {
        &self.subcarrier_map
    }

    /// The number of currently tracked BSSIDs.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The maximum number of BSSIDs this registry can track.
    pub fn capacity(&self) -> usize {
        self.max_bssids
    }

    /// Get an entry by BSSID ID.
    pub fn get(&self, bssid: &BssidId) -> Option<&BssidEntry> {
        self.entries.get(bssid)
    }

    /// Iterate over all tracked entries.
    pub fn entries(&self) -> impl Iterator<Item = &BssidEntry> {
        self.entries.values()
    }

    /// Build a `MultiApFrame` from the current registry state.
    ///
    /// The frame contains one slot per subcarrier (BSSID), with amplitudes
    /// derived from the most recent RSSI observation and pseudo-phase from
    /// the channel number.
    pub fn to_multi_ap_frame(&self) -> MultiApFrame {
        let n = self.subcarrier_map.len();
        let mut rssi_dbm = vec![0.0_f64; n];
        let mut amplitudes = vec![0.0_f64; n];
        let mut phases = vec![0.0_f64; n];
        let mut per_bssid_variance = vec![0.0_f64; n];
        let mut histories: Vec<VecDeque<f64>> = Vec::with_capacity(n);

        for (idx, bssid_id) in self.subcarrier_map.iter().enumerate() {
            if let Some(entry) = self.entries.get(bssid_id) {
                let latest = entry.latest_rssi().unwrap_or(-100.0);
                rssi_dbm[idx] = latest;
                amplitudes[idx] = BssidObservation::rssi_to_amplitude(latest);
                phases[idx] = (entry.meta.channel as f64 / 48.0) * std::f64::consts::PI;
                per_bssid_variance[idx] = entry.variance();
                histories.push(entry.history.clone());
            } else {
                histories.push(VecDeque::new());
            }
        }

        // Estimate sample rate from observation count and time span
        let sample_rate_hz = self.estimate_sample_rate();

        MultiApFrame {
            bssid_count: n,
            rssi_dbm,
            amplitudes,
            phases,
            per_bssid_variance,
            histories,
            sample_rate_hz,
            timestamp: Instant::now(),
        }
    }

    /// Rough estimate of the effective sample rate based on observation history.
    fn estimate_sample_rate(&self) -> f64 {
        // Default to 2 Hz (Tier 1 netsh rate) when we cannot compute
        if self.entries.is_empty() {
            return 2.0;
        }

        // Use the first entry with enough history
        for entry in self.entries.values() {
            if entry.stats.count() >= 4 {
                let elapsed = entry
                    .last_seen
                    .duration_since(entry.meta.first_seen)
                    .as_secs_f64();
                if elapsed > 0.0 {
                    return entry.stats.count() as f64 / elapsed;
                }
            }
        }

        2.0 // Fallback: assume Tier 1 rate
    }
}

impl Default for BssidRegistry {
    fn default() -> Self {
        Self::new(Self::DEFAULT_MAX_BSSIDS, Self::DEFAULT_EXPIRY_SECS)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::bssid::{BandType, RadioType};

    fn make_obs(mac: [u8; 6], rssi: f64, channel: u8) -> BssidObservation {
        BssidObservation {
            bssid: BssidId(mac),
            rssi_dbm: rssi,
            signal_pct: (rssi + 100.0) * 2.0,
            channel,
            band: BandType::from_channel(channel),
            radio_type: RadioType::Ax,
            ssid: "TestNetwork".to_string(),
            timestamp: Instant::now(),
        }
    }

    #[test]
    fn registry_tracks_new_bssids() {
        let mut reg = BssidRegistry::default();
        let obs = vec![
            make_obs([0x01; 6], -60.0, 6),
            make_obs([0x02; 6], -70.0, 36),
        ];
        reg.update(&obs);

        assert_eq!(reg.len(), 2);
        assert_eq!(reg.subcarrier_index(&BssidId([0x01; 6])), Some(0));
        assert_eq!(reg.subcarrier_index(&BssidId([0x02; 6])), Some(1));
    }

    #[test]
    fn registry_updates_existing_bssid() {
        let mut reg = BssidRegistry::default();
        let mac = [0xaa; 6];

        let obs1 = vec![make_obs(mac, -60.0, 6)];
        reg.update(&obs1);

        let obs2 = vec![make_obs(mac, -65.0, 6)];
        reg.update(&obs2);

        let entry = reg.get(&BssidId(mac)).unwrap();
        assert_eq!(entry.stats.count(), 2);
        assert_eq!(entry.history.len(), 2);
        assert!((entry.stats.mean() - (-62.5)).abs() < 1e-9);
    }

    #[test]
    fn registry_respects_capacity() {
        let mut reg = BssidRegistry::new(2, 30);
        let obs = vec![
            make_obs([0x01; 6], -60.0, 1),
            make_obs([0x02; 6], -70.0, 6),
            make_obs([0x03; 6], -80.0, 11), // Should be ignored
        ];
        reg.update(&obs);

        assert_eq!(reg.len(), 2);
        assert!(reg.get(&BssidId([0x03; 6])).is_none());
    }

    #[test]
    fn to_multi_ap_frame_builds_correct_frame() {
        let mut reg = BssidRegistry::default();
        let obs = vec![
            make_obs([0x01; 6], -60.0, 6),
            make_obs([0x02; 6], -70.0, 36),
        ];
        reg.update(&obs);

        let frame = reg.to_multi_ap_frame();
        assert_eq!(frame.bssid_count, 2);
        assert_eq!(frame.rssi_dbm.len(), 2);
        assert_eq!(frame.amplitudes.len(), 2);
        assert_eq!(frame.phases.len(), 2);
        assert!(frame.amplitudes[0] > frame.amplitudes[1]); // -60 dBm > -70 dBm
    }

    #[test]
    fn welford_stats_accuracy() {
        let mut stats = RunningStats::new();
        let values = [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        for v in &values {
            stats.push(*v);
        }

        assert_eq!(stats.count(), 8);
        assert!((stats.mean() - 5.0).abs() < 1e-9);
        // Population variance of this dataset is 4.0
        assert!((stats.variance() - 4.0).abs() < 1e-9);
        // Sample variance is 4.571428...
        assert!((stats.sample_variance() - (32.0 / 7.0)).abs() < 1e-9);
    }
}
