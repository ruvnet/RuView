#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LatencyRecord {
    pub sample_id: usize,
    pub wall_time_us: u64,
    pub kernel_time_us: u64,
    pub seq_len: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LatencyStats {
    pub p50_us: u64,
    pub p95_us: u64,
    pub p99_us: u64,
    pub mean_us: f64,
    pub std_us: f64,
    pub n: usize,
}

/// Compute percentile and summary statistics from wall-time latencies.
pub fn compute_latency_stats(records: &[LatencyRecord]) -> LatencyStats {
    let n = records.len();
    if n == 0 {
        return LatencyStats {
            p50_us: 0,
            p95_us: 0,
            p99_us: 0,
            mean_us: 0.0,
            std_us: 0.0,
            n: 0,
        };
    }
    let mut times: Vec<u64> = records.iter().map(|r| r.wall_time_us).collect();
    times.sort_unstable();
    let mean = times.iter().sum::<u64>() as f64 / n as f64;
    let var = times
        .iter()
        .map(|&t| (t as f64 - mean).powi(2))
        .sum::<f64>()
        / n as f64;
    LatencyStats {
        p50_us: pctl(&times, 50.0),
        p95_us: pctl(&times, 95.0),
        p99_us: pctl(&times, 99.0),
        mean_us: mean,
        std_us: var.sqrt(),
        n,
    }
}

fn pctl(sorted: &[u64], p: f64) -> u64 {
    let idx = ((p / 100.0 * sorted.len() as f64).ceil() as usize)
        .min(sorted.len())
        .saturating_sub(1);
    sorted[idx]
}

#[cfg(test)]
mod tests {
    use super::*;
    fn recs(ts: &[u64]) -> Vec<LatencyRecord> {
        ts.iter()
            .enumerate()
            .map(|(i, &t)| LatencyRecord {
                sample_id: i,
                wall_time_us: t,
                kernel_time_us: t,
                seq_len: 128,
            })
            .collect()
    }

    #[test]
    fn empty() {
        assert_eq!(compute_latency_stats(&[]).n, 0);
    }
    #[test]
    fn single() {
        let s = compute_latency_stats(&recs(&[42]));
        assert_eq!((s.p50_us, s.p99_us, s.n), (42, 42, 1));
    }
    #[test]
    fn multi() {
        let s = compute_latency_stats(&recs(&[10, 20, 30, 40, 50, 60, 70, 80, 90, 100]));
        assert_eq!(s.p50_us, 50);
        assert!((s.mean_us - 55.0).abs() < 1e-9);
    }
    #[test]
    fn unsorted() {
        assert_eq!(
            compute_latency_stats(&recs(&[100, 10, 50, 90, 20])).p50_us,
            50
        );
    }
}
