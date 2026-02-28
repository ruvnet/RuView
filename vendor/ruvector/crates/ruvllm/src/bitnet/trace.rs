//! Structured JSONL Trace Output for BitNet Inference
//!
//! Provides structured tracing of inference decisions including MoE expert
//! routing, citation verification, refusal calibration, and coherence scoring.
//! All trace entries are serialized as JSONL (one JSON object per line) using
//! manual serialization (no serde dependency).
//!
//! ## Trace Fields
//!
//! Each `TraceEntry` captures per-token, per-layer diagnostics:
//! - **Routing**: Which experts were selected and whether they agree with a teacher
//! - **Citations**: Whether generated spans match source chunks
//! - **Refusal**: Whether the model correctly refused harmful prompts
//! - **Coherence**: Token-level coherence score
//! - **Stop Reason**: Why generation terminated
//!
//! ## Example
//!
//! ```rust,ignore
//! use ruvllm::bitnet::trace::{TraceWriter, TraceEntry, StopReason};
//!
//! let mut writer = TraceWriter::new(None);
//! writer.record(entry);
//! let jsonl = writer.to_jsonl();
//! ```

use std::collections::HashSet;
use std::path::PathBuf;

use crate::error::{Result, RuvLLMError};

// ============================================================================
// Trace Data Structures
// ============================================================================

/// Routing trace for a single token at a single layer.
///
/// Records which experts the model selected (top-K) and optionally
/// which experts a teacher model would have selected, enabling
/// routing agreement evaluation.
pub struct RoutingTrace {
    /// Expert indices selected by the student model (top-K)
    pub topk_expert_ids: Vec<usize>,
    /// Corresponding softmax weights for selected experts
    pub topk_weights: Vec<f32>,
    /// Expert indices from teacher model (if available)
    pub teacher_expert_ids: Option<Vec<usize>>,
    /// Corresponding teacher weights (if available)
    pub teacher_weights: Option<Vec<f32>>,
    /// Whether student and teacher selected the same expert set
    pub agreement: bool,
}

/// Citation trace for a single generated span.
///
/// Records whether a generated text span can be traced back to a
/// source chunk, with Jaccard similarity as a quality metric.
pub struct CitationTrace {
    /// Source chunk identifier
    pub chunk_id: String,
    /// Generated text span
    pub span: String,
    /// Whether the citation was validated
    pub valid: bool,
    /// Word-level Jaccard similarity between span and source
    pub jaccard_score: f32,
}

/// Refusal calibration trace.
///
/// Records whether the model should have refused a prompt,
/// whether it actually did, and whether the decision was correct.
pub struct RefusalTrace {
    /// Ground truth: should the model refuse this prompt?
    pub should_refuse: bool,
    /// Model behavior: did the model actually refuse?
    pub did_refuse: bool,
    /// Whether the model's refusal decision matched ground truth
    pub correct: bool,
}

/// Reason why generation stopped.
pub enum StopReason {
    /// End-of-sequence token generated
    Eos,
    /// Maximum generation length reached
    MaxLength,
    /// Model refused to generate (safety)
    Refusal,
    /// Coherence score dropped below threshold
    LowCoherence,
    /// An error occurred during generation
    Error(String),
}

/// A single trace entry capturing per-token, per-layer diagnostics.
pub struct TraceEntry {
    /// Unique identifier for the prompt being traced
    pub prompt_id: String,
    /// Token position in the generated sequence
    pub token_idx: usize,
    /// Transformer layer index
    pub layer_idx: usize,
    /// Expert routing diagnostics
    pub routing: RoutingTrace,
    /// Citation verification results
    pub citations: Vec<CitationTrace>,
    /// Refusal calibration result
    pub refusal: RefusalTrace,
    /// Token-level coherence score (0.0 to 1.0)
    pub coherence_score: f32,
    /// Why generation stopped at this token (if applicable)
    pub stop_reason: StopReason,
    /// Timestamp in milliseconds since epoch
    pub timestamp_ms: u64,
}

// ============================================================================
// Manual JSON Serialization
// ============================================================================

/// Escape a string for JSON output.
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

/// Format a Vec<usize> as a JSON array string.
fn json_usize_array(v: &[usize]) -> String {
    let parts: Vec<String> = v.iter().map(|x| x.to_string()).collect();
    format!("[{}]", parts.join(","))
}

/// Format a Vec<f32> as a JSON array string.
fn json_f32_array(v: &[f32]) -> String {
    let parts: Vec<String> = v.iter().map(|x| format!("{:.6}", x)).collect();
    format!("[{}]", parts.join(","))
}

impl RoutingTrace {
    /// Serialize to a JSON object string.
    pub fn to_json(&self) -> String {
        let teacher_ids = match &self.teacher_expert_ids {
            Some(ids) => json_usize_array(ids),
            None => "null".to_string(),
        };
        let teacher_wts = match &self.teacher_weights {
            Some(wts) => json_f32_array(wts),
            None => "null".to_string(),
        };
        format!(
            "{{\"topk_expert_ids\":{},\"topk_weights\":{},\"teacher_expert_ids\":{},\"teacher_weights\":{},\"agreement\":{}}}",
            json_usize_array(&self.topk_expert_ids),
            json_f32_array(&self.topk_weights),
            teacher_ids,
            teacher_wts,
            self.agreement,
        )
    }
}

impl CitationTrace {
    /// Serialize to a JSON object string.
    pub fn to_json(&self) -> String {
        format!(
            "{{\"chunk_id\":\"{}\",\"span\":\"{}\",\"valid\":{},\"jaccard_score\":{:.6}}}",
            json_escape(&self.chunk_id),
            json_escape(&self.span),
            self.valid,
            self.jaccard_score,
        )
    }
}

impl RefusalTrace {
    /// Serialize to a JSON object string.
    pub fn to_json(&self) -> String {
        format!(
            "{{\"should_refuse\":{},\"did_refuse\":{},\"correct\":{}}}",
            self.should_refuse, self.did_refuse, self.correct,
        )
    }
}

impl StopReason {
    /// Serialize to a JSON string value.
    pub fn to_json(&self) -> String {
        match self {
            StopReason::Eos => "\"eos\"".to_string(),
            StopReason::MaxLength => "\"max_length\"".to_string(),
            StopReason::Refusal => "\"refusal\"".to_string(),
            StopReason::LowCoherence => "\"low_coherence\"".to_string(),
            StopReason::Error(msg) => format!("\"error:{}\"", json_escape(msg)),
        }
    }
}

impl TraceEntry {
    /// Serialize to a JSON object string.
    pub fn to_json(&self) -> String {
        let citations_json: Vec<String> = self.citations.iter().map(|c| c.to_json()).collect();
        format!(
            "{{\"prompt_id\":\"{}\",\"token_idx\":{},\"layer_idx\":{},\"routing\":{},\"citations\":[{}],\"refusal\":{},\"coherence_score\":{:.6},\"stop_reason\":{},\"timestamp_ms\":{}}}",
            json_escape(&self.prompt_id),
            self.token_idx,
            self.layer_idx,
            self.routing.to_json(),
            citations_json.join(","),
            self.refusal.to_json(),
            self.coherence_score,
            self.stop_reason.to_json(),
            self.timestamp_ms,
        )
    }
}

// ============================================================================
// Trace Writer
// ============================================================================

/// Collects trace entries and writes them as JSONL.
///
/// Entries can be accumulated via `record()` and then flushed to a file
/// or retrieved as a JSONL string.
pub struct TraceWriter {
    entries: Vec<TraceEntry>,
    output_path: Option<PathBuf>,
}

impl TraceWriter {
    /// Create a new trace writer.
    ///
    /// If `output_path` is `Some`, `flush()` will write to that file.
    /// If `None`, entries are only available via `to_jsonl()`.
    pub fn new(output_path: Option<PathBuf>) -> Self {
        Self {
            entries: Vec::new(),
            output_path,
        }
    }

    /// Record a trace entry.
    pub fn record(&mut self, entry: TraceEntry) {
        self.entries.push(entry);
    }

    /// Flush all recorded entries to the output file (if configured).
    ///
    /// Each entry is written as a single JSON line. The file is
    /// overwritten on each flush.
    pub fn flush(&mut self) -> Result<()> {
        let path = match &self.output_path {
            Some(p) => p.clone(),
            None => {
                return Err(RuvLLMError::Config(
                    "No output path configured for trace writer".to_string(),
                ));
            }
        };

        let jsonl = self.to_jsonl();
        std::fs::write(&path, jsonl.as_bytes())
            .map_err(|e| RuvLLMError::Model(format!("Failed to write trace file: {}", e)))?;

        Ok(())
    }

    /// Convert all recorded entries to a JSONL string.
    ///
    /// Each entry is one line of valid JSON, separated by newlines.
    pub fn to_jsonl(&self) -> String {
        let lines: Vec<String> = self.entries.iter().map(|e| e.to_json()).collect();
        if lines.is_empty() {
            return String::new();
        }
        let mut result = lines.join("\n");
        result.push('\n');
        result
    }

    /// Get a reference to the recorded entries.
    pub fn entries(&self) -> &[TraceEntry] {
        &self.entries
    }

    /// Clear all recorded entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Compute word-level Jaccard similarity between two strings.
///
/// Splits both strings on whitespace, computes the Jaccard index:
/// `|A intersect B| / |A union B|`
///
/// # Arguments
///
/// * `a` - First string
/// * `b` - Second string
///
/// # Returns
///
/// Jaccard similarity in [0.0, 1.0]. Returns 1.0 if both strings are empty.
pub fn jaccard_similarity(a: &str, b: &str) -> f32 {
    let set_a: HashSet<&str> = a.split_whitespace().collect();
    let set_b: HashSet<&str> = b.split_whitespace().collect();

    if set_a.is_empty() && set_b.is_empty() {
        return 1.0;
    }

    let intersection = set_a.intersection(&set_b).count();
    let union = set_a.union(&set_b).count();

    if union == 0 {
        return 1.0;
    }

    intersection as f32 / union as f32
}

/// Check whether model and teacher routing agree (same set of expert IDs).
///
/// Returns true if both slices contain the same set of expert indices,
/// regardless of order.
///
/// # Arguments
///
/// * `model` - Expert indices selected by the student model
/// * `teacher` - Expert indices selected by the teacher model
pub fn check_routing_agreement(model: &[usize], teacher: &[usize]) -> bool {
    let model_set: HashSet<usize> = model.iter().copied().collect();
    let teacher_set: HashSet<usize> = teacher.iter().copied().collect();
    model_set == teacher_set
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create a minimal trace entry for testing.
    fn make_entry(prompt_id: &str, token_idx: usize, layer_idx: usize) -> TraceEntry {
        TraceEntry {
            prompt_id: prompt_id.to_string(),
            token_idx,
            layer_idx,
            routing: RoutingTrace {
                topk_expert_ids: vec![0, 2],
                topk_weights: vec![0.6, 0.4],
                teacher_expert_ids: Some(vec![0, 2]),
                teacher_weights: Some(vec![0.55, 0.45]),
                agreement: true,
            },
            citations: vec![CitationTrace {
                chunk_id: "doc-1".to_string(),
                span: "the quick fox".to_string(),
                valid: true,
                jaccard_score: 0.85,
            }],
            refusal: RefusalTrace {
                should_refuse: false,
                did_refuse: false,
                correct: true,
            },
            coherence_score: 0.92,
            stop_reason: StopReason::Eos,
            timestamp_ms: 1700000000000,
        }
    }

    #[test]
    fn test_json_serialization_valid() {
        let entry = make_entry("prompt-1", 0, 0);
        let json = entry.to_json();

        // Should start with { and end with }
        assert!(json.starts_with('{'), "JSON should start with {{");
        assert!(json.ends_with('}'), "JSON should end with }}");

        // Should contain key fields
        assert!(json.contains("\"prompt_id\":\"prompt-1\""));
        assert!(json.contains("\"token_idx\":0"));
        assert!(json.contains("\"layer_idx\":0"));
        assert!(json.contains("\"coherence_score\":"));
        assert!(json.contains("\"stop_reason\":\"eos\""));
    }

    #[test]
    fn test_jsonl_one_per_line() {
        let mut writer = TraceWriter::new(None);
        writer.record(make_entry("p1", 0, 0));
        writer.record(make_entry("p1", 1, 0));
        writer.record(make_entry("p2", 0, 0));

        let jsonl = writer.to_jsonl();
        let lines: Vec<&str> = jsonl.trim_end().split('\n').collect();
        assert_eq!(lines.len(), 3, "JSONL should have 3 lines for 3 entries");

        // Each line should be valid JSON (starts with {, ends with })
        for (i, line) in lines.iter().enumerate() {
            assert!(
                line.starts_with('{') && line.ends_with('}'),
                "Line {} is not valid JSON: {}",
                i,
                line
            );
        }
    }

    #[test]
    fn test_jaccard_identical() {
        let score = jaccard_similarity("the quick brown fox", "the quick brown fox");
        assert!(
            (score - 1.0).abs() < 1e-6,
            "Identical strings should have Jaccard = 1.0, got {}",
            score
        );
    }

    #[test]
    fn test_jaccard_disjoint() {
        let score = jaccard_similarity("alpha beta gamma", "delta epsilon zeta");
        assert!(
            score.abs() < 1e-6,
            "Disjoint strings should have Jaccard = 0.0, got {}",
            score
        );
    }

    #[test]
    fn test_jaccard_partial() {
        // "the quick" and "the slow" share "the" out of {"the", "quick", "slow"}
        let score = jaccard_similarity("the quick", "the slow");
        let expected = 1.0 / 3.0; // intersection=1, union=3
        assert!(
            (score - expected).abs() < 1e-6,
            "Partial overlap: expected {}, got {}",
            expected,
            score
        );
    }

    #[test]
    fn test_routing_agreement_same() {
        assert!(
            check_routing_agreement(&[0, 2, 5], &[5, 0, 2]),
            "Same expert set (different order) should agree"
        );
    }

    #[test]
    fn test_routing_agreement_different() {
        assert!(
            !check_routing_agreement(&[0, 2], &[0, 3]),
            "Different expert sets should not agree"
        );
    }

    #[test]
    fn test_flush_and_readback() {
        let dir = std::env::temp_dir();
        let path = dir.join("bitnet_trace_test.jsonl");

        let mut writer = TraceWriter::new(Some(path.clone()));
        writer.record(make_entry("flush-test", 0, 0));
        writer.record(make_entry("flush-test", 1, 1));
        writer.flush().unwrap();

        let contents = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = contents.trim_end().split('\n').collect();
        assert_eq!(lines.len(), 2, "Flushed file should have 2 lines");

        for line in &lines {
            assert!(line.starts_with('{') && line.ends_with('}'));
        }

        // Cleanup
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_stop_reason_serialization() {
        assert_eq!(StopReason::Eos.to_json(), "\"eos\"");
        assert_eq!(StopReason::MaxLength.to_json(), "\"max_length\"");
        assert_eq!(StopReason::Refusal.to_json(), "\"refusal\"");
        assert_eq!(StopReason::LowCoherence.to_json(), "\"low_coherence\"");

        let error_json = StopReason::Error("timeout".to_string()).to_json();
        assert_eq!(error_json, "\"error:timeout\"");
    }

    #[test]
    fn test_clear_entries() {
        let mut writer = TraceWriter::new(None);
        writer.record(make_entry("p1", 0, 0));
        assert_eq!(writer.entries().len(), 1);
        writer.clear();
        assert_eq!(writer.entries().len(), 0);
        assert_eq!(writer.to_jsonl(), "");
    }

    #[test]
    fn test_json_escape_special_chars() {
        let entry = TraceEntry {
            prompt_id: "test\"with\\special\nnewline".to_string(),
            token_idx: 0,
            layer_idx: 0,
            routing: RoutingTrace {
                topk_expert_ids: vec![],
                topk_weights: vec![],
                teacher_expert_ids: None,
                teacher_weights: None,
                agreement: false,
            },
            citations: vec![],
            refusal: RefusalTrace {
                should_refuse: false,
                did_refuse: false,
                correct: true,
            },
            coherence_score: 0.0,
            stop_reason: StopReason::Eos,
            timestamp_ms: 0,
        };

        let json = entry.to_json();
        // The escaped prompt_id should not contain raw quotes or newlines
        assert!(!json.contains("test\"with"), "Raw quote should be escaped");
        assert!(
            json.contains("test\\\"with"),
            "Quote should be escaped as \\\""
        );
        assert!(json.contains("\\n"), "Newline should be escaped as \\n");
    }
}
