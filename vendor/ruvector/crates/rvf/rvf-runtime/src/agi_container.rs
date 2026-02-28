//! AGI Cognitive Container builder and validator (ADR-036).
//!
//! Assembles a complete intelligence runtime into a single RVF artifact:
//! micro Linux kernel, Claude Code + Claude Flow configs, world model,
//! evaluation harness, witness chains, tool adapters, and policies.

use std::time::{SystemTime, UNIX_EPOCH};

use rvf_types::agi_container::*;

use crate::seed_crypto;

/// Builder for assembling an AGI cognitive container manifest.
///
/// The manifest is a META segment in the RVF file. Other segments
/// (KERNEL_SEG, WASM_SEG, VEC_SEG, etc.) are added through the
/// main RVF write path; this builder only handles the manifest.
#[derive(Clone, Debug)]
pub struct AgiContainerBuilder {
    /// Container UUID.
    pub container_id: [u8; 16],
    /// Build UUID.
    pub build_id: [u8; 16],
    /// Pinned model identifier string.
    pub model_id: Option<Vec<u8>>,
    /// Governance policy (binary).
    pub policy: Option<Vec<u8>>,
    /// Policy hash from ADR-035 GovernancePolicy.
    pub policy_hash: [u8; 8],
    /// Orchestrator config (Claude Code + Claude Flow).
    pub orchestrator_config: Option<Vec<u8>>,
    /// MCP tool adapter registry.
    pub tool_registry: Option<Vec<u8>>,
    /// Agent role prompts.
    pub agent_prompts: Option<Vec<u8>>,
    /// Evaluation task suite.
    pub eval_tasks: Option<Vec<u8>>,
    /// Grading rules.
    pub eval_graders: Option<Vec<u8>>,
    /// Skill library.
    pub skill_library: Option<Vec<u8>>,
    /// Replay script.
    pub replay_script: Option<Vec<u8>>,
    /// Kernel boot config.
    pub kernel_config: Option<Vec<u8>>,
    /// Network config.
    pub network_config: Option<Vec<u8>>,
    /// Coherence gate config.
    pub coherence_config: Option<Vec<u8>>,
    /// Project instructions (CLAUDE.md).
    pub project_instructions: Option<Vec<u8>>,
    /// Dependency snapshot.
    pub dependency_snapshot: Option<Vec<u8>>,
    /// Authority level and resource budget config.
    pub authority_config: Option<Vec<u8>>,
    /// Target domain profile.
    pub domain_profile: Option<Vec<u8>>,
    /// Segment inventory.
    pub segments: ContainerSegments,
    /// Extra flags to OR in.
    pub extra_flags: u16,
}

impl AgiContainerBuilder {
    /// Create a new builder with container and build IDs.
    pub fn new(container_id: [u8; 16], build_id: [u8; 16]) -> Self {
        Self {
            container_id,
            build_id,
            model_id: None,
            policy: None,
            policy_hash: [0; 8],
            orchestrator_config: None,
            tool_registry: None,
            agent_prompts: None,
            eval_tasks: None,
            eval_graders: None,
            skill_library: None,
            replay_script: None,
            kernel_config: None,
            network_config: None,
            coherence_config: None,
            project_instructions: None,
            dependency_snapshot: None,
            authority_config: None,
            domain_profile: None,
            segments: ContainerSegments::default(),
            extra_flags: 0,
        }
    }

    /// Pin the model to a specific version.
    pub fn with_model_id(mut self, model_id: &str) -> Self {
        self.model_id = Some(model_id.as_bytes().to_vec());
        self
    }

    /// Set the governance policy.
    pub fn with_policy(mut self, policy: &[u8], hash: [u8; 8]) -> Self {
        self.policy = Some(policy.to_vec());
        self.policy_hash = hash;
        self
    }

    /// Set the Claude Code + Claude Flow orchestrator config.
    pub fn with_orchestrator(mut self, config: &[u8]) -> Self {
        self.orchestrator_config = Some(config.to_vec());
        self.extra_flags |= AGI_HAS_ORCHESTRATOR;
        self
    }

    /// Set the MCP tool adapter registry.
    pub fn with_tool_registry(mut self, registry: &[u8]) -> Self {
        self.tool_registry = Some(registry.to_vec());
        self.extra_flags |= AGI_HAS_TOOLS;
        self
    }

    /// Set agent role prompts.
    pub fn with_agent_prompts(mut self, prompts: &[u8]) -> Self {
        self.agent_prompts = Some(prompts.to_vec());
        self
    }

    /// Set the evaluation task suite.
    pub fn with_eval_tasks(mut self, tasks: &[u8]) -> Self {
        self.eval_tasks = Some(tasks.to_vec());
        self.extra_flags |= AGI_HAS_EVAL;
        self
    }

    /// Set the grading rules.
    pub fn with_eval_graders(mut self, graders: &[u8]) -> Self {
        self.eval_graders = Some(graders.to_vec());
        self
    }

    /// Set the promoted skill library.
    pub fn with_skill_library(mut self, skills: &[u8]) -> Self {
        self.skill_library = Some(skills.to_vec());
        self.extra_flags |= AGI_HAS_SKILLS;
        self
    }

    /// Set the replay automation script.
    pub fn with_replay_script(mut self, script: &[u8]) -> Self {
        self.replay_script = Some(script.to_vec());
        self.extra_flags |= AGI_REPLAY_CAPABLE;
        self
    }

    /// Set the kernel boot configuration.
    pub fn with_kernel_config(mut self, config: &[u8]) -> Self {
        self.kernel_config = Some(config.to_vec());
        self
    }

    /// Set the network configuration.
    pub fn with_network_config(mut self, config: &[u8]) -> Self {
        self.network_config = Some(config.to_vec());
        self
    }

    /// Set the coherence gate configuration.
    pub fn with_coherence_config(mut self, config: &[u8]) -> Self {
        self.coherence_config = Some(config.to_vec());
        self.extra_flags |= AGI_HAS_COHERENCE_GATES;
        self
    }

    /// Set the project instructions (CLAUDE.md content).
    pub fn with_project_instructions(mut self, instructions: &[u8]) -> Self {
        self.project_instructions = Some(instructions.to_vec());
        self
    }

    /// Set the dependency snapshot.
    pub fn with_dependency_snapshot(mut self, snapshot: &[u8]) -> Self {
        self.dependency_snapshot = Some(snapshot.to_vec());
        self
    }

    /// Set authority and resource budget configuration.
    pub fn with_authority_config(mut self, config: &[u8]) -> Self {
        self.authority_config = Some(config.to_vec());
        self
    }

    /// Set the target domain profile.
    pub fn with_domain_profile(mut self, profile: &[u8]) -> Self {
        self.domain_profile = Some(profile.to_vec());
        self
    }

    /// Mark offline capability.
    pub fn offline_capable(mut self) -> Self {
        self.extra_flags |= AGI_OFFLINE_CAPABLE;
        self
    }

    /// Declare the segment inventory.
    pub fn with_segments(mut self, segments: ContainerSegments) -> Self {
        self.segments = segments;
        self
    }

    /// Build the manifest TLV payload (sections only, no header).
    fn build_sections(&self) -> Vec<u8> {
        let mut payload = Vec::new();

        let mut write_section = |tag: u16, data: &[u8]| {
            payload.extend_from_slice(&tag.to_le_bytes());
            payload.extend_from_slice(&(data.len() as u32).to_le_bytes());
            payload.extend_from_slice(data);
        };

        write_section(AGI_TAG_CONTAINER_ID, &self.container_id);
        write_section(AGI_TAG_BUILD_ID, &self.build_id);

        if let Some(ref mid) = self.model_id {
            write_section(AGI_TAG_MODEL_ID, mid);
        }
        if let Some(ref p) = self.policy {
            write_section(AGI_TAG_POLICY, p);
        }
        if let Some(ref oc) = self.orchestrator_config {
            write_section(AGI_TAG_ORCHESTRATOR, oc);
        }
        if let Some(ref tr) = self.tool_registry {
            write_section(AGI_TAG_TOOL_REGISTRY, tr);
        }
        if let Some(ref ap) = self.agent_prompts {
            write_section(AGI_TAG_AGENT_PROMPTS, ap);
        }
        if let Some(ref et) = self.eval_tasks {
            write_section(AGI_TAG_EVAL_TASKS, et);
        }
        if let Some(ref eg) = self.eval_graders {
            write_section(AGI_TAG_EVAL_GRADERS, eg);
        }
        if let Some(ref sl) = self.skill_library {
            write_section(AGI_TAG_SKILL_LIBRARY, sl);
        }
        if let Some(ref rs) = self.replay_script {
            write_section(AGI_TAG_REPLAY_SCRIPT, rs);
        }
        if let Some(ref kc) = self.kernel_config {
            write_section(AGI_TAG_KERNEL_CONFIG, kc);
        }
        if let Some(ref nc) = self.network_config {
            write_section(AGI_TAG_NETWORK_CONFIG, nc);
        }
        if let Some(ref cc) = self.coherence_config {
            write_section(AGI_TAG_COHERENCE_CONFIG, cc);
        }
        if let Some(ref pi) = self.project_instructions {
            write_section(AGI_TAG_PROJECT_INSTRUCTIONS, pi);
        }
        if let Some(ref ds) = self.dependency_snapshot {
            write_section(AGI_TAG_DEPENDENCY_SNAPSHOT, ds);
        }
        if let Some(ref ac) = self.authority_config {
            write_section(AGI_TAG_AUTHORITY_CONFIG, ac);
        }
        if let Some(ref dp) = self.domain_profile {
            write_section(AGI_TAG_DOMAIN_PROFILE, dp);
        }

        payload
    }

    /// Build the manifest: header + TLV sections.
    pub fn build(self) -> Result<(Vec<u8>, AgiContainerHeader), ContainerError> {
        let sections = self.build_sections();

        let model_id_hash = match &self.model_id {
            Some(mid) => seed_crypto::seed_content_hash(mid),
            None => [0u8; 8],
        };

        let flags = self.segments.to_flags() | self.extra_flags;

        let created_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        let header = AgiContainerHeader {
            magic: AGI_MAGIC,
            version: 1,
            flags,
            container_id: self.container_id,
            build_id: self.build_id,
            created_ns,
            model_id_hash,
            policy_hash: self.policy_hash,
        };

        let mut payload = Vec::with_capacity(AGI_HEADER_SIZE + sections.len());
        payload.extend_from_slice(&header.to_bytes());
        payload.extend_from_slice(&sections);

        // Mark manifest as present for validation.
        let mut segs = self.segments.clone();
        segs.manifest_present = true;

        Ok((payload, header))
    }

    /// Build and sign with HMAC-SHA256.
    pub fn build_and_sign(
        mut self,
        key: &[u8],
    ) -> Result<(Vec<u8>, AgiContainerHeader), ContainerError> {
        self.segments.crypto_present = true;
        let (unsigned, mut header) = self.build()?;
        header.flags |= AGI_SIGNED;

        let sig = seed_crypto::sign_seed(key, &unsigned);
        let mut signed = unsigned;
        // Re-write the header with SIGNED flag.
        let header_bytes = header.to_bytes();
        signed[..AGI_HEADER_SIZE].copy_from_slice(&header_bytes);
        signed.extend_from_slice(&sig);

        Ok((signed, header))
    }
}

/// Parsed AGI container manifest with zero-copy section references.
#[derive(Debug)]
pub struct ParsedAgiManifest<'a> {
    /// Parsed header.
    pub header: AgiContainerHeader,
    /// Model identifier string.
    pub model_id: Option<&'a [u8]>,
    /// Policy bytes.
    pub policy: Option<&'a [u8]>,
    /// Orchestrator config.
    pub orchestrator_config: Option<&'a [u8]>,
    /// Tool registry.
    pub tool_registry: Option<&'a [u8]>,
    /// Agent prompts.
    pub agent_prompts: Option<&'a [u8]>,
    /// Eval tasks.
    pub eval_tasks: Option<&'a [u8]>,
    /// Eval graders.
    pub eval_graders: Option<&'a [u8]>,
    /// Skill library.
    pub skill_library: Option<&'a [u8]>,
    /// Replay script.
    pub replay_script: Option<&'a [u8]>,
    /// Kernel config.
    pub kernel_config: Option<&'a [u8]>,
    /// Network config.
    pub network_config: Option<&'a [u8]>,
    /// Coherence gate config.
    pub coherence_config: Option<&'a [u8]>,
    /// Project instructions.
    pub project_instructions: Option<&'a [u8]>,
    /// Dependency snapshot.
    pub dependency_snapshot: Option<&'a [u8]>,
    /// Authority configuration.
    pub authority_config: Option<&'a [u8]>,
    /// Domain profile.
    pub domain_profile: Option<&'a [u8]>,
}

impl<'a> ParsedAgiManifest<'a> {
    /// Parse a manifest from bytes.
    pub fn parse(data: &'a [u8]) -> Result<Self, ContainerError> {
        let header = AgiContainerHeader::from_bytes(data)
            .map_err(|_| ContainerError::InvalidConfig("invalid header"))?;

        let mut result = Self {
            header,
            model_id: None,
            policy: None,
            orchestrator_config: None,
            tool_registry: None,
            agent_prompts: None,
            eval_tasks: None,
            eval_graders: None,
            skill_library: None,
            replay_script: None,
            kernel_config: None,
            network_config: None,
            coherence_config: None,
            project_instructions: None,
            dependency_snapshot: None,
            authority_config: None,
            domain_profile: None,
        };

        // Parse TLV sections after header.
        let mut pos = AGI_HEADER_SIZE;
        while pos + 6 <= data.len() {
            let tag = u16::from_le_bytes([data[pos], data[pos + 1]]);
            let length =
                u32::from_le_bytes([data[pos + 2], data[pos + 3], data[pos + 4], data[pos + 5]])
                    as usize;
            pos += 6;

            if pos + length > data.len() {
                break;
            }

            let value = &data[pos..pos + length];
            match tag {
                AGI_TAG_MODEL_ID => result.model_id = Some(value),
                AGI_TAG_POLICY => result.policy = Some(value),
                AGI_TAG_ORCHESTRATOR => result.orchestrator_config = Some(value),
                AGI_TAG_TOOL_REGISTRY => result.tool_registry = Some(value),
                AGI_TAG_AGENT_PROMPTS => result.agent_prompts = Some(value),
                AGI_TAG_EVAL_TASKS => result.eval_tasks = Some(value),
                AGI_TAG_EVAL_GRADERS => result.eval_graders = Some(value),
                AGI_TAG_SKILL_LIBRARY => result.skill_library = Some(value),
                AGI_TAG_REPLAY_SCRIPT => result.replay_script = Some(value),
                AGI_TAG_KERNEL_CONFIG => result.kernel_config = Some(value),
                AGI_TAG_NETWORK_CONFIG => result.network_config = Some(value),
                AGI_TAG_COHERENCE_CONFIG => result.coherence_config = Some(value),
                AGI_TAG_PROJECT_INSTRUCTIONS => result.project_instructions = Some(value),
                AGI_TAG_DEPENDENCY_SNAPSHOT => result.dependency_snapshot = Some(value),
                AGI_TAG_AUTHORITY_CONFIG => result.authority_config = Some(value),
                AGI_TAG_DOMAIN_PROFILE => result.domain_profile = Some(value),
                _ => {} // forward-compat: ignore unknown tags
            }

            pos += length;
        }

        Ok(result)
    }

    /// Get the model ID as a UTF-8 string.
    pub fn model_id_str(&self) -> Option<&str> {
        self.model_id.and_then(|b| core::str::from_utf8(b).ok())
    }

    /// Check if the manifest has all sections needed for autonomous operation.
    pub fn is_autonomous_capable(&self) -> bool {
        self.orchestrator_config.is_some()
            && self.eval_tasks.is_some()
            && self.eval_graders.is_some()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const ORCHESTRATOR_CONFIG: &[u8] = br#"{
        "claude_code": {
            "model": "claude-opus-4-6",
            "max_turns": 100,
            "permission_mode": "bypassPermissions"
        },
        "claude_flow": {
            "topology": "hierarchical",
            "max_agents": 15,
            "strategy": "specialized",
            "memory": "hybrid"
        }
    }"#;

    const TOOL_REGISTRY: &[u8] = br#"[
        {"name": "ruvector_query", "type": "vector_search"},
        {"name": "ruvector_cypher", "type": "graph_query"},
        {"name": "ruvector_commit_delta", "type": "write"},
        {"name": "rvf_snapshot", "type": "snapshot"},
        {"name": "rvf_witness_export", "type": "export"},
        {"name": "eval_run", "type": "evaluation"}
    ]"#;

    const COHERENCE_CONFIG: &[u8] = br#"{
        "min_cut_threshold": 0.7,
        "contradiction_pressure_max": 0.3,
        "quarantine_ttl_hours": 24,
        "skill_promotion_k": 5,
        "rollback_on_violation": true
    }"#;

    #[test]
    fn build_full_container() {
        let segs = ContainerSegments {
            kernel_present: true,
            kernel_size: 5_000_000,
            wasm_count: 2,
            wasm_total_size: 60_000,
            vec_segment_count: 4,
            index_segment_count: 2,
            witness_count: 100,
            crypto_present: false,
            manifest_present: false,
            orchestrator_present: true,
            world_model_present: true,
            domain_expansion_present: false,
            total_size: 0,
        };

        let builder = AgiContainerBuilder::new([0x01; 16], [0x02; 16])
            .with_model_id("claude-opus-4-6")
            .with_policy(b"autonomous", [0xAA; 8])
            .with_orchestrator(ORCHESTRATOR_CONFIG)
            .with_tool_registry(TOOL_REGISTRY)
            .with_agent_prompts(b"You are a coder agent...")
            .with_eval_tasks(b"[{\"id\":1,\"spec\":\"fix bug\"}]")
            .with_eval_graders(b"[{\"type\":\"test_pass\"}]")
            .with_skill_library(b"[]")
            .with_replay_script(b"#!/bin/sh\nrvf replay $1")
            .with_kernel_config(b"console=ttyS0 root=/dev/vda")
            .with_network_config(b"{\"port\":8080}")
            .with_coherence_config(COHERENCE_CONFIG)
            .with_project_instructions(b"# CLAUDE.md\nFollow DDD...")
            .with_dependency_snapshot(b"sha256:abc123")
            .offline_capable()
            .with_segments(segs);

        let (payload, header) = builder.build().unwrap();

        assert!(header.is_valid_magic());
        assert!(header.has_kernel());
        assert!(header.has_orchestrator());
        assert!(header.is_replay_capable());
        assert!(header.is_offline_capable());

        // Parse it back.
        let parsed = ParsedAgiManifest::parse(&payload).unwrap();
        assert_eq!(parsed.model_id_str(), Some("claude-opus-4-6"));
        assert_eq!(parsed.orchestrator_config.unwrap(), ORCHESTRATOR_CONFIG);
        assert_eq!(parsed.tool_registry.unwrap(), TOOL_REGISTRY);
        assert_eq!(parsed.coherence_config.unwrap(), COHERENCE_CONFIG);
        assert!(parsed.project_instructions.is_some());
        assert!(parsed.is_autonomous_capable());
    }

    #[test]
    fn signed_container_round_trip() {
        let key = b"container-signing-key-for-tests!";
        let builder = AgiContainerBuilder::new([0x10; 16], [0x20; 16])
            .with_model_id("claude-opus-4-6")
            .with_orchestrator(ORCHESTRATOR_CONFIG)
            .with_eval_tasks(b"[]")
            .with_eval_graders(b"[]")
            .with_segments(ContainerSegments {
                kernel_present: true,
                manifest_present: false,
                ..Default::default()
            });

        let (payload, header) = builder.build_and_sign(key).unwrap();
        assert!(header.is_signed());

        // Verify signature.
        let unsigned_len = payload.len() - 32;
        let sig = &payload[unsigned_len..];
        assert!(seed_crypto::verify_seed(key, &payload[..unsigned_len], sig));
    }

    #[test]
    fn minimal_container() {
        let builder =
            AgiContainerBuilder::new([0x30; 16], [0x40; 16]).with_segments(ContainerSegments {
                kernel_present: true,
                manifest_present: true,
                ..Default::default()
            });

        let (payload, header) = builder.build().unwrap();
        assert!(header.has_kernel());

        let parsed = ParsedAgiManifest::parse(&payload).unwrap();
        assert!(parsed.model_id.is_none());
        assert!(!parsed.is_autonomous_capable());
    }

    #[test]
    fn authority_and_domain_round_trip() {
        let authority = br#"{"max_authority":"ExecuteTools","budget":{"max_time_secs":600,"max_tokens":400000}}"#;
        let domain = b"repo-automation-v1";

        let builder = AgiContainerBuilder::new([0x50; 16], [0x60; 16])
            .with_authority_config(authority)
            .with_domain_profile(domain)
            .with_segments(ContainerSegments {
                kernel_present: true,
                manifest_present: true,
                ..Default::default()
            });

        let (payload, _header) = builder.build().unwrap();
        let parsed = ParsedAgiManifest::parse(&payload).unwrap();
        assert_eq!(parsed.authority_config.unwrap(), authority.as_slice());
        assert_eq!(parsed.domain_profile.unwrap(), domain.as_slice());
    }

    #[test]
    fn segment_validation() {
        // Live mode needs kernel or WASM, plus world model.
        let segs = ContainerSegments {
            manifest_present: true,
            kernel_present: true,
            world_model_present: true,
            ..Default::default()
        };
        assert!(segs.validate(ExecutionMode::Live).is_ok());

        // Replay mode needs witness.
        let segs2 = ContainerSegments {
            manifest_present: true,
            witness_count: 50,
            ..Default::default()
        };
        assert!(segs2.validate(ExecutionMode::Replay).is_ok());
    }
}
