# QuDAG Integration Specification

## Overview

This document specifies the optional integration between RuVector-Postgres Neural DAG system and QuDAG (Quantum-resistant Distributed DAG) for federated learning and distributed consensus on learned patterns.

## Integration Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    QUDAG INTEGRATION LAYER                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │                    FEDERATED LEARNING                                 │  │
│  │                                                                       │  │
│  │   Node A (US)           Node B (EU)          Node C (Asia)           │  │
│  │  ┌──────────────┐     ┌──────────────┐     ┌──────────────┐         │  │
│  │  │ RuVector-PG  │     │ RuVector-PG  │     │ RuVector-PG  │         │  │
│  │  │ ┌──────────┐ │     │ ┌──────────┐ │     │ ┌──────────┐ │         │  │
│  │  │ │ Patterns │ │     │ │ Patterns │ │     │ │ Patterns │ │         │  │
│  │  │ └────┬─────┘ │     │ └────┬─────┘ │     │ └────┬─────┘ │         │  │
│  │  └──────┼───────┘     └──────┼───────┘     └──────┼───────┘         │  │
│  │         │                    │                    │                  │  │
│  │         └────────────────────┼────────────────────┘                  │  │
│  │                              ▼                                       │  │
│  │                    ┌─────────────────┐                               │  │
│  │                    │  QuDAG Network  │                               │  │
│  │                    │ (QR-Avalanche)  │                               │  │
│  │                    └────────┬────────┘                               │  │
│  │                              │                                       │  │
│  │         ┌────────────────────┼────────────────────┐                  │  │
│  │         ▼                    ▼                    ▼                  │  │
│  │  ┌────────────┐       ┌────────────┐       ┌────────────┐           │  │
│  │  │  Consensus │       │  Consensus │       │  Consensus │           │  │
│  │  │  Patterns  │       │  Patterns  │       │  Patterns  │           │  │
│  │  └────────────┘       └────────────┘       └────────────┘           │  │
│  │                                                                       │  │
│  └──────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │                    SECURITY LAYER                                     │  │
│  │  ┌────────────┐  ┌────────────┐  ┌────────────┐  ┌────────────┐     │  │
│  │  │  ML-KEM    │  │  ML-DSA    │  │ Differential│  │    rUv     │     │  │
│  │  │ Encryption │  │ Signatures │  │   Privacy   │  │  Tokens    │     │  │
│  │  └────────────┘  └────────────┘  └────────────┘  └────────────┘     │  │
│  └──────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

## QuDAG Client

### Core Structure

```rust
pub struct QuDagClient {
    /// QuDAG node connection
    node_url: String,

    /// Node identity (ML-DSA keypair)
    identity: QuDagIdentity,

    /// Local pattern cache
    pattern_cache: DashMap<PatternId, ConsensusPattern>,

    /// Pending proposals
    pending_proposals: DashMap<ProposalId, PatternProposal>,

    /// Configuration
    config: QuDagConfig,

    /// Metrics
    metrics: QuDagMetrics,
}

#[derive(Clone)]
pub struct QuDagIdentity {
    /// ML-DSA-65 public key
    pub public_key: MlDsaPublicKey,

    /// ML-DSA-65 private key (encrypted at rest)
    private_key: MlDsaPrivateKey,

    /// Node identifier
    pub node_id: NodeId,

    /// Dark address (for anonymous communication)
    pub dark_address: Option<DarkAddress>,
}

#[derive(Clone, Debug)]
pub struct QuDagConfig {
    /// Enable QuDAG integration
    pub enabled: bool,

    /// QuDAG node URL
    pub node_url: String,

    /// Differential privacy epsilon
    pub dp_epsilon: f64,

    /// Minimum validators for consensus
    pub min_validators: usize,

    /// Consensus timeout (seconds)
    pub consensus_timeout_secs: u64,

    /// Sync interval (seconds)
    pub sync_interval_secs: u64,

    /// Maximum patterns per proposal
    pub max_patterns_per_proposal: usize,

    /// rUv staking requirement
    pub min_stake_ruv: u64,
}

impl Default for QuDagConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            node_url: "https://yyz.qudag.darknet/mcp".to_string(),
            dp_epsilon: 1.0,
            min_validators: 5,
            consensus_timeout_secs: 30,
            sync_interval_secs: 3600,
            max_patterns_per_proposal: 100,
            min_stake_ruv: 10,
        }
    }
}
```

### Pattern Proposal

```rust
impl QuDagClient {
    /// Propose local patterns for consensus
    pub async fn propose_patterns(
        &self,
        patterns: &[LearnedDagPattern],
    ) -> Result<ProposalId, QuDagError> {
        // 1. Add differential privacy noise
        let noisy_patterns = self.add_dp_noise(patterns)?;

        // 2. Create proposal
        let proposal = PatternProposal {
            id: self.generate_proposal_id(),
            proposer: self.identity.node_id.clone(),
            patterns: noisy_patterns,
            stake: self.config.min_stake_ruv,
            timestamp: SystemTime::now(),
            signature: None,
        };

        // 3. Sign with ML-DSA
        let signed_proposal = self.sign_proposal(proposal)?;

        // 4. Submit to QuDAG network
        self.submit_proposal(&signed_proposal).await?;

        // 5. Track pending
        self.pending_proposals.insert(signed_proposal.id, signed_proposal.clone());

        Ok(signed_proposal.id)
    }

    /// Add differential privacy noise to patterns
    fn add_dp_noise(&self, patterns: &[LearnedDagPattern]) -> Result<Vec<NoisyPattern>, QuDagError> {
        let epsilon = self.config.dp_epsilon;

        patterns.iter()
            .map(|p| {
                // Add Laplace noise to centroid
                let noisy_centroid: Vec<f32> = p.centroid.iter()
                    .map(|&v| {
                        let noise = laplace_sample(0.0, 1.0 / epsilon);
                        v + noise as f32
                    })
                    .collect();

                // Quantize quality scores
                let quantized_quality = (p.avg_metrics.quality * 10.0).round() / 10.0;

                Ok(NoisyPattern {
                    centroid: noisy_centroid,
                    attention_type: p.optimal_attention.clone(),
                    quality: quantized_quality,
                    sample_count_bucket: bucket_sample_count(p.sample_count),
                })
            })
            .collect()
    }

    /// Sign proposal with ML-DSA-65
    fn sign_proposal(&self, mut proposal: PatternProposal) -> Result<PatternProposal, QuDagError> {
        let message = proposal.to_signing_bytes();
        let signature = self.identity.private_key.sign(&message)?;
        proposal.signature = Some(signature);
        Ok(proposal)
    }

    /// Submit proposal to QuDAG network
    async fn submit_proposal(&self, proposal: &PatternProposal) -> Result<(), QuDagError> {
        // Connect to QuDAG MCP server
        let client = McpClient::connect(&self.config.node_url).await?;

        // Call dag_submit tool
        let response = client.call_tool("dag_submit", json!({
            "vertex_type": "pattern_proposal",
            "payload": proposal.to_encrypted_bytes(&self.get_network_key())?,
            "parents": self.get_recent_vertices().await?,
        })).await?;

        if response["success"].as_bool().unwrap_or(false) {
            Ok(())
        } else {
            Err(QuDagError::SubmissionFailed(
                response["error"].as_str().unwrap_or("Unknown error").to_string()
            ))
        }
    }
}

#[derive(Clone, Debug)]
pub struct PatternProposal {
    pub id: ProposalId,
    pub proposer: NodeId,
    pub patterns: Vec<NoisyPattern>,
    pub stake: u64,
    pub timestamp: SystemTime,
    pub signature: Option<MlDsaSignature>,
}

#[derive(Clone, Debug)]
pub struct NoisyPattern {
    /// Centroid with DP noise
    pub centroid: Vec<f32>,

    /// Attention type (no noise needed)
    pub attention_type: DagAttentionType,

    /// Quantized quality
    pub quality: f64,

    /// Bucketed sample count (privacy)
    pub sample_count_bucket: SampleCountBucket,
}

#[derive(Clone, Debug)]
pub enum SampleCountBucket {
    Few,       // < 10
    Some,      // 10-50
    Many,      // 50-200
    Lots,      // > 200
}
```

### Consensus Validation

```rust
impl QuDagClient {
    /// Validate incoming pattern proposals
    pub async fn validate_proposal(
        &self,
        proposal: &PatternProposal,
    ) -> Result<ValidationResult, QuDagError> {
        // 1. Verify signature
        if !self.verify_signature(proposal)? {
            return Ok(ValidationResult::Rejected {
                reason: "Invalid signature".to_string(),
            });
        }

        // 2. Check stake
        let balance = self.get_ruv_balance(&proposal.proposer).await?;
        if balance < proposal.stake {
            return Ok(ValidationResult::Rejected {
                reason: "Insufficient stake".to_string(),
            });
        }

        // 3. Validate pattern quality
        let quality_scores: Vec<f64> = proposal.patterns.iter()
            .map(|p| p.quality)
            .collect();

        let avg_quality = quality_scores.iter().sum::<f64>() / quality_scores.len() as f64;
        if avg_quality < 0.3 {
            return Ok(ValidationResult::Rejected {
                reason: "Low quality patterns".to_string(),
            });
        }

        // 4. Check for duplicate patterns
        let duplicates = self.check_duplicates(&proposal.patterns).await?;
        if duplicates > proposal.patterns.len() / 2 {
            return Ok(ValidationResult::Rejected {
                reason: "Too many duplicate patterns".to_string(),
            });
        }

        // 5. Compute accuracy improvement (sample-based)
        let improvement = self.estimate_improvement(&proposal.patterns).await?;

        Ok(ValidationResult::Accepted {
            quality_score: avg_quality,
            improvement_estimate: improvement,
            validator: self.identity.node_id.clone(),
        })
    }

    /// Submit validation to QuDAG
    pub async fn submit_validation(
        &self,
        proposal_id: ProposalId,
        result: &ValidationResult,
    ) -> Result<(), QuDagError> {
        let validation = Validation {
            proposal_id,
            result: result.clone(),
            validator: self.identity.node_id.clone(),
            timestamp: SystemTime::now(),
            signature: None,
        };

        let signed = self.sign_validation(validation)?;

        let client = McpClient::connect(&self.config.node_url).await?;
        client.call_tool("dag_submit", json!({
            "vertex_type": "pattern_validation",
            "payload": signed.to_encrypted_bytes(&self.get_network_key())?,
            "parents": [proposal_id.to_string()],
        })).await?;

        Ok(())
    }
}

#[derive(Clone, Debug)]
pub enum ValidationResult {
    Accepted {
        quality_score: f64,
        improvement_estimate: f32,
        validator: NodeId,
    },
    Rejected {
        reason: String,
    },
}
```

### Pattern Synchronization

```rust
impl QuDagClient {
    /// Sync consensus patterns from QuDAG
    pub async fn sync_patterns(&self) -> Result<SyncResult, QuDagError> {
        let start = Instant::now();

        // 1. Get latest consensus patterns
        let client = McpClient::connect(&self.config.node_url).await?;

        let response = client.call_tool("dag_query", json!({
            "query_type": "consensus_patterns",
            "since": self.last_sync_timestamp(),
            "limit": 1000,
        })).await?;

        let consensus_patterns: Vec<ConsensusPattern> = serde_json::from_value(
            response["patterns"].clone()
        )?;

        // 2. Verify signatures
        let verified: Vec<_> = consensus_patterns.into_iter()
            .filter(|p| self.verify_consensus_signature(p).unwrap_or(false))
            .collect();

        // 3. Update local cache
        let mut new_count = 0;
        for pattern in &verified {
            if !self.pattern_cache.contains_key(&pattern.id) {
                self.pattern_cache.insert(pattern.id, pattern.clone());
                new_count += 1;
            }
        }

        // 4. Update local ReasoningBank
        let imported = self.import_to_reasoning_bank(&verified)?;

        Ok(SyncResult {
            patterns_received: verified.len(),
            new_patterns: new_count,
            patterns_imported: imported,
            duration: start.elapsed(),
        })
    }

    /// Import consensus patterns to local ReasoningBank
    fn import_to_reasoning_bank(&self, patterns: &[ConsensusPattern]) -> Result<usize, QuDagError> {
        let engines = get_all_dag_engines();
        let mut imported = 0;

        for pattern in patterns {
            // Find matching local engine by pattern type
            for engine in &engines {
                let local_pattern = LearnedDagPattern {
                    id: self.generate_local_pattern_id(),
                    centroid: pattern.centroid.clone(),
                    optimal_params: ExecutionParams::default(),
                    optimal_attention: pattern.attention_type.clone(),
                    confidence: pattern.consensus_confidence,
                    sample_count: pattern.total_samples,
                    avg_metrics: AverageMetrics {
                        latency_us: 0.0,  // Unknown from consensus
                        memory_bytes: 0.0,
                        quality: pattern.avg_quality,
                    },
                    updated_at: SystemTime::now(),
                };

                let mut bank = engine.dag_reasoning_bank.write();
                bank.store(local_pattern);
                imported += 1;
            }
        }

        Ok(imported)
    }
}

#[derive(Clone, Debug)]
pub struct ConsensusPattern {
    pub id: PatternId,
    pub centroid: Vec<f32>,
    pub attention_type: DagAttentionType,
    pub avg_quality: f64,
    pub total_samples: usize,
    pub consensus_confidence: f32,
    pub validators: Vec<NodeId>,
    pub signatures: Vec<MlDsaSignature>,
    pub finalized_at: SystemTime,
}

#[derive(Clone, Debug)]
pub struct SyncResult {
    pub patterns_received: usize,
    pub new_patterns: usize,
    pub patterns_imported: usize,
    pub duration: Duration,
}
```

## rUv Token Integration

### Token Economy

```rust
pub struct RuvTokenClient {
    /// QuDAG client reference
    qudag: Arc<QuDagClient>,

    /// Local balance cache
    balance_cache: AtomicU64,

    /// Pending rewards
    pending_rewards: DashMap<TransactionId, PendingReward>,
}

impl RuvTokenClient {
    /// Check rUv balance
    pub async fn get_balance(&self) -> Result<u64, QuDagError> {
        let client = McpClient::connect(&self.qudag.config.node_url).await?;

        let response = client.call_tool("ruv_balance", json!({
            "address": self.qudag.identity.node_id.to_string(),
        })).await?;

        let balance = response["balance"].as_u64().unwrap_or(0);
        self.balance_cache.store(balance, Ordering::Relaxed);

        Ok(balance)
    }

    /// Stake rUv for pattern proposal
    pub async fn stake(&self, amount: u64) -> Result<TransactionId, QuDagError> {
        let client = McpClient::connect(&self.qudag.config.node_url).await?;

        let response = client.call_tool("ruv_stake", json!({
            "amount": amount,
            "purpose": "pattern_proposal",
            "signature": self.sign_stake_request(amount)?,
        })).await?;

        Ok(TransactionId::from_str(response["tx_id"].as_str().unwrap())?)
    }

    /// Claim rewards for accepted patterns
    pub async fn claim_rewards(&self) -> Result<ClaimResult, QuDagError> {
        let client = McpClient::connect(&self.qudag.config.node_url).await?;

        let response = client.call_tool("ruv_claim_rewards", json!({
            "address": self.qudag.identity.node_id.to_string(),
            "signature": self.sign_claim_request()?,
        })).await?;

        let claimed = response["claimed"].as_u64().unwrap_or(0);
        let new_balance = response["new_balance"].as_u64().unwrap_or(0);

        self.balance_cache.store(new_balance, Ordering::Relaxed);

        Ok(ClaimResult {
            amount_claimed: claimed,
            new_balance,
        })
    }
}

/// Reward structure
#[derive(Clone, Debug)]
pub struct RewardStructure {
    /// Base reward for accepted pattern
    pub pattern_accepted: u64,  // 10 rUv

    /// Bonus for accuracy improvement
    pub accuracy_bonus_per_percent: u64,  // 10 rUv per 1%

    /// Validation reward
    pub validation_reward: u64,  // 2 rUv

    /// Penalty for rejected pattern
    pub rejection_penalty: u64,  // 5 rUv

    /// Byzantine behavior penalty
    pub byzantine_penalty: u64,  // 1000 rUv
}

impl Default for RewardStructure {
    fn default() -> Self {
        Self {
            pattern_accepted: 10,
            accuracy_bonus_per_percent: 10,
            validation_reward: 2,
            rejection_penalty: 5,
            byzantine_penalty: 1000,
        }
    }
}
```

## Security Layer

### ML-KEM Encryption

```rust
pub struct PatternEncryption {
    /// Network public key (for encryption)
    network_key: MlKemPublicKey,

    /// Local private key (for decryption)
    local_key: MlKemPrivateKey,
}

impl PatternEncryption {
    /// Encrypt pattern for network transmission
    pub fn encrypt(&self, pattern: &NoisyPattern) -> Result<EncryptedPattern, CryptoError> {
        let plaintext = pattern.to_bytes();

        // Encapsulate shared secret
        let (ciphertext, shared_secret) = self.network_key.encapsulate()?;

        // Derive key from shared secret
        let key = blake3::derive_key("QuDAG Pattern Encryption", &shared_secret);

        // Encrypt with ChaCha20-Poly1305
        let nonce = generate_nonce();
        let encrypted = chacha20_poly1305_encrypt(&key, &nonce, &plaintext)?;

        Ok(EncryptedPattern {
            ciphertext,
            encrypted_data: encrypted,
            nonce,
        })
    }

    /// Decrypt pattern from network
    pub fn decrypt(&self, encrypted: &EncryptedPattern) -> Result<NoisyPattern, CryptoError> {
        // Decapsulate shared secret
        let shared_secret = self.local_key.decapsulate(&encrypted.ciphertext)?;

        // Derive key
        let key = blake3::derive_key("QuDAG Pattern Encryption", &shared_secret);

        // Decrypt
        let plaintext = chacha20_poly1305_decrypt(
            &key,
            &encrypted.nonce,
            &encrypted.encrypted_data,
        )?;

        NoisyPattern::from_bytes(&plaintext)
    }
}
```

### ML-DSA Signatures

```rust
pub struct PatternSigning {
    /// Signing key
    private_key: MlDsaPrivateKey,

    /// Verification key
    public_key: MlDsaPublicKey,
}

impl PatternSigning {
    /// Sign pattern proposal
    pub fn sign_proposal(&self, proposal: &PatternProposal) -> Result<MlDsaSignature, CryptoError> {
        let message = proposal.to_signing_bytes();
        self.private_key.sign(&message)
    }

    /// Verify proposal signature
    pub fn verify_proposal(
        &self,
        proposal: &PatternProposal,
        public_key: &MlDsaPublicKey,
    ) -> Result<bool, CryptoError> {
        let message = proposal.to_signing_bytes();
        let signature = proposal.signature.as_ref()
            .ok_or(CryptoError::MissingSignature)?;

        public_key.verify(&message, signature)
    }

    /// Sign validation
    pub fn sign_validation(&self, validation: &Validation) -> Result<MlDsaSignature, CryptoError> {
        let message = validation.to_signing_bytes();
        self.private_key.sign(&message)
    }
}
```

## SQL Interface

```sql
-- Enable QuDAG integration
SELECT ruvector_dag_qudag_enable('{
    "node_url": "https://yyz.qudag.darknet/mcp",
    "dp_epsilon": 1.0,
    "min_stake_ruv": 10
}'::jsonb);

-- Register identity
SELECT ruvector_dag_qudag_register();

-- Propose patterns for consensus
SELECT ruvector_dag_qudag_propose('documents');

-- Sync consensus patterns
SELECT ruvector_dag_qudag_sync();

-- Get rUv balance
SELECT ruvector_dag_ruv_balance();

-- Claim rewards
SELECT ruvector_dag_ruv_claim();

-- Get QuDAG status
SELECT ruvector_dag_qudag_status();
```

## Configuration

### PostgreSQL GUC Variables

```sql
-- Enable/disable QuDAG
SET ruvector.dag_qudag_enabled = true;

-- QuDAG node URL
SET ruvector.dag_qudag_node_url = 'https://yyz.qudag.darknet/mcp';

-- Differential privacy epsilon
SET ruvector.dag_qudag_dp_epsilon = 1.0;

-- Sync interval (seconds)
SET ruvector.dag_qudag_sync_interval = 3600;

-- Minimum stake for proposals
SET ruvector.dag_qudag_min_stake = 10;
```

## Metrics

```rust
#[derive(Clone, Debug, Default)]
pub struct QuDagMetrics {
    pub proposals_submitted: AtomicU64,
    pub proposals_accepted: AtomicU64,
    pub proposals_rejected: AtomicU64,
    pub validations_performed: AtomicU64,
    pub patterns_synced: AtomicU64,
    pub ruv_earned: AtomicU64,
    pub ruv_spent: AtomicU64,
    pub last_sync_time: AtomicU64,
}

impl QuDagMetrics {
    pub fn to_json(&self) -> serde_json::Value {
        json!({
            "proposals_submitted": self.proposals_submitted.load(Ordering::Relaxed),
            "proposals_accepted": self.proposals_accepted.load(Ordering::Relaxed),
            "proposals_rejected": self.proposals_rejected.load(Ordering::Relaxed),
            "acceptance_rate": self.acceptance_rate(),
            "validations_performed": self.validations_performed.load(Ordering::Relaxed),
            "patterns_synced": self.patterns_synced.load(Ordering::Relaxed),
            "ruv_net": self.ruv_net(),
        })
    }

    fn acceptance_rate(&self) -> f64 {
        let submitted = self.proposals_submitted.load(Ordering::Relaxed);
        let accepted = self.proposals_accepted.load(Ordering::Relaxed);
        if submitted > 0 {
            accepted as f64 / submitted as f64
        } else {
            0.0
        }
    }

    fn ruv_net(&self) -> i64 {
        self.ruv_earned.load(Ordering::Relaxed) as i64
            - self.ruv_spent.load(Ordering::Relaxed) as i64
    }
}
```
