use crate::latency::LatencyRecord;
use crate::memory::MemorySnapshot;
use std::io::Write;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResultRow {
    pub setting: String,
    pub coherence_delta: f64,
    pub kv_cache_reduction: f64,
    pub peak_mem_reduction: f64,
    pub energy_reduction: f64,
    pub p95_latency_us: u64,
    pub accuracy: f64,
}

pub fn write_results_csv(path: &str, rows: &[ResultRow]) -> std::io::Result<()> {
    let mut f = std::fs::File::create(path)?;
    writeln!(f, "setting,coherence_delta,kv_cache_reduction,peak_mem_reduction,energy_reduction,p95_latency_us,accuracy")?;
    for r in rows {
        writeln!(
            f,
            "{},{},{},{},{},{},{}",
            esc(&r.setting),
            r.coherence_delta,
            r.kv_cache_reduction,
            r.peak_mem_reduction,
            r.energy_reduction,
            r.p95_latency_us,
            r.accuracy
        )?;
    }
    Ok(())
}

pub fn write_latency_csv(path: &str, records: &[LatencyRecord]) -> std::io::Result<()> {
    let mut f = std::fs::File::create(path)?;
    writeln!(f, "sample_id,wall_time_us,kernel_time_us,seq_len")?;
    for r in records {
        writeln!(
            f,
            "{},{},{},{}",
            r.sample_id, r.wall_time_us, r.kernel_time_us, r.seq_len
        )?;
    }
    Ok(())
}

pub fn write_memory_csv(path: &str, snapshots: &[MemorySnapshot]) -> std::io::Result<()> {
    let mut f = std::fs::File::create(path)?;
    writeln!(
        f,
        "timestamp_us,peak_rss_bytes,kv_cache_bytes,activation_bytes,temp_buffer_bytes"
    )?;
    for s in snapshots {
        writeln!(
            f,
            "{},{},{},{},{}",
            s.timestamp_us,
            s.peak_rss_bytes,
            s.kv_cache_bytes,
            s.activation_bytes,
            s.temp_buffer_bytes
        )?;
    }
    Ok(())
}

fn esc(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn esc_plain() {
        assert_eq!(esc("hello"), "hello");
    }
    #[test]
    fn esc_comma() {
        assert_eq!(esc("a,b"), "\"a,b\"");
    }

    #[test]
    fn roundtrip_results() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("r.csv");
        write_results_csv(
            p.to_str().unwrap(),
            &[ResultRow {
                setting: "base".into(),
                coherence_delta: 0.01,
                kv_cache_reduction: 0.0,
                peak_mem_reduction: 0.0,
                energy_reduction: 0.0,
                p95_latency_us: 1200,
                accuracy: 0.95,
            }],
        )
        .unwrap();
        let c = std::fs::read_to_string(&p).unwrap();
        assert_eq!(c.lines().count(), 2);
    }

    #[test]
    fn roundtrip_latency() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("l.csv");
        write_latency_csv(
            p.to_str().unwrap(),
            &[LatencyRecord {
                sample_id: 0,
                wall_time_us: 100,
                kernel_time_us: 80,
                seq_len: 64,
            }],
        )
        .unwrap();
        assert_eq!(std::fs::read_to_string(&p).unwrap().lines().count(), 2);
    }

    #[test]
    fn roundtrip_memory() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("m.csv");
        write_memory_csv(
            p.to_str().unwrap(),
            &[MemorySnapshot {
                peak_rss_bytes: 1024,
                kv_cache_bytes: 256,
                activation_bytes: 512,
                temp_buffer_bytes: 128,
                timestamp_us: 999,
            }],
        )
        .unwrap();
        let c = std::fs::read_to_string(&p).unwrap();
        assert!(c.contains("999,1024,256,512,128"));
    }
}
