use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MemorySnapshot {
    pub peak_rss_bytes: u64,
    pub kv_cache_bytes: u64,
    pub activation_bytes: u64,
    pub temp_buffer_bytes: u64,
    pub timestamp_us: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MemoryReport {
    pub label: String,
    pub peak_rss: u64,
    pub mean_rss: u64,
    pub kv_cache_total: u64,
    pub activation_total: u64,
}

/// Capture current memory via /proc/self/status (Linux) or zero fallback.
pub fn capture_memory() -> MemorySnapshot {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as u64;
    MemorySnapshot {
        peak_rss_bytes: read_vm_rss(),
        kv_cache_bytes: 0,
        activation_bytes: 0,
        temp_buffer_bytes: 0,
        timestamp_us: ts,
    }
}

#[cfg(target_os = "linux")]
fn read_vm_rss() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmRSS:"))
                .and_then(|l| {
                    l.trim_start_matches("VmRSS:")
                        .trim()
                        .trim_end_matches("kB")
                        .trim()
                        .parse::<u64>()
                        .ok()
                })
                .map(|kb| kb * 1024)
        })
        .unwrap_or(0)
}

#[cfg(not(target_os = "linux"))]
fn read_vm_rss() -> u64 {
    0
}

pub struct MemoryTracker {
    pub snapshots: Vec<MemorySnapshot>,
    pub label: String,
}

impl MemoryTracker {
    pub fn new(label: &str) -> Self {
        Self {
            snapshots: Vec::new(),
            label: label.to_string(),
        }
    }

    pub fn snapshot(&mut self) {
        self.snapshots.push(capture_memory());
    }

    pub fn peak(&self) -> u64 {
        self.snapshots
            .iter()
            .map(|s| s.peak_rss_bytes)
            .max()
            .unwrap_or(0)
    }

    pub fn report(&self) -> MemoryReport {
        let n = self.snapshots.len().max(1) as u64;
        MemoryReport {
            label: self.label.clone(),
            peak_rss: self.peak(),
            mean_rss: self.snapshots.iter().map(|s| s.peak_rss_bytes).sum::<u64>() / n,
            kv_cache_total: self.snapshots.iter().map(|s| s.kv_cache_bytes).sum(),
            activation_total: self.snapshots.iter().map(|s| s.activation_bytes).sum(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_returns_nonzero_timestamp() {
        assert!(capture_memory().timestamp_us > 0);
    }

    #[test]
    fn tracker_peak_empty() {
        assert_eq!(MemoryTracker::new("x").peak(), 0);
    }

    #[test]
    fn tracker_report_aggregates() {
        let mut t = MemoryTracker::new("test");
        let mk = |rss, kv, act| MemorySnapshot {
            peak_rss_bytes: rss,
            kv_cache_bytes: kv,
            activation_bytes: act,
            temp_buffer_bytes: 0,
            timestamp_us: 1,
        };
        t.snapshots.push(mk(100, 10, 20));
        t.snapshots.push(mk(200, 30, 40));
        let r = t.report();
        assert_eq!(
            (r.peak_rss, r.mean_rss, r.kv_cache_total, r.activation_total),
            (200, 150, 40, 60)
        );
    }
}
