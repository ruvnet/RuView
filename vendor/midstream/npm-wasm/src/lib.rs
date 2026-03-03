use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Set panic hook for better error messages in browser
#[wasm_bindgen(start)]
pub fn init_panic_hook() {
    console_error_panic_hook::set_once();
}

// ============================================================================
// TEMPORAL COMPARISON MODULE (DTW, LCS, Edit Distance)
// ============================================================================

#[wasm_bindgen]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalMetrics {
    dtw_distance: f64,
    lcs_length: usize,
    edit_distance: usize,
    similarity_score: f64,
}

#[wasm_bindgen]
impl TemporalMetrics {
    #[wasm_bindgen(getter)]
    pub fn dtw_distance(&self) -> f64 {
        self.dtw_distance
    }

    #[wasm_bindgen(getter)]
    pub fn lcs_length(&self) -> usize {
        self.lcs_length
    }

    #[wasm_bindgen(getter)]
    pub fn edit_distance(&self) -> usize {
        self.edit_distance
    }

    #[wasm_bindgen(getter)]
    pub fn similarity_score(&self) -> f64 {
        self.similarity_score
    }
}

#[wasm_bindgen]
pub struct TemporalCompare {
    window_size: usize,
}

#[wasm_bindgen]
impl TemporalCompare {
    #[wasm_bindgen(constructor)]
    pub fn new(window_size: Option<usize>) -> Self {
        Self {
            window_size: window_size.unwrap_or(100),
        }
    }

    /// Dynamic Time Warping distance between two sequences
    #[wasm_bindgen]
    pub fn dtw(&self, seq1: &[f64], seq2: &[f64]) -> f64 {
        let n = seq1.len();
        let m = seq2.len();

        if n == 0 || m == 0 {
            return f64::INFINITY;
        }

        let mut dtw_matrix = vec![vec![f64::INFINITY; m + 1]; n + 1];
        dtw_matrix[0][0] = 0.0;

        for i in 1..=n {
            for j in 1..=m {
                let cost = (seq1[i - 1] - seq2[j - 1]).abs();
                dtw_matrix[i][j] = cost + dtw_matrix[i - 1][j]
                    .min(dtw_matrix[i][j - 1])
                    .min(dtw_matrix[i - 1][j - 1]);
            }
        }

        dtw_matrix[n][m]
    }

    /// Longest Common Subsequence length
    #[wasm_bindgen]
    pub fn lcs(&self, seq1: &[i32], seq2: &[i32]) -> usize {
        let n = seq1.len();
        let m = seq2.len();

        let mut dp = vec![vec![0; m + 1]; n + 1];

        for i in 1..=n {
            for j in 1..=m {
                if seq1[i - 1] == seq2[j - 1] {
                    dp[i][j] = dp[i - 1][j - 1] + 1;
                } else {
                    dp[i][j] = dp[i - 1][j].max(dp[i][j - 1]);
                }
            }
        }

        dp[n][m]
    }

    /// Levenshtein edit distance
    #[wasm_bindgen]
    pub fn edit_distance(&self, s1: &str, s2: &str) -> usize {
        let s1_chars: Vec<char> = s1.chars().collect();
        let s2_chars: Vec<char> = s2.chars().collect();
        let n = s1_chars.len();
        let m = s2_chars.len();

        let mut dp = vec![vec![0; m + 1]; n + 1];

        for i in 0..=n {
            dp[i][0] = i;
        }
        for j in 0..=m {
            dp[0][j] = j;
        }

        for i in 1..=n {
            for j in 1..=m {
                let cost = if s1_chars[i - 1] == s2_chars[j - 1] { 0 } else { 1 };
                dp[i][j] = (dp[i - 1][j] + 1)
                    .min(dp[i][j - 1] + 1)
                    .min(dp[i - 1][j - 1] + cost);
            }
        }

        dp[n][m]
    }

    /// Comprehensive temporal analysis
    #[wasm_bindgen]
    pub fn analyze(&self, seq1: &[f64], seq2: &[f64]) -> TemporalMetrics {
        let dtw_distance = self.dtw(seq1, seq2);

        // Convert to i32 for LCS (quantize to integer values)
        let seq1_int: Vec<i32> = seq1.iter().map(|&x| (x * 100.0) as i32).collect();
        let seq2_int: Vec<i32> = seq2.iter().map(|&x| (x * 100.0) as i32).collect();
        let lcs_length = self.lcs(&seq1_int, &seq2_int);

        // String representation for edit distance
        let s1: String = seq1.iter().map(|&x| ((x * 10.0) as u8 as char)).collect();
        let s2: String = seq2.iter().map(|&x| ((x * 10.0) as u8 as char)).collect();
        let edit_distance = self.edit_distance(&s1, &s2);

        // Calculate similarity score (0.0 to 1.0)
        let max_len = seq1.len().max(seq2.len()) as f64;
        let similarity_score = if max_len > 0.0 {
            1.0 - (dtw_distance / (max_len * 100.0)).min(1.0)
        } else {
            0.0
        };

        TemporalMetrics {
            dtw_distance,
            lcs_length,
            edit_distance,
            similarity_score,
        }
    }
}

// ============================================================================
// NANOSECOND SCHEDULER MODULE
// ============================================================================

#[wasm_bindgen]
pub struct NanoScheduler {
    tasks: HashMap<u32, ScheduledTask>,
    next_id: u32,
}

#[derive(Clone)]
struct ScheduledTask {
    id: u32,
    callback: js_sys::Function,
    delay_ns: f64,
    scheduled_at: f64,
    repeating: bool,
}

#[wasm_bindgen]
impl NanoScheduler {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
            next_id: 1,
        }
    }

    /// Schedule a task with nanosecond precision
    #[wasm_bindgen]
    pub fn schedule(&mut self, callback: js_sys::Function, delay_ns: f64) -> u32 {
        let id = self.next_id;
        self.next_id += 1;

        let now = self.now_ns();

        let task = ScheduledTask {
            id,
            callback,
            delay_ns,
            scheduled_at: now,
            repeating: false,
        };

        self.tasks.insert(id, task);
        id
    }

    /// Schedule a repeating task
    #[wasm_bindgen]
    pub fn schedule_repeating(&mut self, callback: js_sys::Function, interval_ns: f64) -> u32 {
        let id = self.next_id;
        self.next_id += 1;

        let now = self.now_ns();

        let task = ScheduledTask {
            id,
            callback,
            delay_ns: interval_ns,
            scheduled_at: now,
            repeating: true,
        };

        self.tasks.insert(id, task);
        id
    }

    /// Cancel a scheduled task
    #[wasm_bindgen]
    pub fn cancel(&mut self, task_id: u32) -> bool {
        self.tasks.remove(&task_id).is_some()
    }

    /// Get current time in nanoseconds (using performance.now())
    #[wasm_bindgen]
    pub fn now_ns(&self) -> f64 {
        let window = web_sys::window().expect("no global window");
        let performance = window.performance().expect("no performance");
        performance.now() * 1_000_000.0 // Convert ms to ns
    }

    /// Process pending tasks (call from requestAnimationFrame)
    #[wasm_bindgen]
    pub fn tick(&mut self) -> usize {
        let now = self.now_ns();
        let mut executed = 0;
        let mut to_reschedule = Vec::new();

        let ready_tasks: Vec<_> = self.tasks
            .iter()
            .filter(|(_, task)| now >= task.scheduled_at + task.delay_ns)
            .map(|(id, _)| *id)
            .collect();

        for task_id in ready_tasks {
            if let Some(task) = self.tasks.get(&task_id) {
                let _ = task.callback.call0(&JsValue::NULL);
                executed += 1;

                if task.repeating {
                    to_reschedule.push((task_id, task.clone()));
                } else {
                    self.tasks.remove(&task_id);
                }
            }
        }

        // Reschedule repeating tasks
        for (task_id, mut task) in to_reschedule {
            task.scheduled_at = now;
            self.tasks.insert(task_id, task);
        }

        executed
    }

    #[wasm_bindgen(getter)]
    pub fn pending_count(&self) -> usize {
        self.tasks.len()
    }
}

// ============================================================================
// STRANGE LOOP META-LEARNING MODULE
// ============================================================================

#[wasm_bindgen]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaPattern {
    pattern_id: String,
    confidence: f64,
    iteration: u32,
    improvement: f64,
}

#[wasm_bindgen]
impl MetaPattern {
    #[wasm_bindgen(getter)]
    pub fn pattern_id(&self) -> String {
        self.pattern_id.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn confidence(&self) -> f64 {
        self.confidence
    }

    #[wasm_bindgen(getter)]
    pub fn iteration(&self) -> u32 {
        self.iteration
    }

    #[wasm_bindgen(getter)]
    pub fn improvement(&self) -> f64 {
        self.improvement
    }
}

#[wasm_bindgen]
pub struct StrangeLoop {
    patterns: HashMap<String, MetaPattern>,
    iteration: u32,
    learning_rate: f64,
}

#[wasm_bindgen]
impl StrangeLoop {
    #[wasm_bindgen(constructor)]
    pub fn new(learning_rate: Option<f64>) -> Self {
        Self {
            patterns: HashMap::new(),
            iteration: 0,
            learning_rate: learning_rate.unwrap_or(0.1),
        }
    }

    /// Learn from a pattern observation
    #[wasm_bindgen]
    pub fn observe(&mut self, pattern_id: String, performance: f64) {
        self.iteration += 1;

        let improvement = if let Some(existing) = self.patterns.get(&pattern_id) {
            performance - existing.confidence
        } else {
            performance
        };

        let new_confidence = if let Some(existing) = self.patterns.get(&pattern_id) {
            existing.confidence + self.learning_rate * improvement
        } else {
            performance * self.learning_rate
        };

        let pattern = MetaPattern {
            pattern_id: pattern_id.clone(),
            confidence: new_confidence.max(0.0).min(1.0),
            iteration: self.iteration,
            improvement,
        };

        self.patterns.insert(pattern_id, pattern);
    }

    /// Get pattern confidence
    #[wasm_bindgen]
    pub fn get_confidence(&self, pattern_id: &str) -> Option<f64> {
        self.patterns.get(pattern_id).map(|p| p.confidence)
    }

    /// Get best pattern
    #[wasm_bindgen]
    pub fn best_pattern(&self) -> Option<MetaPattern> {
        self.patterns
            .values()
            .max_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap())
            .cloned()
    }

    /// Reflect on learning progress (meta-cognition)
    #[wasm_bindgen]
    pub fn reflect(&self) -> JsValue {
        let summary = serde_wasm_bindgen::to_value(&self.patterns).unwrap();
        summary
    }

    #[wasm_bindgen(getter)]
    pub fn iteration_count(&self) -> u32 {
        self.iteration
    }

    #[wasm_bindgen(getter)]
    pub fn pattern_count(&self) -> usize {
        self.patterns.len()
    }
}

// ============================================================================
// QUIC MULTISTREAM (WebTransport compatible)
// ============================================================================

#[wasm_bindgen]
pub struct QuicMultistream {
    streams: HashMap<u32, StreamInfo>,
    next_stream_id: u32,
}

#[derive(Clone)]
struct StreamInfo {
    stream_id: u32,
    priority: u8,
    bytes_sent: usize,
    bytes_received: usize,
}

#[wasm_bindgen]
impl QuicMultistream {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            streams: HashMap::new(),
            next_stream_id: 0,
        }
    }

    /// Open a new stream with priority
    #[wasm_bindgen]
    pub fn open_stream(&mut self, priority: u8) -> u32 {
        let stream_id = self.next_stream_id;
        self.next_stream_id += 1;

        let stream = StreamInfo {
            stream_id,
            priority,
            bytes_sent: 0,
            bytes_received: 0,
        };

        self.streams.insert(stream_id, stream);
        stream_id
    }

    /// Close a stream
    #[wasm_bindgen]
    pub fn close_stream(&mut self, stream_id: u32) -> bool {
        self.streams.remove(&stream_id).is_some()
    }

    /// Send data on a stream (simulated)
    #[wasm_bindgen]
    pub fn send(&mut self, stream_id: u32, data: &[u8]) -> Result<usize, JsValue> {
        if let Some(stream) = self.streams.get_mut(&stream_id) {
            stream.bytes_sent += data.len();
            Ok(data.len())
        } else {
            Err(JsValue::from_str("Stream not found"))
        }
    }

    /// Receive data on a stream (simulated)
    #[wasm_bindgen]
    pub fn receive(&mut self, stream_id: u32, size: usize) -> Result<Vec<u8>, JsValue> {
        if let Some(stream) = self.streams.get_mut(&stream_id) {
            stream.bytes_received += size;
            Ok(vec![0u8; size])
        } else {
            Err(JsValue::from_str("Stream not found"))
        }
    }

    /// Get stream statistics
    #[wasm_bindgen]
    pub fn get_stats(&self, stream_id: u32) -> JsValue {
        if let Some(stream) = self.streams.get(&stream_id) {
            let stats = js_sys::Object::new();
            js_sys::Reflect::set(&stats, &"stream_id".into(), &stream.stream_id.into()).unwrap();
            js_sys::Reflect::set(&stats, &"priority".into(), &stream.priority.into()).unwrap();
            js_sys::Reflect::set(&stats, &"bytes_sent".into(), &stream.bytes_sent.into()).unwrap();
            js_sys::Reflect::set(&stats, &"bytes_received".into(), &stream.bytes_received.into()).unwrap();
            stats.into()
        } else {
            JsValue::NULL
        }
    }

    #[wasm_bindgen(getter)]
    pub fn stream_count(&self) -> usize {
        self.streams.len()
    }
}

// ============================================================================
// UTILITY FUNCTIONS
// ============================================================================

#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[wasm_bindgen]
pub fn benchmark_dtw(size: usize, iterations: usize) -> f64 {
    let temporal = TemporalCompare::new(None);
    let seq1: Vec<f64> = (0..size).map(|i| (i as f64).sin()).collect();
    let seq2: Vec<f64> = (0..size).map(|i| (i as f64).cos()).collect();

    let start = js_sys::Date::now();
    for _ in 0..iterations {
        temporal.dtw(&seq1, &seq2);
    }
    let elapsed = js_sys::Date::now() - start;

    elapsed / iterations as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dtw() {
        let temporal = TemporalCompare::new(None);
        let seq1 = vec![1.0, 2.0, 3.0];
        let seq2 = vec![1.0, 2.0, 3.0];
        assert_eq!(temporal.dtw(&seq1, &seq2), 0.0);
    }

    #[test]
    fn test_lcs() {
        let temporal = TemporalCompare::new(None);
        let seq1 = vec![1, 2, 3, 4];
        let seq2 = vec![1, 3, 4];
        assert_eq!(temporal.lcs(&seq1, &seq2), 3);
    }

    #[test]
    fn test_edit_distance() {
        let temporal = TemporalCompare::new(None);
        assert_eq!(temporal.edit_distance("kitten", "sitting"), 3);
    }
}
