//! Witness bundle builder, parser, scorecard aggregator, and governance
//! enforcement for ADR-035 capability reports.
//!
//! A witness bundle is the atomic proof unit: one task execution, fully
//! captured, signed, and replayable. A scorecard aggregates bundles into
//! a capability report.

use std::time::{SystemTime, UNIX_EPOCH};

use rvf_types::witness::*;

use crate::seed_crypto;

/// Errors specific to witness operations.
#[derive(Debug)]
pub enum WitnessError {
    /// Header parse or validation failure.
    InvalidHeader(rvf_types::RvfError),
    /// Section extends beyond payload.
    SectionOverflow { tag: u16, offset: usize },
    /// Signature verification failed.
    SignatureInvalid,
    /// Policy violation detected.
    PolicyViolation(String),
    /// Missing required section.
    MissingSection(&'static str),
    /// Bundle too large.
    TooLarge { size: usize },
}

impl From<rvf_types::RvfError> for WitnessError {
    fn from(e: rvf_types::RvfError) -> Self {
        WitnessError::InvalidHeader(e)
    }
}

impl core::fmt::Display for WitnessError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            WitnessError::InvalidHeader(e) => write!(f, "invalid header: {e}"),
            WitnessError::SectionOverflow { tag, offset } => {
                write!(f, "section 0x{tag:04X} overflows at offset {offset}")
            }
            WitnessError::SignatureInvalid => write!(f, "signature verification failed"),
            WitnessError::PolicyViolation(msg) => write!(f, "policy violation: {msg}"),
            WitnessError::MissingSection(s) => write!(f, "missing section: {s}"),
            WitnessError::TooLarge { size } => write!(f, "bundle too large: {size} bytes"),
        }
    }
}

// ---------------------------------------------------------------------------
// Governance policy
// ---------------------------------------------------------------------------

/// A governance policy that constrains what actions are allowed.
#[derive(Clone, Debug)]
pub struct GovernancePolicy {
    /// The governance mode.
    pub mode: GovernanceMode,
    /// Allowed tool names (empty = all allowed in Autonomous mode).
    pub allowed_tools: Vec<String>,
    /// Denied tool names (checked first).
    pub denied_tools: Vec<String>,
    /// Maximum cost in microdollars before requiring confirmation.
    pub max_cost_microdollars: u32,
    /// Maximum tool calls before requiring confirmation.
    pub max_tool_calls: u16,
}

impl GovernancePolicy {
    /// Create a restricted policy (read-only).
    pub fn restricted() -> Self {
        Self {
            mode: GovernanceMode::Restricted,
            allowed_tools: vec![
                "Read".into(),
                "Glob".into(),
                "Grep".into(),
                "WebFetch".into(),
                "WebSearch".into(),
            ],
            denied_tools: vec!["Bash".into(), "Write".into(), "Edit".into()],
            max_cost_microdollars: 10_000, // $0.01
            max_tool_calls: 50,
        }
    }

    /// Create an approved policy (writes with gates).
    pub fn approved() -> Self {
        Self {
            mode: GovernanceMode::Approved,
            allowed_tools: Vec::new(), // all allowed but gated
            denied_tools: Vec::new(),
            max_cost_microdollars: 100_000, // $0.10
            max_tool_calls: 200,
        }
    }

    /// Create an autonomous policy (bounded authority).
    pub fn autonomous() -> Self {
        Self {
            mode: GovernanceMode::Autonomous,
            allowed_tools: Vec::new(),
            denied_tools: Vec::new(),
            max_cost_microdollars: 1_000_000, // $1.00
            max_tool_calls: 500,
        }
    }

    /// Check if a tool call is allowed under this policy.
    pub fn check_tool(&self, tool: &str) -> PolicyCheck {
        // Deny list takes priority.
        if self.denied_tools.iter().any(|t| t == tool) {
            return PolicyCheck::Denied;
        }

        match self.mode {
            GovernanceMode::Restricted => {
                if self.allowed_tools.iter().any(|t| t == tool) {
                    PolicyCheck::Allowed
                } else {
                    PolicyCheck::Denied
                }
            }
            GovernanceMode::Approved => PolicyCheck::Confirmed,
            GovernanceMode::Autonomous => PolicyCheck::Allowed,
        }
    }

    /// Compute the SHA-256 policy hash (truncated to 8 bytes).
    pub fn hash(&self) -> [u8; 8] {
        // Hash the mode + tool lists as a deterministic string.
        let mut policy_str = format!("mode={}", self.mode as u8);
        for t in &self.allowed_tools {
            policy_str.push_str(&format!("+{t}"));
        }
        for t in &self.denied_tools {
            policy_str.push_str(&format!("-{t}"));
        }
        policy_str.push_str(&format!(
            "|cost={}|calls={}",
            self.max_cost_microdollars, self.max_tool_calls
        ));
        seed_crypto::seed_content_hash(policy_str.as_bytes())
    }
}

// ---------------------------------------------------------------------------
// Witness builder
// ---------------------------------------------------------------------------

/// Builder for constructing a witness bundle.
#[derive(Clone, Debug)]
pub struct WitnessBuilder {
    /// Task identifier.
    pub task_id: [u8; 16],
    /// Governance policy used.
    pub policy: GovernancePolicy,
    /// Task outcome.
    pub outcome: TaskOutcome,
    /// Tool call entries.
    pub trace: Vec<ToolCallEntry>,
    /// Spec / prompt text.
    pub spec: Option<Vec<u8>>,
    /// Plan graph.
    pub plan: Option<Vec<u8>>,
    /// Code diff.
    pub diff: Option<Vec<u8>>,
    /// Test log.
    pub test_log: Option<Vec<u8>>,
    /// Postmortem.
    pub postmortem: Option<Vec<u8>>,
    /// Accumulated cost.
    total_cost_microdollars: u32,
    /// Accumulated latency.
    total_latency_ms: u32,
    /// Accumulated tokens.
    total_tokens: u32,
    /// Accumulated retries.
    retry_count: u16,
    /// Policy violations recorded.
    pub policy_violations: Vec<String>,
    /// Rollback events recorded.
    pub rollback_count: u32,
}

impl WitnessBuilder {
    /// Create a new witness builder.
    pub fn new(task_id: [u8; 16], policy: GovernancePolicy) -> Self {
        Self {
            task_id,
            policy,
            outcome: TaskOutcome::Skipped,
            trace: Vec::new(),
            spec: None,
            plan: None,
            diff: None,
            test_log: None,
            postmortem: None,
            total_cost_microdollars: 0,
            total_latency_ms: 0,
            total_tokens: 0,
            retry_count: 0,
            policy_violations: Vec::new(),
            rollback_count: 0,
        }
    }

    /// Set the spec / prompt.
    pub fn with_spec(mut self, spec: &[u8]) -> Self {
        self.spec = Some(spec.to_vec());
        self
    }

    /// Set the plan.
    pub fn with_plan(mut self, plan: &[u8]) -> Self {
        self.plan = Some(plan.to_vec());
        self
    }

    /// Set the diff.
    pub fn with_diff(mut self, diff: &[u8]) -> Self {
        self.diff = Some(diff.to_vec());
        self
    }

    /// Set the test log.
    pub fn with_test_log(mut self, log: &[u8]) -> Self {
        self.test_log = Some(log.to_vec());
        self
    }

    /// Set the postmortem.
    pub fn with_postmortem(mut self, pm: &[u8]) -> Self {
        self.postmortem = Some(pm.to_vec());
        self
    }

    /// Set the outcome.
    pub fn with_outcome(mut self, outcome: TaskOutcome) -> Self {
        self.outcome = outcome;
        self
    }

    /// Record a tool call. Enforces governance policy.
    pub fn record_tool_call(&mut self, entry: ToolCallEntry) -> PolicyCheck {
        let tool_name = core::str::from_utf8(&entry.action).unwrap_or("");
        let check = self.policy.check_tool(tool_name);

        if check == PolicyCheck::Denied {
            self.policy_violations
                .push(format!("denied tool: {tool_name}"));
        }

        self.total_cost_microdollars = self
            .total_cost_microdollars
            .saturating_add(entry.cost_microdollars);
        self.total_latency_ms = self.total_latency_ms.saturating_add(entry.latency_ms);
        self.total_tokens = self.total_tokens.saturating_add(entry.tokens);

        let mut recorded = entry;
        recorded.policy_check = check;
        self.trace.push(recorded);

        // Check cost budget.
        if self.total_cost_microdollars > self.policy.max_cost_microdollars {
            self.policy_violations.push(format!(
                "cost budget exceeded: {} > {}",
                self.total_cost_microdollars, self.policy.max_cost_microdollars
            ));
        }

        // Check tool call budget.
        if self.trace.len() as u16 > self.policy.max_tool_calls {
            self.policy_violations.push(format!(
                "tool call budget exceeded: {} > {}",
                self.trace.len(),
                self.policy.max_tool_calls
            ));
        }

        check
    }

    /// Record a retry.
    pub fn record_retry(&mut self) {
        self.retry_count = self.retry_count.saturating_add(1);
    }

    /// Record a rollback.
    pub fn record_rollback(&mut self) {
        self.rollback_count += 1;
    }

    /// Build the TLV payload sections.
    fn build_sections(&self) -> (Vec<u8>, u16, u16) {
        let mut payload = Vec::new();
        let mut section_count: u16 = 0;
        let mut flags: u16 = 0;

        // Helper: write one TLV section.
        let mut write_section = |tag: u16, flag: u16, data: &[u8]| {
            payload.extend_from_slice(&tag.to_le_bytes());
            payload.extend_from_slice(&(data.len() as u32).to_le_bytes());
            payload.extend_from_slice(data);
            section_count += 1;
            flags |= flag;
        };

        if let Some(ref spec) = self.spec {
            write_section(WIT_TAG_SPEC, WIT_HAS_SPEC, spec);
        }
        if let Some(ref plan) = self.plan {
            write_section(WIT_TAG_PLAN, WIT_HAS_PLAN, plan);
        }

        // Trace: serialize all tool call entries.
        if !self.trace.is_empty() {
            let mut trace_buf = Vec::new();
            for entry in &self.trace {
                trace_buf.extend_from_slice(&entry.to_bytes());
            }
            write_section(WIT_TAG_TRACE, WIT_HAS_TRACE, &trace_buf);
        }

        if let Some(ref diff) = self.diff {
            write_section(WIT_TAG_DIFF, WIT_HAS_DIFF, diff);
        }
        if let Some(ref log) = self.test_log {
            write_section(WIT_TAG_TEST_LOG, WIT_HAS_TEST_LOG, log);
        }
        if let Some(ref pm) = self.postmortem {
            write_section(WIT_TAG_POSTMORTEM, WIT_HAS_POSTMORTEM, pm);
        }

        (payload, section_count, flags)
    }

    /// Build an unsigned witness bundle.
    pub fn build(self) -> Result<(Vec<u8>, WitnessHeader), WitnessError> {
        let (sections, section_count, flags) = self.build_sections();

        let total_bundle_size = WITNESS_HEADER_SIZE + sections.len();

        let created_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        let header = WitnessHeader {
            magic: WITNESS_MAGIC,
            version: 1,
            flags,
            task_id: self.task_id,
            policy_hash: self.policy.hash(),
            created_ns,
            outcome: self.outcome as u8,
            governance_mode: self.policy.mode as u8,
            tool_call_count: self.trace.len() as u16,
            total_cost_microdollars: self.total_cost_microdollars,
            total_latency_ms: self.total_latency_ms,
            total_tokens: self.total_tokens,
            retry_count: self.retry_count,
            section_count,
            total_bundle_size: total_bundle_size as u32,
        };

        let mut payload = Vec::with_capacity(total_bundle_size);
        payload.extend_from_slice(&header.to_bytes());
        payload.extend_from_slice(&sections);
        debug_assert_eq!(payload.len(), total_bundle_size);

        Ok((payload, header))
    }

    /// Build and sign with HMAC-SHA256.
    pub fn build_and_sign(
        self,
        signing_key: &[u8],
    ) -> Result<(Vec<u8>, WitnessHeader), WitnessError> {
        let (sections, section_count, mut flags) = self.build_sections();
        flags |= WIT_SIGNED;

        let sig_len: usize = 32; // HMAC-SHA256
        let total_bundle_size = WITNESS_HEADER_SIZE + sections.len() + sig_len;

        let created_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        let header = WitnessHeader {
            magic: WITNESS_MAGIC,
            version: 1,
            flags,
            task_id: self.task_id,
            policy_hash: self.policy.hash(),
            created_ns,
            outcome: self.outcome as u8,
            governance_mode: self.policy.mode as u8,
            tool_call_count: self.trace.len() as u16,
            total_cost_microdollars: self.total_cost_microdollars,
            total_latency_ms: self.total_latency_ms,
            total_tokens: self.total_tokens,
            retry_count: self.retry_count,
            section_count,
            total_bundle_size: total_bundle_size as u32,
        };

        // Build unsigned payload.
        let unsigned_size = WITNESS_HEADER_SIZE + sections.len();
        let mut unsigned = Vec::with_capacity(unsigned_size);
        unsigned.extend_from_slice(&header.to_bytes());
        unsigned.extend_from_slice(&sections);

        // Sign.
        let sig = seed_crypto::sign_seed(signing_key, &unsigned);

        let mut payload = unsigned;
        payload.extend_from_slice(&sig);
        debug_assert_eq!(payload.len(), total_bundle_size);

        Ok((payload, header))
    }
}

// ---------------------------------------------------------------------------
// Parsed witness bundle
// ---------------------------------------------------------------------------

/// A parsed witness bundle with zero-copy references to sections.
#[derive(Debug)]
pub struct ParsedWitness<'a> {
    /// The parsed header.
    pub header: WitnessHeader,
    /// Spec section bytes.
    pub spec: Option<&'a [u8]>,
    /// Plan section bytes.
    pub plan: Option<&'a [u8]>,
    /// Tool call trace bytes (contains serialized ToolCallEntry array).
    pub trace: Option<&'a [u8]>,
    /// Diff section bytes.
    pub diff: Option<&'a [u8]>,
    /// Test log section bytes.
    pub test_log: Option<&'a [u8]>,
    /// Postmortem section bytes.
    pub postmortem: Option<&'a [u8]>,
    /// Signature bytes (if signed).
    pub signature: Option<&'a [u8]>,
}

impl<'a> ParsedWitness<'a> {
    /// Parse a witness bundle from bytes.
    pub fn parse(data: &'a [u8]) -> Result<Self, WitnessError> {
        let header = WitnessHeader::from_bytes(data)?;

        if (header.total_bundle_size as usize) > data.len() {
            return Err(WitnessError::InvalidHeader(
                rvf_types::RvfError::SizeMismatch {
                    expected: header.total_bundle_size as usize,
                    got: data.len(),
                },
            ));
        }

        let mut spec = None;
        let mut plan = None;
        let mut trace = None;
        let mut diff = None;
        let mut test_log = None;
        let mut postmortem = None;

        // Determine where signature starts (if signed).
        let sig_len = if header.is_signed() { 32usize } else { 0 };
        let sections_end = header.total_bundle_size as usize - sig_len;

        // Parse TLV sections.
        let mut pos = WITNESS_HEADER_SIZE;
        while pos + 6 <= sections_end {
            let tag = u16::from_le_bytes([data[pos], data[pos + 1]]);
            let length =
                u32::from_le_bytes([data[pos + 2], data[pos + 3], data[pos + 4], data[pos + 5]])
                    as usize;
            pos += 6;

            if pos + length > sections_end {
                return Err(WitnessError::SectionOverflow { tag, offset: pos });
            }

            let value = &data[pos..pos + length];
            match tag {
                WIT_TAG_SPEC => spec = Some(value),
                WIT_TAG_PLAN => plan = Some(value),
                WIT_TAG_TRACE => trace = Some(value),
                WIT_TAG_DIFF => diff = Some(value),
                WIT_TAG_TEST_LOG => test_log = Some(value),
                WIT_TAG_POSTMORTEM => postmortem = Some(value),
                _ => {} // forward-compat: ignore unknown tags
            }

            pos += length;
        }

        let signature = if header.is_signed() && sig_len > 0 {
            let sig_start = header.total_bundle_size as usize - sig_len;
            Some(&data[sig_start..header.total_bundle_size as usize])
        } else {
            None
        };

        Ok(ParsedWitness {
            header,
            spec,
            plan,
            trace,
            diff,
            test_log,
            postmortem,
            signature,
        })
    }

    /// Parse tool call entries from the trace section.
    pub fn parse_trace(&self) -> Vec<ToolCallEntry> {
        let data = match self.trace {
            Some(d) => d,
            None => return Vec::new(),
        };
        let mut entries = Vec::new();
        let mut pos = 0;
        while pos < data.len() {
            match ToolCallEntry::from_bytes(&data[pos..]) {
                Some((entry, consumed)) => {
                    entries.push(entry);
                    pos += consumed;
                }
                None => break,
            }
        }
        entries
    }

    /// Get the unsigned payload (everything before the signature).
    pub fn unsigned_payload<'b>(&self, full_data: &'b [u8]) -> Option<&'b [u8]> {
        if self.header.is_signed() {
            let end = self.header.total_bundle_size as usize - 32;
            Some(&full_data[..end])
        } else {
            None
        }
    }

    /// Verify the HMAC-SHA256 signature.
    pub fn verify_signature(&self, key: &[u8], full_data: &[u8]) -> Result<(), WitnessError> {
        let sig = self
            .signature
            .ok_or(WitnessError::MissingSection("signature"))?;
        let unsigned = self
            .unsigned_payload(full_data)
            .ok_or(WitnessError::MissingSection("unsigned payload"))?;
        if seed_crypto::verify_seed(key, unsigned, sig) {
            Ok(())
        } else {
            Err(WitnessError::SignatureInvalid)
        }
    }

    /// Full verification: magic + signature.
    pub fn verify_all(&self, key: &[u8], full_data: &[u8]) -> Result<(), WitnessError> {
        if !self.header.is_valid_magic() {
            return Err(WitnessError::InvalidHeader(rvf_types::RvfError::BadMagic {
                expected: WITNESS_MAGIC,
                got: self.header.magic,
            }));
        }
        if self.header.is_signed() {
            self.verify_signature(key, full_data)?;
        }
        Ok(())
    }

    /// Check evidence completeness: does this bundle have all required sections?
    pub fn evidence_complete(&self) -> bool {
        self.spec.is_some() && self.diff.is_some() && self.test_log.is_some()
    }
}

// ---------------------------------------------------------------------------
// Scorecard aggregator
// ---------------------------------------------------------------------------

/// Aggregates multiple witness bundles into a capability scorecard.
pub struct ScorecardBuilder {
    latencies: Vec<u32>,
    total_cost: u64,
    total_tokens: u64,
    total_retries: u32,
    solved: u32,
    failed: u32,
    skipped: u32,
    errors: u32,
    policy_violations: u32,
    rollback_count: u32,
    evidence_complete_count: u32,
    solved_count_for_evidence: u32,
}

impl ScorecardBuilder {
    /// Create a new scorecard builder.
    pub fn new() -> Self {
        Self {
            latencies: Vec::new(),
            total_cost: 0,
            total_tokens: 0,
            total_retries: 0,
            solved: 0,
            failed: 0,
            skipped: 0,
            errors: 0,
            policy_violations: 0,
            rollback_count: 0,
            evidence_complete_count: 0,
            solved_count_for_evidence: 0,
        }
    }

    /// Add a parsed witness bundle to the scorecard.
    pub fn add_witness(&mut self, parsed: &ParsedWitness<'_>, violations: u32, rollbacks: u32) {
        self.latencies.push(parsed.header.total_latency_ms);
        self.total_cost += parsed.header.total_cost_microdollars as u64;
        self.total_tokens += parsed.header.total_tokens as u64;
        self.total_retries += parsed.header.retry_count as u32;
        self.policy_violations += violations;
        self.rollback_count += rollbacks;

        match TaskOutcome::try_from(parsed.header.outcome) {
            Ok(TaskOutcome::Solved) => {
                self.solved += 1;
                self.solved_count_for_evidence += 1;
                if parsed.evidence_complete() {
                    self.evidence_complete_count += 1;
                }
            }
            Ok(TaskOutcome::Failed) => self.failed += 1,
            Ok(TaskOutcome::Skipped) => self.skipped += 1,
            Ok(TaskOutcome::Errored) => self.errors += 1,
            Err(_) => self.errors += 1,
        }
    }

    /// Finalize and produce the scorecard.
    pub fn finish(&mut self) -> Scorecard {
        let total = self.solved + self.failed + self.skipped + self.errors;

        // Sort latencies for percentiles.
        self.latencies.sort_unstable();
        let median = percentile(&self.latencies, 50);
        let p95 = percentile(&self.latencies, 95);

        let cost_per_solve = if self.solved > 0 {
            (self.total_cost / self.solved as u64) as u32
        } else {
            0
        };

        let solve_rate = if total > 0 {
            self.solved as f32 / total as f32
        } else {
            0.0
        };

        let evidence_coverage = if self.solved_count_for_evidence > 0 {
            self.evidence_complete_count as f32 / self.solved_count_for_evidence as f32
        } else {
            0.0
        };

        Scorecard {
            total_tasks: total,
            solved: self.solved,
            failed: self.failed,
            skipped: self.skipped,
            errors: self.errors,
            policy_violations: self.policy_violations,
            rollback_count: self.rollback_count,
            total_cost_microdollars: self.total_cost,
            median_latency_ms: median,
            p95_latency_ms: p95,
            total_tokens: self.total_tokens,
            total_retries: self.total_retries,
            evidence_coverage,
            cost_per_solve_microdollars: cost_per_solve,
            solve_rate,
        }
    }
}

impl Default for ScorecardBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute a percentile from a sorted slice.
fn percentile(sorted: &[u32], pct: usize) -> u32 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = (pct * sorted.len() / 100).min(sorted.len() - 1);
    sorted[idx]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_policy() -> GovernancePolicy {
        GovernancePolicy::autonomous()
    }

    fn make_entry(tool: &str, latency_ms: u32, cost: u32, tokens: u32) -> ToolCallEntry {
        ToolCallEntry {
            action: tool.as_bytes().to_vec(),
            args_hash: [0x11; 8],
            result_hash: [0x22; 8],
            latency_ms,
            cost_microdollars: cost,
            tokens,
            policy_check: PolicyCheck::Allowed,
        }
    }

    #[test]
    fn build_minimal_witness() {
        let builder =
            WitnessBuilder::new([0x01; 16], make_policy()).with_outcome(TaskOutcome::Solved);
        let (payload, header) = builder.build().unwrap();
        assert_eq!(header.magic, WITNESS_MAGIC);
        assert_eq!(payload.len(), WITNESS_HEADER_SIZE);
        assert_eq!(header.outcome, TaskOutcome::Solved as u8);
    }

    #[test]
    fn build_with_sections() {
        let builder = WitnessBuilder::new([0x02; 16], make_policy())
            .with_spec(b"fix authentication bug")
            .with_plan(b"1. read code\n2. fix bug\n3. test")
            .with_diff(b"--- a/src/auth.rs\n+++ b/src/auth.rs")
            .with_test_log(b"test auth::login ... ok")
            .with_outcome(TaskOutcome::Solved);
        let (payload, header) = builder.build().unwrap();
        assert!(payload.len() > WITNESS_HEADER_SIZE);
        assert_eq!(header.section_count, 4); // spec + plan + diff + test_log

        let parsed = ParsedWitness::parse(&payload).unwrap();
        assert_eq!(parsed.spec.unwrap(), b"fix authentication bug");
        assert_eq!(parsed.plan.unwrap(), b"1. read code\n2. fix bug\n3. test");
        assert!(parsed.evidence_complete());
    }

    #[test]
    fn build_with_trace() {
        let mut builder =
            WitnessBuilder::new([0x03; 16], make_policy()).with_outcome(TaskOutcome::Solved);

        builder.record_tool_call(make_entry("Read", 50, 100, 500));
        builder.record_tool_call(make_entry("Edit", 100, 200, 1000));
        builder.record_tool_call(make_entry("Bash", 2000, 0, 0));

        let (payload, header) = builder.build().unwrap();
        assert_eq!(header.tool_call_count, 3);
        assert_eq!(header.total_cost_microdollars, 300);
        assert_eq!(header.total_latency_ms, 2150);
        assert_eq!(header.total_tokens, 1500);

        let parsed = ParsedWitness::parse(&payload).unwrap();
        let entries = parsed.parse_trace();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].action, b"Read");
        assert_eq!(entries[1].action, b"Edit");
        assert_eq!(entries[2].action, b"Bash");
    }

    #[test]
    fn signed_round_trip() {
        let key = b"test-signing-key-for-witness-ok!";
        let builder = WitnessBuilder::new([0x04; 16], make_policy())
            .with_spec(b"test spec")
            .with_outcome(TaskOutcome::Solved);
        let (payload, header) = builder.build_and_sign(key).unwrap();
        assert!(header.is_signed());

        let parsed = ParsedWitness::parse(&payload).unwrap();
        parsed.verify_all(key, &payload).unwrap();
    }

    #[test]
    fn wrong_key_rejected() {
        let key = b"test-signing-key-for-witness-ok!";
        let builder = WitnessBuilder::new([0x05; 16], make_policy())
            .with_spec(b"test spec")
            .with_outcome(TaskOutcome::Solved);
        let (payload, _) = builder.build_and_sign(key).unwrap();

        let parsed = ParsedWitness::parse(&payload).unwrap();
        assert!(parsed
            .verify_signature(b"wrong-key-should-fail-immediate!", &payload)
            .is_err());
    }

    #[test]
    fn tampered_payload_fails() {
        let key = b"test-signing-key-for-witness-ok!";
        let builder = WitnessBuilder::new([0x06; 16], make_policy())
            .with_spec(b"test spec")
            .with_outcome(TaskOutcome::Solved);
        let (mut payload, _) = builder.build_and_sign(key).unwrap();

        // Tamper with spec section.
        payload[WITNESS_HEADER_SIZE + 10] ^= 0xFF;
        let parsed = ParsedWitness::parse(&payload).unwrap();
        assert!(parsed.verify_signature(key, &payload).is_err());
    }

    #[test]
    fn governance_restricted_denies_writes() {
        let policy = GovernancePolicy::restricted();
        assert_eq!(policy.check_tool("Read"), PolicyCheck::Allowed);
        assert_eq!(policy.check_tool("Glob"), PolicyCheck::Allowed);
        assert_eq!(policy.check_tool("Bash"), PolicyCheck::Denied);
        assert_eq!(policy.check_tool("Write"), PolicyCheck::Denied);
        assert_eq!(policy.check_tool("Edit"), PolicyCheck::Denied);
        assert_eq!(policy.check_tool("UnknownTool"), PolicyCheck::Denied);
    }

    #[test]
    fn governance_approved_gates_everything() {
        let policy = GovernancePolicy::approved();
        assert_eq!(policy.check_tool("Read"), PolicyCheck::Confirmed);
        assert_eq!(policy.check_tool("Bash"), PolicyCheck::Confirmed);
        assert_eq!(policy.check_tool("Edit"), PolicyCheck::Confirmed);
    }

    #[test]
    fn governance_autonomous_allows_all() {
        let policy = GovernancePolicy::autonomous();
        assert_eq!(policy.check_tool("Read"), PolicyCheck::Allowed);
        assert_eq!(policy.check_tool("Bash"), PolicyCheck::Allowed);
        assert_eq!(policy.check_tool("Edit"), PolicyCheck::Allowed);
    }

    #[test]
    fn policy_violation_recorded() {
        let policy = GovernancePolicy::restricted();
        let mut builder = WitnessBuilder::new([0x07; 16], policy).with_outcome(TaskOutcome::Failed);

        let check = builder.record_tool_call(make_entry("Bash", 100, 0, 0));
        assert_eq!(check, PolicyCheck::Denied);
        assert_eq!(builder.policy_violations.len(), 1);
        assert!(builder.policy_violations[0].contains("denied tool: Bash"));
    }

    #[test]
    fn cost_budget_violation() {
        let mut policy = GovernancePolicy::autonomous();
        policy.max_cost_microdollars = 500;
        let mut builder = WitnessBuilder::new([0x08; 16], policy).with_outcome(TaskOutcome::Solved);

        builder.record_tool_call(make_entry("Read", 50, 300, 100));
        assert!(builder.policy_violations.is_empty());

        builder.record_tool_call(make_entry("Edit", 50, 300, 100));
        assert_eq!(builder.policy_violations.len(), 1);
        assert!(builder.policy_violations[0].contains("cost budget exceeded"));
    }

    #[test]
    fn scorecard_basic() {
        let policy = make_policy();
        let key = b"test-signing-key-for-witness-ok!";

        let mut sc = ScorecardBuilder::new();

        // Solved task with evidence.
        let b1 = WitnessBuilder::new([0x01; 16], policy.clone())
            .with_spec(b"fix bug")
            .with_diff(b"diff")
            .with_test_log(b"ok")
            .with_outcome(TaskOutcome::Solved);
        let (p1, _) = b1.build_and_sign(key).unwrap();
        let w1 = ParsedWitness::parse(&p1).unwrap();
        sc.add_witness(&w1, 0, 0);

        // Failed task.
        let b2 = WitnessBuilder::new([0x02; 16], policy.clone())
            .with_spec(b"add feature")
            .with_outcome(TaskOutcome::Failed);
        let (p2, _) = b2.build_and_sign(key).unwrap();
        let w2 = ParsedWitness::parse(&p2).unwrap();
        sc.add_witness(&w2, 1, 0);

        // Solved task without full evidence.
        let b3 = WitnessBuilder::new([0x03; 16], policy.clone())
            .with_spec(b"refactor")
            .with_outcome(TaskOutcome::Solved);
        let (p3, _) = b3.build_and_sign(key).unwrap();
        let w3 = ParsedWitness::parse(&p3).unwrap();
        sc.add_witness(&w3, 0, 1);

        let card = sc.finish();
        assert_eq!(card.total_tasks, 3);
        assert_eq!(card.solved, 2);
        assert_eq!(card.failed, 1);
        assert_eq!(card.policy_violations, 1);
        assert_eq!(card.rollback_count, 1);
        assert!((card.solve_rate - 0.6667).abs() < 0.01);
        assert!((card.evidence_coverage - 0.5).abs() < 0.01); // 1/2 solved with full evidence
    }

    #[test]
    fn scorecard_empty() {
        let card = ScorecardBuilder::new().finish();
        assert_eq!(card.total_tasks, 0);
        assert_eq!(card.solve_rate, 0.0);
        assert_eq!(card.median_latency_ms, 0);
        assert_eq!(card.p95_latency_ms, 0);
    }

    #[test]
    fn policy_hash_deterministic() {
        let p1 = GovernancePolicy::restricted();
        let p2 = GovernancePolicy::restricted();
        assert_eq!(p1.hash(), p2.hash());

        let p3 = GovernancePolicy::autonomous();
        assert_ne!(p1.hash(), p3.hash());
    }

    #[test]
    fn witness_error_display() {
        let e = WitnessError::PolicyViolation("denied tool: Bash".into());
        assert!(format!("{e}").contains("denied tool: Bash"));

        let e2 = WitnessError::TooLarge { size: 99999 };
        assert!(format!("{e2}").contains("99999"));
    }
}
