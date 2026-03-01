# ADR-DB-010: Delta Security Model

**Status**: Proposed
**Date**: 2026-01-28
**Authors**: RuVector Architecture Team
**Deciders**: Architecture Review Board, Security Team
**Parent**: ADR-DB-001 Delta Behavior Core Architecture

## Version History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 0.1 | 2026-01-28 | Architecture Team | Initial proposal |

---

## Context and Problem Statement

### The Security Challenge

Delta-first architecture introduces new attack surfaces:

1. **Delta Integrity**: Deltas could be tampered with in transit or storage
2. **Authorization**: Who can create, modify, or read deltas?
3. **Replay Attacks**: Resubmission of old deltas
4. **Information Leakage**: Delta patterns reveal update frequency
5. **Denial of Service**: Flood of malicious deltas

### Threat Model

| Threat Actor | Capability | Goal |
|--------------|------------|------|
| External Attacker | Network access | Data exfiltration, corruption |
| Malicious Insider | API access | Unauthorized modifications |
| Compromised Replica | Full replica access | State corruption |
| Network Adversary | Traffic interception | Delta manipulation |

### Security Requirements

| Requirement | Priority | Description |
|-------------|----------|-------------|
| Integrity | Critical | Detect tampered deltas |
| Authentication | Critical | Verify delta origin |
| Authorization | High | Enforce access control |
| Confidentiality | Medium | Protect delta contents |
| Non-repudiation | Medium | Prove delta authorship |
| Availability | High | Resist DoS attacks |

---

## Decision

### Adopt Signed Deltas with Capability Tokens

We implement a defense-in-depth security model with cryptographically signed deltas and fine-grained capability-based authorization.

### Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          SECURITY PERIMETER                                  │
│                                                                             │
│  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐  ┌──────────────┐ │
│  │  TLS 1.3      │  │  mTLS         │  │  Rate Limit   │  │  WAF         │ │
│  │  Transport    │  │  Auth         │  │  (per-client) │  │  (optional)  │ │
│  └───────────────┘  └───────────────┘  └───────────────┘  └──────────────┘ │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    v
┌─────────────────────────────────────────────────────────────────────────────┐
│                          AUTHENTICATION LAYER                                │
│                                                                             │
│  ┌───────────────────────────────────────────────────────────────────────┐ │
│  │               Identity Verification                                     │ │
│  │   API Key │ JWT │ Client Certificate │ Capability Token                │ │
│  └───────────────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    v
┌─────────────────────────────────────────────────────────────────────────────┐
│                          AUTHORIZATION LAYER                                 │
│                                                                             │
│  ┌────────────────┐  ┌────────────────┐  ┌────────────────────────────────┐│
│  │  Capability    │  │  RBAC          │  │  Namespace Isolation           ││
│  │  Tokens        │  │  Policies      │  │                                ││
│  └────────────────┘  └────────────────┘  └────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    v
┌─────────────────────────────────────────────────────────────────────────────┐
│                          DELTA SECURITY                                      │
│                                                                             │
│  ┌────────────────┐  ┌────────────────┐  ┌────────────────────────────────┐│
│  │  Signature     │  │  Replay        │  │  Integrity                     ││
│  │  Verification  │  │  Protection    │  │  Validation                    ││
│  └────────────────┘  └────────────────┘  └────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────────────────┘
```

### Core Components

#### 1. Signed Deltas

```rust
use ed25519_dalek::{Signature, SigningKey, VerifyingKey};
use sha2::{Sha256, Digest};

/// A cryptographically signed delta
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedDelta {
    /// The delta content
    pub delta: VectorDelta,
    /// Ed25519 signature over delta hash
    pub signature: Signature,
    /// Signing key identifier
    pub key_id: KeyId,
    /// Timestamp of signing
    pub signed_at: DateTime<Utc>,
    /// Nonce for replay protection
    pub nonce: [u8; 16],
}

/// Delta signer for creating signed deltas
pub struct DeltaSigner {
    /// Signing key
    signing_key: SigningKey,
    /// Key identifier
    key_id: KeyId,
    /// Nonce tracker
    nonce_tracker: NonceTracker,
}

impl DeltaSigner {
    /// Sign a delta
    pub fn sign(&self, delta: VectorDelta) -> Result<SignedDelta, SigningError> {
        // Generate nonce
        let nonce = self.nonce_tracker.generate();

        // Create signing payload
        let payload = SigningPayload {
            delta: &delta,
            nonce: &nonce,
            timestamp: Utc::now(),
        };

        // Compute hash
        let hash = self.compute_payload_hash(&payload);

        // Sign hash
        let signature = self.signing_key.sign(&hash);

        Ok(SignedDelta {
            delta,
            signature,
            key_id: self.key_id.clone(),
            signed_at: payload.timestamp,
            nonce,
        })
    }

    fn compute_payload_hash(&self, payload: &SigningPayload) -> [u8; 32] {
        let mut hasher = Sha256::new();

        // Hash delta content
        hasher.update(&bincode::serialize(&payload.delta).unwrap());

        // Hash nonce
        hasher.update(payload.nonce);

        // Hash timestamp
        hasher.update(&payload.timestamp.timestamp().to_le_bytes());

        hasher.finalize().into()
    }
}

/// Delta verifier for validating signed deltas
pub struct DeltaVerifier {
    /// Known public keys
    public_keys: DashMap<KeyId, VerifyingKey>,
    /// Nonce store for replay protection
    nonce_store: NonceStore,
    /// Clock skew tolerance
    clock_tolerance: Duration,
}

impl DeltaVerifier {
    /// Verify a signed delta
    pub fn verify(&self, signed_delta: &SignedDelta) -> Result<(), VerificationError> {
        // Check key exists
        let public_key = self.public_keys
            .get(&signed_delta.key_id)
            .ok_or(VerificationError::UnknownKey)?;

        // Check timestamp is recent
        let age = Utc::now().signed_duration_since(signed_delta.signed_at);
        if age.abs() > self.clock_tolerance.as_secs() as i64 {
            return Err(VerificationError::ExpiredOrFuture);
        }

        // Check nonce hasn't been used
        if self.nonce_store.is_used(&signed_delta.nonce) {
            return Err(VerificationError::ReplayDetected);
        }

        // Verify signature
        let payload = SigningPayload {
            delta: &signed_delta.delta,
            nonce: &signed_delta.nonce,
            timestamp: signed_delta.signed_at,
        };
        let hash = self.compute_payload_hash(&payload);

        public_key.verify(&hash, &signed_delta.signature)
            .map_err(|_| VerificationError::InvalidSignature)?;

        // Mark nonce as used
        self.nonce_store.mark_used(signed_delta.nonce);

        Ok(())
    }
}
```

#### 2. Capability Tokens

```rust
/// Capability token for fine-grained authorization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityToken {
    /// Token identifier
    pub token_id: TokenId,
    /// Subject (who this token is for)
    pub subject: Subject,
    /// Granted capabilities
    pub capabilities: Vec<Capability>,
    /// Token issuer
    pub issuer: String,
    /// Issued at
    pub issued_at: DateTime<Utc>,
    /// Expires at
    pub expires_at: DateTime<Utc>,
    /// Restrictions
    pub restrictions: TokenRestrictions,
    /// Signature
    pub signature: Signature,
}

/// Individual capability grant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Capability {
    /// Create deltas for specific vectors
    CreateDelta {
        vector_patterns: Vec<VectorPattern>,
        operation_types: Vec<OperationType>,
    },
    /// Read vectors and their deltas
    ReadVector {
        vector_patterns: Vec<VectorPattern>,
    },
    /// Search capability
    Search {
        namespaces: Vec<String>,
        max_k: usize,
    },
    /// Compact delta chains
    Compact {
        vector_patterns: Vec<VectorPattern>,
    },
    /// Administrative capability
    Admin {
        scope: AdminScope,
    },
}

/// Pattern for matching vector IDs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VectorPattern {
    /// Exact match
    Exact(VectorId),
    /// Prefix match
    Prefix(String),
    /// Regex match
    Regex(String),
    /// All vectors in namespace
    Namespace(String),
    /// All vectors
    All,
}

/// Token restrictions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenRestrictions {
    /// Rate limit (requests per second)
    pub rate_limit: Option<f32>,
    /// IP address restrictions
    pub allowed_ips: Option<Vec<IpNetwork>>,
    /// Time of day restrictions
    pub time_windows: Option<Vec<TimeWindow>>,
    /// Maximum delta size
    pub max_delta_size: Option<usize>,
}

/// Capability verifier
pub struct CapabilityVerifier {
    /// Trusted issuers' public keys
    issuer_keys: DashMap<String, VerifyingKey>,
    /// Token revocation list
    revoked: HashSet<TokenId>,
}

impl CapabilityVerifier {
    /// Verify token and extract capabilities
    pub fn verify_token(&self, token: &CapabilityToken) -> Result<&[Capability], AuthError> {
        // Check not revoked
        if self.revoked.contains(&token.token_id) {
            return Err(AuthError::TokenRevoked);
        }

        // Check expiration
        if Utc::now() > token.expires_at {
            return Err(AuthError::TokenExpired);
        }

        // Check not before issued
        if Utc::now() < token.issued_at {
            return Err(AuthError::TokenNotYetValid);
        }

        // Verify signature
        let issuer_key = self.issuer_keys
            .get(&token.issuer)
            .ok_or(AuthError::UnknownIssuer)?;

        let payload = self.compute_token_hash(token);
        issuer_key.verify(&payload, &token.signature)
            .map_err(|_| AuthError::InvalidTokenSignature)?;

        Ok(&token.capabilities)
    }

    /// Check if token authorizes an operation
    pub fn authorize(
        &self,
        token: &CapabilityToken,
        operation: &DeltaOperation,
        vector_id: &VectorId,
    ) -> Result<(), AuthError> {
        let capabilities = self.verify_token(token)?;

        for cap in capabilities {
            if self.capability_allows(cap, operation, vector_id) {
                return Ok(());
            }
        }

        Err(AuthError::Unauthorized)
    }

    fn capability_allows(
        &self,
        cap: &Capability,
        operation: &DeltaOperation,
        vector_id: &VectorId,
    ) -> bool {
        match cap {
            Capability::CreateDelta { vector_patterns, operation_types } => {
                // Check vector pattern
                let vector_match = vector_patterns.iter()
                    .any(|p| self.pattern_matches(p, vector_id));

                // Check operation type
                let op_match = operation_types.contains(&operation.operation_type());

                vector_match && op_match
            }
            Capability::Admin { scope: AdminScope::Full } => true,
            _ => false,
        }
    }

    fn pattern_matches(&self, pattern: &VectorPattern, vector_id: &VectorId) -> bool {
        match pattern {
            VectorPattern::Exact(id) => id == vector_id,
            VectorPattern::Prefix(prefix) => vector_id.starts_with(prefix),
            VectorPattern::Regex(re) => {
                regex::Regex::new(re)
                    .map(|r| r.is_match(vector_id))
                    .unwrap_or(false)
            }
            VectorPattern::Namespace(ns) => {
                vector_id.starts_with(&format!("{}:", ns))
            }
            VectorPattern::All => true,
        }
    }
}
```

#### 3. Rate Limiting and DoS Protection

```rust
/// Rate limiter for delta operations
pub struct DeltaRateLimiter {
    /// Per-client limits
    client_limits: DashMap<ClientId, TokenBucket>,
    /// Per-vector limits
    vector_limits: DashMap<VectorId, TokenBucket>,
    /// Global limit
    global_limit: TokenBucket,
    /// Configuration
    config: RateLimitConfig,
}

/// Token bucket for rate limiting
pub struct TokenBucket {
    /// Current tokens
    tokens: AtomicF64,
    /// Last refill time
    last_refill: AtomicU64,
    /// Tokens per second
    rate: f64,
    /// Maximum tokens
    capacity: f64,
}

impl TokenBucket {
    /// Try to consume tokens
    pub fn try_consume(&self, tokens: f64) -> bool {
        // Refill based on elapsed time
        self.refill();

        loop {
            let current = self.tokens.load(Ordering::Relaxed);
            if current < tokens {
                return false;
            }

            if self.tokens.compare_exchange(
                current,
                current - tokens,
                Ordering::SeqCst,
                Ordering::Relaxed,
            ).is_ok() {
                return true;
            }
        }
    }

    fn refill(&self) {
        let now = Instant::now().elapsed().as_millis() as u64;
        let last = self.last_refill.load(Ordering::Relaxed);
        let elapsed = (now - last) as f64 / 1000.0;

        let new_tokens = (self.tokens.load(Ordering::Relaxed) + elapsed * self.rate)
            .min(self.capacity);

        self.tokens.store(new_tokens, Ordering::Relaxed);
        self.last_refill.store(now, Ordering::Relaxed);
    }
}

impl DeltaRateLimiter {
    /// Check if operation is allowed
    pub fn check(&self, client_id: &ClientId, vector_id: &VectorId) -> Result<(), RateLimitError> {
        // Check global limit
        if !self.global_limit.try_consume(1.0) {
            return Err(RateLimitError::GlobalLimitExceeded);
        }

        // Check client limit
        let client_bucket = self.client_limits
            .entry(client_id.clone())
            .or_insert_with(|| TokenBucket::new(
                self.config.client_rate,
                self.config.client_burst,
            ));

        if !client_bucket.try_consume(1.0) {
            return Err(RateLimitError::ClientLimitExceeded);
        }

        // Check vector limit (prevent hot-key abuse)
        let vector_bucket = self.vector_limits
            .entry(vector_id.clone())
            .or_insert_with(|| TokenBucket::new(
                self.config.vector_rate,
                self.config.vector_burst,
            ));

        if !vector_bucket.try_consume(1.0) {
            return Err(RateLimitError::VectorLimitExceeded);
        }

        Ok(())
    }
}
```

#### 4. Input Validation

```rust
/// Delta input validator
pub struct DeltaValidator {
    /// Maximum delta size
    max_delta_size: usize,
    /// Maximum dimensions
    max_dimensions: usize,
    /// Allowed operation types
    allowed_operations: HashSet<OperationType>,
    /// Metadata schema (optional)
    metadata_schema: Option<JsonSchema>,
}

impl DeltaValidator {
    /// Validate a delta before processing
    pub fn validate(&self, delta: &VectorDelta) -> Result<(), ValidationError> {
        // Check delta ID format
        self.validate_id(&delta.delta_id)?;
        self.validate_id(&delta.vector_id)?;

        // Check operation type allowed
        if !self.allowed_operations.contains(&delta.operation.operation_type()) {
            return Err(ValidationError::DisallowedOperation);
        }

        // Validate operation content
        self.validate_operation(&delta.operation)?;

        // Validate metadata if present
        if let Some(metadata) = &delta.metadata_delta {
            self.validate_metadata(metadata)?;
        }

        // Check timestamp is sane
        self.validate_timestamp(delta.timestamp)?;

        Ok(())
    }

    fn validate_id(&self, id: &str) -> Result<(), ValidationError> {
        // Check length
        if id.len() > 256 {
            return Err(ValidationError::IdTooLong);
        }

        // Check for path traversal
        if id.contains("..") || id.contains('/') || id.contains('\\') {
            return Err(ValidationError::InvalidIdChars);
        }

        // Check for null bytes
        if id.contains('\0') {
            return Err(ValidationError::InvalidIdChars);
        }

        Ok(())
    }

    fn validate_operation(&self, op: &DeltaOperation) -> Result<(), ValidationError> {
        match op {
            DeltaOperation::Sparse { indices, values } => {
                // Check arrays have same length
                if indices.len() != values.len() {
                    return Err(ValidationError::MismatchedArrayLengths);
                }

                // Check indices are valid
                for &idx in indices {
                    if idx as usize >= self.max_dimensions {
                        return Err(ValidationError::IndexOutOfBounds);
                    }
                }

                // Check for NaN/Inf values
                for &val in values {
                    if !val.is_finite() {
                        return Err(ValidationError::InvalidValue);
                    }
                }

                // Check total size
                if indices.len() * 8 > self.max_delta_size {
                    return Err(ValidationError::DeltaTooLarge);
                }
            }

            DeltaOperation::Dense { vector } => {
                // Check dimensions
                if vector.len() > self.max_dimensions {
                    return Err(ValidationError::TooManyDimensions);
                }

                // Check for NaN/Inf
                for &val in vector {
                    if !val.is_finite() {
                        return Err(ValidationError::InvalidValue);
                    }
                }

                // Check size
                if vector.len() * 4 > self.max_delta_size {
                    return Err(ValidationError::DeltaTooLarge);
                }
            }

            DeltaOperation::Scale { factor } => {
                if !factor.is_finite() || *factor == 0.0 {
                    return Err(ValidationError::InvalidValue);
                }
            }

            _ => {}
        }

        Ok(())
    }

    fn validate_timestamp(&self, ts: DateTime<Utc>) -> Result<(), ValidationError> {
        let now = Utc::now();
        let age = now.signed_duration_since(ts);

        // Reject timestamps too far in the past (7 days)
        if age.num_days() > 7 {
            return Err(ValidationError::TimestampTooOld);
        }

        // Reject timestamps in the future (with 5 min tolerance)
        if age.num_minutes() < -5 {
            return Err(ValidationError::TimestampInFuture);
        }

        Ok(())
    }
}
```

---

## Threat Model Analysis

### Attack Vectors and Mitigations

| Attack | Vector | Mitigation | Residual Risk |
|--------|--------|------------|---------------|
| Delta tampering | Network MitM | TLS + signatures | Low |
| Replay attack | Network replay | Nonces + timestamp | Low |
| Unauthorized access | API abuse | Capability tokens | Low |
| Data exfiltration | Side channels | Rate limiting | Medium |
| DoS flooding | Request flood | Rate limiting | Medium |
| Key compromise | Key theft | Key rotation | Medium |
| Privilege escalation | Token forge | Signature verification | Low |
| Input injection | Malformed delta | Input validation | Low |

### Security Guarantees

| Guarantee | Mechanism | Strength |
|-----------|-----------|----------|
| Integrity | Ed25519 signatures | Cryptographic |
| Authentication | mTLS + tokens | Cryptographic |
| Authorization | Capability tokens | Logical |
| Replay protection | Nonces + timestamps | Probabilistic |
| Rate limiting | Token buckets | Statistical |

---

## Considered Options

### Option 1: Simple API Keys

**Description**: Basic API key authentication.

**Pros**:
- Simple to implement
- Easy to understand

**Cons**:
- No fine-grained control
- Key compromise is catastrophic
- No delta-level security

**Verdict**: Rejected - insufficient for delta integrity.

### Option 2: JWT Tokens

**Description**: Standard JWT for authentication.

**Pros**:
- Industry standard
- Rich ecosystem

**Cons**:
- No per-delta signatures
- Revocation complexity
- Limited capability model

**Verdict**: Partially adopted - used alongside capabilities.

### Option 3: Signed Deltas + Capabilities (Selected)

**Description**: Cryptographic signatures on deltas with capability-based auth.

**Pros**:
- Delta-level integrity
- Fine-grained authorization
- Non-repudiation
- Composable security

**Cons**:
- Complexity
- Performance overhead
- Key management

**Verdict**: Adopted - provides comprehensive security.

### Option 4: Zero-Knowledge Proofs

**Description**: ZK proofs for privacy-preserving updates.

**Pros**:
- Maximum privacy
- Verifiable computation

**Cons**:
- Very complex
- High overhead
- Limited tooling

**Verdict**: Deferred - consider for future privacy features.

---

## Technical Specification

### Security Configuration

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Enable delta signing
    pub signing_enabled: bool,
    /// Signing algorithm
    pub signing_algorithm: SigningAlgorithm,
    /// Enable capability tokens
    pub capabilities_enabled: bool,
    /// Token issuer public keys
    pub trusted_issuers: Vec<TrustedIssuer>,
    /// Rate limiting configuration
    pub rate_limits: RateLimitConfig,
    /// Input validation configuration
    pub validation: ValidationConfig,
    /// Clock skew tolerance
    pub clock_tolerance: Duration,
    /// Nonce window (for replay protection)
    pub nonce_window: Duration,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            signing_enabled: true,
            signing_algorithm: SigningAlgorithm::Ed25519,
            capabilities_enabled: true,
            trusted_issuers: vec![],
            rate_limits: RateLimitConfig {
                global_rate: 100_000.0,  // 100K ops/s global
                client_rate: 1000.0,     // 1K ops/s per client
                client_burst: 100.0,
                vector_rate: 100.0,      // 100 ops/s per vector
                vector_burst: 10.0,
            },
            validation: ValidationConfig {
                max_delta_size: 1024 * 1024,  // 1MB
                max_dimensions: 4096,
                max_metadata_size: 65536,
            },
            clock_tolerance: Duration::from_secs(300),  // 5 minutes
            nonce_window: Duration::from_secs(86400),   // 24 hours
        }
    }
}
```

### Wire Format for Signed Delta

```
Signed Delta Format:
+--------+--------+--------+--------+--------+--------+--------+--------+
| Magic  | Version| Flags  |    Reserved      |    Delta Length         |
| 0x53   | 0x01   |        |                  |    (32-bit LE)          |
+--------+--------+--------+--------+--------+--------+--------+--------+
|                         Delta Payload                                 |
|                    (VectorDelta, encoded)                             |
+-----------------------------------------------------------------------+
|                         Key ID (32 bytes)                             |
+-----------------------------------------------------------------------+
|                     Timestamp (64-bit LE, Unix ms)                    |
+-----------------------------------------------------------------------+
|                         Nonce (16 bytes)                              |
+-----------------------------------------------------------------------+
|                     Signature (64 bytes, Ed25519)                     |
+-----------------------------------------------------------------------+

Flags:
  bit 0: Compressed delta payload
  bit 1: Has capability token attached
  bits 2-7: Reserved
```

---

## Consequences

### Benefits

1. **Integrity**: Tamper-proof deltas with cryptographic verification
2. **Authorization**: Fine-grained capability-based access control
3. **Auditability**: Non-repudiation through signatures
4. **Resilience**: DoS protection through rate limiting
5. **Flexibility**: Configurable security levels

### Risks and Mitigations

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Key compromise | Low | Critical | Key rotation, HSM |
| Performance overhead | Medium | Medium | Batch verification |
| Configuration errors | Medium | High | Secure defaults |
| Clock drift | Low | Medium | NTP, tolerance |

---

## References

1. NIST SP 800-63: Digital Identity Guidelines
2. RFC 8032: Edwards-Curve Digital Signature Algorithm (EdDSA)
3. ADR-DB-001: Delta Behavior Core Architecture
4. ADR-007: Security Review & Technical Debt

---

## Related Decisions

- **ADR-DB-001**: Delta Behavior Core Architecture
- **ADR-DB-003**: Delta Propagation Protocol
- **ADR-DB-009**: Delta Observability
- **ADR-007**: Security Review & Technical Debt
