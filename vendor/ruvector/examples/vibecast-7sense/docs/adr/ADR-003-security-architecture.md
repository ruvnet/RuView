# ADR-003: Security Architecture for 7sense Bioacoustics Platform

## Status

**Accepted**

## Date

2026-01-15

## Context

7sense is a bioacoustics platform that processes audio recordings of wildlife vocalizations, generates embeddings using the Perch 2.0 ONNX model, and stores them in a RuVector vector database for similarity search and pattern analysis. The platform implements Retrieval-Augmented Bioacoustics (RAB) for evidence-based interpretation of wildlife communication patterns.

### Security-Critical Components

1. **Audio Processing Pipeline**: Ingests 5-second mono audio at 32kHz (160,000 samples)
2. **Perch 2.0 ONNX Model**: Generates 1536-dimensional embeddings from mel spectrograms
3. **RuVector Database**: Stores embeddings with HNSW indexing and GNN learning layers
4. **RAB Evidence Packs**: Aggregates retrieval results with provenance for interpretations
5. **API Layer**: Exposes search, ingestion, and analysis capabilities

### Regulatory Considerations

- Endangered Species Act (ESA) compliance for protected species data
- CITES requirements for international wildlife data sharing
- Research ethics for sensitive habitat location data
- Data sovereignty for indigenous lands recordings

## Decision

We will implement a defense-in-depth security architecture with the following layers:

### 1. Threat Model

#### 1.1 Primary Threat Actors

| Actor | Motivation | Capability | Risk Level |
|-------|------------|------------|------------|
| Data Exfiltrators | Steal research data, endangered species locations | Moderate-High | Critical |
| Model Poisoners | Corrupt embeddings to degrade analysis quality | Moderate | High |
| Inference Attackers | Extract training data or model internals | High | High |
| Malicious Researchers | Upload harmful content, abuse API | Low-Moderate | Medium |
| Script Kiddies | Automated scanning, opportunistic attacks | Low | Low |

#### 1.2 Attack Vectors

```
                         ATTACK SURFACE MAP
    +------------------------------------------------------------------+
    |                        API BOUNDARY                               |
    |  [Audio Upload] [Search Query] [Batch Ingestion] [Admin Endpoints]|
    +------------------------------------------------------------------+
              |              |              |              |
              v              v              v              v
    +------------------------------------------------------------------+
    |                    INPUT VALIDATION LAYER                        |
    |  - Audio format validation      - Query sanitization             |
    |  - File size limits             - Rate limiting                  |
    |  - Path traversal prevention    - Authentication check           |
    +------------------------------------------------------------------+
              |              |              |              |
              v              v              v              v
    +------------------------------------------------------------------+
    |                    PROCESSING LAYER                              |
    |  - ONNX model sandboxing        - Memory bounds checking         |
    |  - Embedding normalization      - Resource quotas                |
    +------------------------------------------------------------------+
              |              |              |              |
              v              v              v              v
    +------------------------------------------------------------------+
    |                    STORAGE LAYER                                 |
    |  - Encrypted at rest            - Access control (RBAC)          |
    |  - Audit logging                - Data classification            |
    +------------------------------------------------------------------+
```

#### 1.3 Threat Scenarios

**T1: Model Poisoning via Malicious Audio**
- Attack: Upload crafted audio that produces adversarial embeddings
- Impact: Corrupts similarity search, clusters benign calls with malicious
- Mitigation: Embedding bounds validation, anomaly detection on insertions

**T2: Inference Attack on Embeddings**
- Attack: Query embeddings to reconstruct original audio or model weights
- Impact: Intellectual property theft, privacy breach
- Mitigation: Differential privacy on query results, rate limiting

**T3: Path Traversal on Audio Storage**
- Attack: Manipulate file paths to access system files
- Impact: System compromise, data exfiltration
- Mitigation: Strict path canonicalization, chroot-style isolation

**T4: Protected Species Location Leakage**
- Attack: Correlate audio metadata to locate endangered species
- Impact: Poaching risk, regulatory violations
- Mitigation: Location fuzzing, access tiering, audit logging

**T5: RAB Attribution Manipulation**
- Attack: Forge or modify evidence pack citations
- Impact: Loss of scientific integrity, misinformation
- Mitigation: Cryptographic signatures on RAB outputs

### 2. Input Validation Strategy

#### 2.1 Audio File Validation

```rust
// audio_validator.rs

use std::io::{Read, Seek, SeekFrom};

pub struct AudioValidationConfig {
    pub max_file_size: usize,           // 50 MB default
    pub allowed_formats: Vec<String>,    // ["wav", "flac", "ogg"]
    pub required_sample_rate: u32,       // 32000 Hz (Perch 2.0 requirement)
    pub required_channels: u8,           // 1 (mono)
    pub max_duration_seconds: f64,       // 300.0 (5 minutes)
    pub min_duration_seconds: f64,       // 0.5
}

pub enum AudioValidationError {
    FileTooLarge { size: usize, max: usize },
    UnsupportedFormat { format: String },
    InvalidSampleRate { found: u32, expected: u32 },
    InvalidChannels { found: u8, expected: u8 },
    DurationOutOfRange { duration: f64 },
    MalformedHeader,
    SuspiciousPayload { reason: String },
}

pub fn validate_audio_file<R: Read + Seek>(
    reader: &mut R,
    config: &AudioValidationConfig,
) -> Result<AudioMetadata, AudioValidationError> {
    // 1. Check file size without loading entire file
    let file_size = reader.seek(SeekFrom::End(0))? as usize;
    reader.seek(SeekFrom::Start(0))?;

    if file_size > config.max_file_size {
        return Err(AudioValidationError::FileTooLarge {
            size: file_size,
            max: config.max_file_size,
        });
    }

    // 2. Validate magic bytes for format detection
    let mut magic = [0u8; 12];
    reader.read_exact(&mut magic)?;
    reader.seek(SeekFrom::Start(0))?;

    let format = detect_audio_format(&magic)?;
    if !config.allowed_formats.contains(&format) {
        return Err(AudioValidationError::UnsupportedFormat { format });
    }

    // 3. Parse and validate header (format-specific)
    let metadata = parse_audio_metadata(reader, &format)?;

    // 4. Validate sample rate matches Perch 2.0 requirement
    if metadata.sample_rate != config.required_sample_rate {
        return Err(AudioValidationError::InvalidSampleRate {
            found: metadata.sample_rate,
            expected: config.required_sample_rate,
        });
    }

    // 5. Validate mono channel requirement
    if metadata.channels != config.required_channels {
        return Err(AudioValidationError::InvalidChannels {
            found: metadata.channels,
            expected: config.required_channels,
        });
    }

    // 6. Validate duration bounds
    if metadata.duration < config.min_duration_seconds
        || metadata.duration > config.max_duration_seconds {
        return Err(AudioValidationError::DurationOutOfRange {
            duration: metadata.duration,
        });
    }

    // 7. Scan for suspicious embedded content
    scan_for_polyglot_attacks(reader)?;

    Ok(metadata)
}

fn scan_for_polyglot_attacks<R: Read + Seek>(reader: &mut R) -> Result<(), AudioValidationError> {
    // Check for embedded executables, scripts, or other dangerous payloads
    // that could exploit audio parser vulnerabilities
    let mut buffer = [0u8; 4096];
    reader.seek(SeekFrom::Start(0))?;

    while let Ok(n) = reader.read(&mut buffer) {
        if n == 0 { break; }

        // Check for common executable signatures
        if contains_executable_signature(&buffer[..n]) {
            return Err(AudioValidationError::SuspiciousPayload {
                reason: "Embedded executable detected".into(),
            });
        }

        // Check for script injection patterns
        if contains_script_patterns(&buffer[..n]) {
            return Err(AudioValidationError::SuspiciousPayload {
                reason: "Script content detected".into(),
            });
        }
    }

    reader.seek(SeekFrom::Start(0))?;
    Ok(())
}
```

#### 2.2 Embedding Bounds Validation

```rust
// embedding_validator.rs

pub struct EmbeddingValidationConfig {
    pub expected_dimensions: usize,      // 1536 for Perch 2.0
    pub max_l2_norm: f32,                // 100.0 (generous bound)
    pub min_l2_norm: f32,                // 0.01 (detect collapsed embeddings)
    pub max_element_value: f32,          // 50.0
    pub min_element_value: f32,          // -50.0
    pub nan_policy: NanPolicy,           // Reject
    pub inf_policy: InfPolicy,           // Reject
}

pub enum EmbeddingValidationError {
    DimensionMismatch { found: usize, expected: usize },
    NormOutOfBounds { norm: f32, min: f32, max: f32 },
    ElementOutOfBounds { index: usize, value: f32 },
    ContainsNaN { indices: Vec<usize> },
    ContainsInf { indices: Vec<usize> },
    SuspiciousPattern { reason: String },
}

pub fn validate_embedding(
    embedding: &[f32],
    config: &EmbeddingValidationConfig,
) -> Result<EmbeddingStats, EmbeddingValidationError> {
    // 1. Dimension check
    if embedding.len() != config.expected_dimensions {
        return Err(EmbeddingValidationError::DimensionMismatch {
            found: embedding.len(),
            expected: config.expected_dimensions,
        });
    }

    let mut nan_indices = Vec::new();
    let mut inf_indices = Vec::new();
    let mut sum_squares = 0.0f64;

    for (i, &val) in embedding.iter().enumerate() {
        // 2. NaN check
        if val.is_nan() {
            nan_indices.push(i);
            continue;
        }

        // 3. Infinity check
        if val.is_infinite() {
            inf_indices.push(i);
            continue;
        }

        // 4. Element bounds check
        if val < config.min_element_value || val > config.max_element_value {
            return Err(EmbeddingValidationError::ElementOutOfBounds {
                index: i,
                value: val,
            });
        }

        sum_squares += (val as f64) * (val as f64);
    }

    // Report NaN/Inf based on policy
    if !nan_indices.is_empty() {
        return Err(EmbeddingValidationError::ContainsNaN { indices: nan_indices });
    }
    if !inf_indices.is_empty() {
        return Err(EmbeddingValidationError::ContainsInf { indices: inf_indices });
    }

    // 5. L2 norm bounds check
    let l2_norm = (sum_squares as f32).sqrt();
    if l2_norm < config.min_l2_norm || l2_norm > config.max_l2_norm {
        return Err(EmbeddingValidationError::NormOutOfBounds {
            norm: l2_norm,
            min: config.min_l2_norm,
            max: config.max_l2_norm,
        });
    }

    // 6. Statistical anomaly detection
    detect_adversarial_patterns(embedding)?;

    Ok(EmbeddingStats {
        l2_norm,
        mean: embedding.iter().sum::<f32>() / embedding.len() as f32,
        variance: compute_variance(embedding),
    })
}

fn detect_adversarial_patterns(embedding: &[f32]) -> Result<(), EmbeddingValidationError> {
    // Detect patterns indicative of adversarial manipulation:
    // - Unusual sparsity (most values zero)
    // - Extreme clustering at specific values
    // - Patterns inconsistent with learned embedding distribution

    let zero_count = embedding.iter().filter(|&&v| v.abs() < 1e-6).count();
    let sparsity = zero_count as f32 / embedding.len() as f32;

    if sparsity > 0.95 {
        return Err(EmbeddingValidationError::SuspiciousPattern {
            reason: format!("Abnormal sparsity: {:.2}%", sparsity * 100.0),
        });
    }

    Ok(())
}
```

### 3. Path Traversal Prevention

```rust
// path_security.rs

use std::path::{Path, PathBuf, Component};

pub struct SecurePathConfig {
    pub audio_root: PathBuf,          // /data/audio
    pub embedding_root: PathBuf,      // /data/embeddings
    pub model_root: PathBuf,          // /models
    pub temp_root: PathBuf,           // /tmp/sevensense
    pub max_path_depth: usize,        // 10
    pub allowed_extensions: Vec<String>,
}

pub enum PathSecurityError {
    PathTraversalAttempt { path: String, reason: String },
    OutsideAllowedRoot { path: String, root: String },
    DisallowedExtension { ext: String },
    SymlinkDetected { path: String },
    PathTooDeep { depth: usize, max: usize },
    InvalidUtf8,
    NullByteDetected,
}

/// Sanitize and validate a user-provided path against traversal attacks.
///
/// CRITICAL: This function MUST be called for ALL user-provided file paths.
pub fn secure_path(
    user_path: &str,
    allowed_root: &Path,
    config: &SecurePathConfig,
) -> Result<PathBuf, PathSecurityError> {
    // 1. Check for null bytes (common bypass technique)
    if user_path.contains('\0') {
        return Err(PathSecurityError::NullByteDetected);
    }

    // 2. Check for URL encoding bypass attempts
    let decoded = percent_decode(user_path)?;

    // 3. Reject paths with explicit traversal sequences
    let dangerous_patterns = [
        "..", "..\\", "../", "..%2f", "..%5c",
        "%2e%2e", "%252e%252e",  // Double encoding
        "....//", "....\\\\",    // Variant bypasses
    ];

    let lower = decoded.to_lowercase();
    for pattern in &dangerous_patterns {
        if lower.contains(pattern) {
            return Err(PathSecurityError::PathTraversalAttempt {
                path: user_path.to_string(),
                reason: format!("Contains dangerous pattern: {}", pattern),
            });
        }
    }

    // 4. Parse and canonicalize the path
    let user_path_buf = PathBuf::from(&decoded);

    // 5. Validate each component
    let mut depth = 0;
    for component in user_path_buf.components() {
        match component {
            Component::ParentDir => {
                return Err(PathSecurityError::PathTraversalAttempt {
                    path: user_path.to_string(),
                    reason: "Parent directory reference detected".into(),
                });
            }
            Component::Normal(segment) => {
                depth += 1;
                // Validate segment doesn't contain hidden traversal
                let seg_str = segment.to_str()
                    .ok_or(PathSecurityError::InvalidUtf8)?;
                if seg_str.starts_with('.') && seg_str.len() > 1 {
                    // Allow single dot but reject hidden files/dirs
                    if seg_str != "." {
                        return Err(PathSecurityError::PathTraversalAttempt {
                            path: user_path.to_string(),
                            reason: "Hidden file/directory not allowed".into(),
                        });
                    }
                }
            }
            _ => {}
        }
    }

    // 6. Check path depth
    if depth > config.max_path_depth {
        return Err(PathSecurityError::PathTooDeep {
            depth,
            max: config.max_path_depth,
        });
    }

    // 7. Construct the final path within the allowed root
    let final_path = allowed_root.join(&user_path_buf);

    // 8. Canonicalize and verify it's still under the root
    // Note: We canonicalize the root first to handle symlinks in the root itself
    let canonical_root = allowed_root.canonicalize()
        .map_err(|_| PathSecurityError::PathTraversalAttempt {
            path: user_path.to_string(),
            reason: "Root path resolution failed".into(),
        })?;

    // For new files, canonicalize parent and append filename
    let canonical_final = if final_path.exists() {
        final_path.canonicalize()
            .map_err(|_| PathSecurityError::PathTraversalAttempt {
                path: user_path.to_string(),
                reason: "Path resolution failed".into(),
            })?
    } else {
        let parent = final_path.parent()
            .ok_or(PathSecurityError::PathTraversalAttempt {
                path: user_path.to_string(),
                reason: "Invalid parent path".into(),
            })?;
        let filename = final_path.file_name()
            .ok_or(PathSecurityError::PathTraversalAttempt {
                path: user_path.to_string(),
                reason: "Missing filename".into(),
            })?;

        parent.canonicalize()
            .map_err(|_| PathSecurityError::PathTraversalAttempt {
                path: user_path.to_string(),
                reason: "Parent path resolution failed".into(),
            })?
            .join(filename)
    };

    // 9. Final containment check
    if !canonical_final.starts_with(&canonical_root) {
        return Err(PathSecurityError::OutsideAllowedRoot {
            path: canonical_final.display().to_string(),
            root: canonical_root.display().to_string(),
        });
    }

    // 10. Check for symlinks (optional, depending on policy)
    if final_path.exists() && final_path.symlink_metadata()?.file_type().is_symlink() {
        return Err(PathSecurityError::SymlinkDetected {
            path: user_path.to_string(),
        });
    }

    // 11. Validate extension if applicable
    if let Some(ext) = canonical_final.extension() {
        let ext_str = ext.to_str().ok_or(PathSecurityError::InvalidUtf8)?;
        if !config.allowed_extensions.contains(&ext_str.to_lowercase()) {
            return Err(PathSecurityError::DisallowedExtension {
                ext: ext_str.to_string(),
            });
        }
    }

    Ok(canonical_final)
}
```

### 4. API Security

#### 4.1 Authentication Architecture

```rust
// auth.rs

use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use rand::rngs::OsRng;

/// Authentication configuration - NO HARDCODED CREDENTIALS
pub struct AuthConfig {
    /// JWT signing key - MUST be loaded from environment or secure vault
    pub jwt_secret: String,
    /// Token expiration in seconds
    pub token_expiry_secs: u64,
    /// Refresh token expiration in seconds
    pub refresh_expiry_secs: u64,
    /// Argon2 parameters for password hashing
    pub argon2_params: Argon2Params,
}

pub struct Argon2Params {
    pub memory_cost: u32,      // 65536 (64 MB)
    pub time_cost: u32,        // 3 iterations
    pub parallelism: u32,      // 4 threads
    pub output_length: usize,  // 32 bytes
}

impl Default for Argon2Params {
    fn default() -> Self {
        Self {
            memory_cost: 65536,
            time_cost: 3,
            parallelism: 4,
            output_length: 32,
        }
    }
}

/// Hash password using Argon2id (OWASP recommended)
pub fn hash_password(password: &str, params: &Argon2Params) -> Result<String, AuthError> {
    let salt = argon2::password_hash::SaltString::generate(&mut OsRng);

    let argon2 = Argon2::new(
        argon2::Algorithm::Argon2id,
        argon2::Version::V0x13,
        argon2::Params::new(
            params.memory_cost,
            params.time_cost,
            params.parallelism,
            Some(params.output_length),
        ).map_err(|e| AuthError::HashingError(e.to_string()))?,
    );

    let hash = argon2.hash_password(password.as_bytes(), &salt)
        .map_err(|e| AuthError::HashingError(e.to_string()))?;

    Ok(hash.to_string())
}

/// Verify password against stored hash
pub fn verify_password(password: &str, hash: &str) -> Result<bool, AuthError> {
    let parsed_hash = PasswordHash::new(hash)
        .map_err(|e| AuthError::VerificationError(e.to_string()))?;

    let argon2 = Argon2::default();

    Ok(argon2.verify_password(password.as_bytes(), &parsed_hash).is_ok())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,           // User ID
    pub role: UserRole,        // Access level
    pub exp: u64,              // Expiration timestamp
    pub iat: u64,              // Issued at
    pub jti: String,           // Unique token ID (for revocation)
    pub permissions: Vec<Permission>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum UserRole {
    Public,          // Read-only access to public data
    Researcher,      // Read/write access to research data
    DataCurator,     // Can modify data classifications
    Administrator,   // Full system access
    Service,         // Machine-to-machine authentication
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Permission {
    AudioRead,
    AudioWrite,
    AudioDelete,
    EmbeddingRead,
    EmbeddingWrite,
    ProtectedSpeciesRead,  // Requires additional verification
    ProtectedSpeciesWrite,
    ModelExecute,
    AdminAccess,
    AuditLogRead,
}
```

#### 4.2 Rate Limiting

```rust
// rate_limiter.rs

use std::collections::HashMap;
use std::time::{Duration, Instant};
use parking_lot::RwLock;

pub struct RateLimiterConfig {
    /// Limits per endpoint category
    pub limits: HashMap<EndpointCategory, RateLimit>,
    /// Global limit across all endpoints
    pub global_limit: RateLimit,
    /// Penalty multiplier for repeated violations
    pub violation_penalty: f32,
    /// Max penalty duration
    pub max_penalty_duration: Duration,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum EndpointCategory {
    AudioUpload,
    EmbeddingQuery,
    BatchIngestion,
    Search,
    Admin,
    ProtectedData,
}

#[derive(Debug, Clone)]
pub struct RateLimit {
    /// Requests allowed per window
    pub requests: u32,
    /// Time window duration
    pub window: Duration,
    /// Burst allowance (token bucket)
    pub burst: u32,
    /// Cost per request (for weighted limiting)
    pub cost: u32,
}

impl Default for RateLimiterConfig {
    fn default() -> Self {
        let mut limits = HashMap::new();

        // Conservative defaults - adjust based on capacity
        limits.insert(EndpointCategory::AudioUpload, RateLimit {
            requests: 100,
            window: Duration::from_secs(3600), // 100/hour
            burst: 10,
            cost: 10,
        });

        limits.insert(EndpointCategory::EmbeddingQuery, RateLimit {
            requests: 1000,
            window: Duration::from_secs(60), // 1000/minute
            burst: 50,
            cost: 1,
        });

        limits.insert(EndpointCategory::Search, RateLimit {
            requests: 500,
            window: Duration::from_secs(60), // 500/minute
            burst: 20,
            cost: 1,
        });

        limits.insert(EndpointCategory::BatchIngestion, RateLimit {
            requests: 10,
            window: Duration::from_secs(3600), // 10/hour
            burst: 2,
            cost: 100,
        });

        limits.insert(EndpointCategory::ProtectedData, RateLimit {
            requests: 50,
            window: Duration::from_secs(3600), // 50/hour
            burst: 5,
            cost: 20,
        });

        limits.insert(EndpointCategory::Admin, RateLimit {
            requests: 100,
            window: Duration::from_secs(60), // 100/minute
            burst: 10,
            cost: 5,
        });

        Self {
            limits,
            global_limit: RateLimit {
                requests: 10000,
                window: Duration::from_secs(60),
                burst: 100,
                cost: 1,
            },
            violation_penalty: 2.0,
            max_penalty_duration: Duration::from_secs(86400), // 24 hours
        }
    }
}

pub struct TokenBucket {
    tokens: f32,
    max_tokens: f32,
    refill_rate: f32,  // tokens per second
    last_refill: Instant,
}

impl TokenBucket {
    pub fn new(max_tokens: f32, refill_rate: f32) -> Self {
        Self {
            tokens: max_tokens,
            max_tokens,
            refill_rate,
            last_refill: Instant::now(),
        }
    }

    pub fn try_consume(&mut self, cost: f32) -> bool {
        self.refill();

        if self.tokens >= cost {
            self.tokens -= cost;
            true
        } else {
            false
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f32();
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.max_tokens);
        self.last_refill = now;
    }
}
```

### 5. Data Classification

```rust
// data_classification.rs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Data classification levels following sensitivity hierarchy
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum ClassificationLevel {
    /// Publicly available data, no restrictions
    Public = 0,

    /// Research data with attribution requirements
    Research = 1,

    /// Internal use only, not for public release
    Internal = 2,

    /// Sensitive habitat or behavioral data
    Sensitive = 3,

    /// Protected species data - regulatory restrictions
    Protected = 4,

    /// Classified/embargoed data - strict access control
    Restricted = 5,
}

/// Classification metadata for audio recordings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataClassification {
    /// Primary classification level
    pub level: ClassificationLevel,

    /// Specific classification tags
    pub tags: Vec<ClassificationTag>,

    /// Regulatory frameworks that apply
    pub regulations: Vec<Regulation>,

    /// Access requirements
    pub access_requirements: AccessRequirements,

    /// Retention policy
    pub retention: RetentionPolicy,

    /// Classification reason and justification
    pub rationale: String,

    /// Who assigned the classification
    pub classified_by: String,

    /// When the classification was assigned
    pub classified_at: DateTime<Utc>,

    /// Review date for reclassification
    pub review_date: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClassificationTag {
    /// Contains protected species vocalizations
    ProtectedSpecies { species_code: String, conservation_status: ConservationStatus },

    /// Contains precise location data
    PreciseLocation,

    /// Contains indigenous lands recordings
    IndigenousTerritory { territory_code: String },

    /// Contains breeding site information
    BreedingSite,

    /// Contains data under active research embargo
    ResearchEmbargo { lift_date: DateTime<Utc> },

    /// Contains personally identifiable information (researcher voices, etc.)
    PII,

    /// Commercial restrictions apply
    CommercialRestriction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConservationStatus {
    LeastConcern,
    NearThreatened,
    Vulnerable,
    Endangered,
    CriticallyEndangered,
    ExtinctInWild,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Regulation {
    /// US Endangered Species Act
    ESA { permit_required: bool, permit_number: Option<String> },

    /// Convention on International Trade in Endangered Species
    CITES { appendix: u8 },

    /// EU Habitats Directive
    HabitatsDirective,

    /// Migratory Bird Treaty Act
    MBTA,

    /// Institution-specific IRB approval
    IRB { protocol_number: String },

    /// Data sovereignty requirements
    DataSovereignty { jurisdiction: String },

    /// Custom regulatory framework
    Custom { name: String, requirements: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessRequirements {
    /// Minimum role required
    pub min_role: crate::auth::UserRole,

    /// Additional permissions required
    pub required_permissions: Vec<crate::auth::Permission>,

    /// Requires signed data use agreement
    pub requires_dua: bool,

    /// Requires institutional affiliation verification
    pub requires_affiliation: bool,

    /// Requires ethics approval
    pub requires_ethics_approval: bool,

    /// Geographic restrictions on access
    pub geographic_restrictions: Option<Vec<String>>,

    /// Time-based access restrictions
    pub time_restrictions: Option<TimeRestrictions>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRestrictions {
    /// Earliest time data can be accessed
    pub not_before: Option<DateTime<Utc>>,

    /// Latest time data can be accessed
    pub not_after: Option<DateTime<Utc>>,

    /// Seasonal restrictions (e.g., no access during breeding season)
    pub seasonal_blackouts: Vec<SeasonalBlackout>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeasonalBlackout {
    pub name: String,
    pub start_month: u8,
    pub start_day: u8,
    pub end_month: u8,
    pub end_day: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionPolicy {
    /// Minimum retention period
    pub min_retention: Duration,

    /// Maximum retention period (for PII, etc.)
    pub max_retention: Option<Duration>,

    /// Action after retention period
    pub post_retention_action: PostRetentionAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PostRetentionAction {
    Delete,
    Archive,
    Anonymize,
    Review,
}

/// Apply classification-based access control
pub fn check_access(
    classification: &DataClassification,
    user_role: &crate::auth::UserRole,
    user_permissions: &[crate::auth::Permission],
    context: &AccessContext,
) -> Result<(), AccessDeniedReason> {
    // Check role hierarchy
    if *user_role < classification.access_requirements.min_role {
        return Err(AccessDeniedReason::InsufficientRole {
            required: classification.access_requirements.min_role.clone(),
            actual: user_role.clone(),
        });
    }

    // Check required permissions
    for required in &classification.access_requirements.required_permissions {
        if !user_permissions.contains(required) {
            return Err(AccessDeniedReason::MissingPermission {
                required: required.clone(),
            });
        }
    }

    // Check geographic restrictions
    if let Some(ref allowed_regions) = classification.access_requirements.geographic_restrictions {
        if !allowed_regions.contains(&context.requester_region) {
            return Err(AccessDeniedReason::GeographicRestriction {
                requester_region: context.requester_region.clone(),
            });
        }
    }

    // Check time restrictions
    if let Some(ref time_restrictions) = classification.access_requirements.time_restrictions {
        let now = Utc::now();

        if let Some(not_before) = time_restrictions.not_before {
            if now < not_before {
                return Err(AccessDeniedReason::TemporalRestriction {
                    reason: format!("Data not available until {}", not_before),
                });
            }
        }

        if let Some(not_after) = time_restrictions.not_after {
            if now > not_after {
                return Err(AccessDeniedReason::TemporalRestriction {
                    reason: format!("Data access expired at {}", not_after),
                });
            }
        }
    }

    Ok(())
}
```

### 6. Audit Logging and Provenance

```rust
// audit.rs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use uuid::Uuid;

/// Immutable audit log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Unique entry ID
    pub id: Uuid,

    /// Timestamp of the event
    pub timestamp: DateTime<Utc>,

    /// Type of event
    pub event_type: AuditEventType,

    /// User or service that performed the action
    pub actor: Actor,

    /// Resource affected
    pub resource: Resource,

    /// Action performed
    pub action: Action,

    /// Outcome of the action
    pub outcome: Outcome,

    /// Additional context
    pub context: AuditContext,

    /// Hash of previous entry (blockchain-style chain)
    pub previous_hash: String,

    /// Hash of this entry
    pub entry_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditEventType {
    Authentication,
    Authorization,
    DataAccess,
    DataModification,
    DataDeletion,
    ModelExecution,
    ConfigurationChange,
    SecurityEvent,
    SystemEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Actor {
    pub actor_type: ActorType,
    pub id: String,
    pub name: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActorType {
    User,
    Service,
    System,
    Anonymous,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    pub resource_type: ResourceType,
    pub id: String,
    pub classification: Option<ClassificationLevel>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResourceType {
    AudioRecording,
    Embedding,
    Model,
    Query,
    Configuration,
    User,
    ApiKey,
    RABEvidencePack,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub action_type: ActionType,
    pub details: String,
    pub parameters: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionType {
    Create,
    Read,
    Update,
    Delete,
    Query,
    Export,
    Import,
    Execute,
    Authenticate,
    Authorize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Outcome {
    pub success: bool,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub affected_count: Option<u64>,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditContext {
    /// Request correlation ID for tracing
    pub correlation_id: String,

    /// Server that processed the request
    pub server_id: String,

    /// API endpoint or function
    pub endpoint: String,

    /// Request method
    pub method: String,

    /// Query or search terms (sanitized)
    pub query_sanitized: Option<String>,

    /// Data classification of accessed resources
    pub data_classification: Option<ClassificationLevel>,

    /// Regulatory frameworks involved
    pub regulations_involved: Vec<String>,
}

impl AuditEntry {
    pub fn new(
        event_type: AuditEventType,
        actor: Actor,
        resource: Resource,
        action: Action,
        outcome: Outcome,
        context: AuditContext,
        previous_hash: String,
    ) -> Self {
        let mut entry = Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            event_type,
            actor,
            resource,
            action,
            outcome,
            context,
            previous_hash,
            entry_hash: String::new(),
        };

        entry.entry_hash = entry.compute_hash();
        entry
    }

    fn compute_hash(&self) -> String {
        let mut hasher = Sha256::new();

        hasher.update(self.id.to_string().as_bytes());
        hasher.update(self.timestamp.to_rfc3339().as_bytes());
        hasher.update(serde_json::to_string(&self.event_type).unwrap().as_bytes());
        hasher.update(serde_json::to_string(&self.actor).unwrap().as_bytes());
        hasher.update(serde_json::to_string(&self.resource).unwrap().as_bytes());
        hasher.update(serde_json::to_string(&self.action).unwrap().as_bytes());
        hasher.update(serde_json::to_string(&self.outcome).unwrap().as_bytes());
        hasher.update(self.previous_hash.as_bytes());

        format!("{:x}", hasher.finalize())
    }

    pub fn verify_chain(&self, previous: &AuditEntry) -> bool {
        self.previous_hash == previous.entry_hash
    }
}

/// RAB Evidence Pack Provenance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RABProvenance {
    /// Unique provenance ID
    pub id: Uuid,

    /// When the evidence pack was generated
    pub generated_at: DateTime<Utc>,

    /// Query that triggered the generation
    pub query_id: String,

    /// Retrieved neighbors with source attribution
    pub retrieved_sources: Vec<RetrievedSource>,

    /// Model version used for embeddings
    pub embedding_model: ModelVersion,

    /// Search parameters used
    pub search_parameters: SearchParameters,

    /// Confidence metrics
    pub confidence: ConfidenceMetrics,

    /// Cryptographic signature for integrity
    pub signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievedSource {
    /// Source recording ID
    pub recording_id: String,

    /// Segment within recording
    pub segment_id: String,

    /// Distance/similarity score
    pub similarity_score: f32,

    /// Original data source (dataset name, institution)
    pub data_source: String,

    /// License/usage terms
    pub license: String,

    /// Attribution string
    pub attribution: String,

    /// Timestamp of source recording
    pub source_timestamp: Option<DateTime<Utc>>,

    /// Location (if not restricted)
    pub location: Option<FuzzedLocation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuzzedLocation {
    /// Fuzzing applied (for protected species)
    pub fuzzing_radius_km: f32,

    /// Fuzzed coordinates
    pub latitude: f64,
    pub longitude: f64,

    /// Region name (safe to disclose)
    pub region: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelVersion {
    pub name: String,
    pub version: String,
    pub hash: String,  // SHA256 of model weights
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchParameters {
    pub top_k: usize,
    pub distance_metric: String,
    pub min_similarity: f32,
    pub filters_applied: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceMetrics {
    /// Overall retrieval confidence
    pub retrieval_confidence: f32,

    /// Similarity distribution statistics
    pub similarity_mean: f32,
    pub similarity_std: f32,

    /// Number of sources above threshold
    pub high_confidence_count: usize,
}
```

### 7. Secure ONNX Model Execution

```rust
// model_security.rs

use std::path::Path;
use sha2::{Sha256, Digest};

/// Configuration for secure ONNX model execution
pub struct ONNXSecurityConfig {
    /// Expected model hash (SHA256)
    pub expected_model_hash: String,

    /// Maximum input tensor size (bytes)
    pub max_input_size: usize,

    /// Maximum output tensor size (bytes)
    pub max_output_size: usize,

    /// Execution timeout (milliseconds)
    pub execution_timeout_ms: u64,

    /// Memory limit for inference (bytes)
    pub memory_limit: usize,

    /// Allow GPU execution
    pub allow_gpu: bool,

    /// Allowed execution providers
    pub allowed_providers: Vec<String>,
}

impl Default for ONNXSecurityConfig {
    fn default() -> Self {
        Self {
            expected_model_hash: String::new(), // Must be set explicitly
            max_input_size: 160_000 * 4,        // 160k samples * 4 bytes (f32)
            max_output_size: 1536 * 4,          // 1536-dim embedding * 4 bytes
            execution_timeout_ms: 30_000,        // 30 seconds
            memory_limit: 2 * 1024 * 1024 * 1024, // 2 GB
            allow_gpu: true,
            allowed_providers: vec![
                "CPUExecutionProvider".into(),
                "CUDAExecutionProvider".into(),
            ],
        }
    }
}

pub struct SecureONNXRuntime {
    config: ONNXSecurityConfig,
    model_hash: String,
    // session: ort::Session, // actual ONNX runtime session
}

impl SecureONNXRuntime {
    /// Load and verify ONNX model
    pub fn load(model_path: &Path, config: ONNXSecurityConfig) -> Result<Self, ModelSecurityError> {
        // 1. Verify model file integrity
        let model_bytes = std::fs::read(model_path)
            .map_err(|e| ModelSecurityError::LoadError(e.to_string()))?;

        let mut hasher = Sha256::new();
        hasher.update(&model_bytes);
        let model_hash = format!("{:x}", hasher.finalize());

        if !config.expected_model_hash.is_empty() && model_hash != config.expected_model_hash {
            return Err(ModelSecurityError::IntegrityViolation {
                expected: config.expected_model_hash.clone(),
                actual: model_hash,
            });
        }

        // 2. Validate model structure (basic sanity checks)
        validate_onnx_structure(&model_bytes)?;

        // 3. Create ONNX runtime session with security constraints
        // let session = create_secure_session(&model_bytes, &config)?;

        Ok(Self {
            config,
            model_hash,
            // session,
        })
    }

    /// Execute inference with security constraints
    pub fn infer(&self, input: &[f32]) -> Result<Vec<f32>, ModelSecurityError> {
        // 1. Validate input size
        let input_bytes = input.len() * std::mem::size_of::<f32>();
        if input_bytes > self.config.max_input_size {
            return Err(ModelSecurityError::InputTooLarge {
                size: input_bytes,
                max: self.config.max_input_size,
            });
        }

        // 2. Validate input dimensions for Perch 2.0 (160,000 samples)
        if input.len() != 160_000 {
            return Err(ModelSecurityError::InvalidInputDimensions {
                expected: 160_000,
                actual: input.len(),
            });
        }

        // 3. Check for NaN/Inf in input
        for (i, &val) in input.iter().enumerate() {
            if val.is_nan() {
                return Err(ModelSecurityError::InvalidInputValue {
                    index: i,
                    reason: "NaN value".into(),
                });
            }
            if val.is_infinite() {
                return Err(ModelSecurityError::InvalidInputValue {
                    index: i,
                    reason: "Infinite value".into(),
                });
            }
        }

        // 4. Execute with timeout
        // let output = tokio::time::timeout(
        //     Duration::from_millis(self.config.execution_timeout_ms),
        //     self.session.run(input)
        // ).await??;

        // 5. Validate output
        // validate_output(&output, &self.config)?;

        // Placeholder - actual implementation uses ort crate
        Ok(vec![0.0; 1536])
    }
}

fn validate_onnx_structure(model_bytes: &[u8]) -> Result<(), ModelSecurityError> {
    // Basic ONNX format validation
    // Check magic bytes, version, graph structure

    if model_bytes.len() < 8 {
        return Err(ModelSecurityError::InvalidFormat("File too small".into()));
    }

    // ONNX files start with specific protobuf structure
    // This is a simplified check - production should use onnx crate for parsing

    Ok(())
}

#[derive(Debug)]
pub enum ModelSecurityError {
    LoadError(String),
    IntegrityViolation { expected: String, actual: String },
    InvalidFormat(String),
    InputTooLarge { size: usize, max: usize },
    InvalidInputDimensions { expected: usize, actual: usize },
    InvalidInputValue { index: usize, reason: String },
    ExecutionTimeout,
    MemoryExceeded,
    OutputValidationFailed(String),
}
```

### 8. Memory Safety (Rust Advantages)

```rust
// memory_safety.rs

//! 7sense leverages Rust's memory safety guarantees to prevent
//! entire classes of vulnerabilities common in systems handling
//! binary data (audio files, embeddings, model weights).

/// Key Memory Safety Features Utilized
///
/// 1. BUFFER OVERFLOW PREVENTION
///    - Rust's bounds checking on array/slice access
///    - No raw pointer arithmetic without unsafe blocks
///    - Example: Audio sample access is always bounds-checked
///
/// 2. USE-AFTER-FREE PREVENTION
///    - Ownership system ensures memory is freed exactly once
///    - Embedding vectors cannot be accessed after transfer
///    - Example: Once an embedding is moved to RuVector, caller cannot access it
///
/// 3. DATA RACE PREVENTION
///    - Send/Sync traits enforce thread-safe data sharing
///    - RuVector's concurrent access is compile-time verified
///    - Example: Concurrent embedding queries are proven race-free
///
/// 4. NULL POINTER PREVENTION
///    - Option<T> explicitly represents nullable values
///    - No null pointer dereferences possible
///    - Example: Missing metadata returns None, not crash
///
/// 5. INTEGER OVERFLOW PROTECTION
///    - Debug mode panics on overflow
///    - Release mode can use checked_* methods
///    - Example: Audio duration calculations use checked arithmetic

/// Safe audio buffer handling
pub struct AudioBuffer {
    samples: Vec<f32>,
    sample_rate: u32,
}

impl AudioBuffer {
    /// Create a new audio buffer with validated dimensions
    pub fn new(samples: Vec<f32>, sample_rate: u32) -> Result<Self, AudioError> {
        // Capacity is already allocated, no buffer overflow possible
        if samples.is_empty() {
            return Err(AudioError::EmptyBuffer);
        }

        // Checked arithmetic prevents integer overflow
        let duration_samples = samples.len();
        let _duration_seconds = duration_samples
            .checked_div(sample_rate as usize)
            .ok_or(AudioError::InvalidSampleRate)?;

        Ok(Self { samples, sample_rate })
    }

    /// Access samples safely - bounds checked at compile time with iterators
    pub fn iter(&self) -> impl Iterator<Item = &f32> {
        self.samples.iter()
    }

    /// Slice access - bounds checked at runtime, returns None if out of bounds
    pub fn get_segment(&self, start: usize, end: usize) -> Option<&[f32]> {
        self.samples.get(start..end)
    }
}

/// Safe embedding handling with ownership transfer
pub struct EmbeddingHandle {
    /// Private field prevents external construction
    embedding: Box<[f32; 1536]>,
    /// Metadata stays with the embedding
    metadata: EmbeddingMetadata,
}

impl EmbeddingHandle {
    /// Consume the handle to get the embedding - prevents double-use
    pub fn into_inner(self) -> Box<[f32; 1536]> {
        // self is moved here, cannot be used again
        self.embedding
    }

    /// Borrow for read-only access
    pub fn as_slice(&self) -> &[f32] {
        &self.embedding[..]
    }
}

/// Thread-safe shared state for concurrent embedding operations
pub struct ConcurrentEmbeddingStore {
    /// RwLock allows multiple readers or single writer
    /// Compile-time guaranteed no data races
    store: parking_lot::RwLock<std::collections::HashMap<String, EmbeddingHandle>>,
}

impl ConcurrentEmbeddingStore {
    pub fn new() -> Self {
        Self {
            store: parking_lot::RwLock::new(std::collections::HashMap::new()),
        }
    }

    /// Read access - multiple threads can read simultaneously
    pub fn get(&self, key: &str) -> Option<Vec<f32>> {
        let guard = self.store.read();
        guard.get(key).map(|h| h.as_slice().to_vec())
    }

    /// Write access - exclusive, blocks readers
    pub fn insert(&self, key: String, handle: EmbeddingHandle) {
        let mut guard = self.store.write();
        guard.insert(key, handle);
        // Lock released here, other threads can proceed
    }
}

/// Zeroing sensitive data on drop
pub struct SensitiveBuffer {
    data: Vec<u8>,
}

impl Drop for SensitiveBuffer {
    fn drop(&mut self) {
        // Explicitly zero memory before deallocation
        // Prevents sensitive data from lingering in freed memory
        for byte in &mut self.data {
            unsafe {
                std::ptr::write_volatile(byte, 0);
            }
        }
        // Compiler fence prevents optimization from removing the zeroing
        std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);
    }
}

#[derive(Debug)]
pub enum AudioError {
    EmptyBuffer,
    InvalidSampleRate,
}

pub struct EmbeddingMetadata {
    pub source_id: String,
    pub generated_at: chrono::DateTime<chrono::Utc>,
}
```

### 9. OWASP Top 10 Mitigations (Bioacoustics Domain)

| OWASP Category | 7sense-Specific Risk | Mitigation |
|----------------|------------------------|------------|
| **A01:2021 Broken Access Control** | Unauthorized access to protected species data | RBAC with classification-based access, location fuzzing for sensitive coordinates |
| **A02:2021 Cryptographic Failures** | Embedding data exposure, weak provenance | AES-256 encryption at rest, Ed25519 signatures on RAB evidence packs |
| **A03:2021 Injection** | Path traversal in audio storage, query injection in Cypher | Strict path canonicalization (Section 3), parameterized queries only |
| **A04:2021 Insecure Design** | Model poisoning via adversarial audio | Embedding bounds validation, anomaly detection on insertions |
| **A05:2021 Security Misconfiguration** | Exposed ONNX model internals, debug endpoints | Hardened default config, model integrity verification (Section 7) |
| **A06:2021 Vulnerable Components** | Outdated ONNX runtime, RuVector dependencies | Automated dependency scanning, pinned versions with hash verification |
| **A07:2021 Auth Failures** | Weak API key management, session hijacking | Argon2id hashing, short-lived JWTs, secure session management |
| **A08:2021 Data Integrity Failures** | Corrupted embeddings, falsified provenance | Hash-chained audit logs, cryptographic RAB signatures |
| **A09:2021 Logging Failures** | Missing audit trail for protected data access | Comprehensive audit logging (Section 6), immutable log chain |
| **A10:2021 SSRF** | Model loading from attacker-controlled URLs | Local-only model loading, no remote URL support |

### 10. Security Testing Requirements

```rust
// security_tests.rs

#[cfg(test)]
mod security_tests {
    use super::*;

    /// Test: Path traversal attempts must be rejected
    #[test]
    fn test_path_traversal_prevention() {
        let config = SecurePathConfig::default();
        let root = Path::new("/data/audio");

        let malicious_paths = [
            "../../../etc/passwd",
            "..\\..\\..\\windows\\system32\\config\\sam",
            "audio/../../secret",
            "audio%2f..%2f..%2fsecret",
            "audio\x00.wav",  // Null byte injection
            "....//....//etc/passwd",  // Bypass attempts
        ];

        for path in &malicious_paths {
            let result = secure_path(path, root, &config);
            assert!(result.is_err(), "Path should be rejected: {}", path);
        }
    }

    /// Test: Embedding bounds are enforced
    #[test]
    fn test_embedding_bounds_validation() {
        let config = EmbeddingValidationConfig::default();

        // Test NaN rejection
        let mut nan_embedding = vec![0.0f32; 1536];
        nan_embedding[100] = f32::NAN;
        assert!(validate_embedding(&nan_embedding, &config).is_err());

        // Test infinity rejection
        let mut inf_embedding = vec![0.0f32; 1536];
        inf_embedding[500] = f32::INFINITY;
        assert!(validate_embedding(&inf_embedding, &config).is_err());

        // Test dimension mismatch
        let wrong_dim = vec![0.0f32; 512];
        assert!(validate_embedding(&wrong_dim, &config).is_err());

        // Test extreme values
        let mut extreme_embedding = vec![0.0f32; 1536];
        extreme_embedding[0] = 1000.0;  // Way above max
        assert!(validate_embedding(&extreme_embedding, &config).is_err());
    }

    /// Test: Audio validation rejects malformed files
    #[test]
    fn test_audio_validation() {
        let config = AudioValidationConfig::default();

        // Test: Reject files exceeding size limit
        // Test: Reject non-audio files disguised as audio
        // Test: Reject wrong sample rate
        // Test: Reject stereo files (require mono)
        // Test: Detect embedded executables
    }

    /// Test: Rate limiting prevents abuse
    #[test]
    fn test_rate_limiting() {
        let config = RateLimiterConfig::default();
        let limiter = RateLimiter::new(config);

        // Exhaust rate limit
        for _ in 0..1000 {
            let _ = limiter.check("user1", EndpointCategory::Search);
        }

        // Next request should be limited
        let result = limiter.check("user1", EndpointCategory::Search);
        assert!(result.is_err());
    }

    /// Test: Classification access control enforced
    #[test]
    fn test_classification_access() {
        let protected_classification = DataClassification {
            level: ClassificationLevel::Protected,
            access_requirements: AccessRequirements {
                min_role: UserRole::Researcher,
                required_permissions: vec![Permission::ProtectedSpeciesRead],
                requires_dua: true,
                ..Default::default()
            },
            ..Default::default()
        };

        // Public user should be denied
        let public_context = AccessContext {
            requester_region: "US".into(),
            ..Default::default()
        };
        assert!(check_access(
            &protected_classification,
            &UserRole::Public,
            &[Permission::AudioRead],
            &public_context
        ).is_err());

        // Researcher with correct permissions should be allowed
        assert!(check_access(
            &protected_classification,
            &UserRole::Researcher,
            &[Permission::ProtectedSpeciesRead],
            &public_context
        ).is_ok());
    }

    /// Test: Audit log chain integrity
    #[test]
    fn test_audit_chain_integrity() {
        let entry1 = AuditEntry::new(
            AuditEventType::DataAccess,
            Actor { actor_type: ActorType::User, id: "user1".into(), ..Default::default() },
            Resource { resource_type: ResourceType::AudioRecording, id: "rec1".into(), ..Default::default() },
            Action { action_type: ActionType::Read, details: "Query".into(), ..Default::default() },
            Outcome { success: true, ..Default::default() },
            AuditContext::default(),
            "genesis".into(),
        );

        let entry2 = AuditEntry::new(
            AuditEventType::DataAccess,
            Actor { actor_type: ActorType::User, id: "user2".into(), ..Default::default() },
            Resource { resource_type: ResourceType::AudioRecording, id: "rec2".into(), ..Default::default() },
            Action { action_type: ActionType::Read, details: "Query".into(), ..Default::default() },
            Outcome { success: true, ..Default::default() },
            AuditContext::default(),
            entry1.entry_hash.clone(),
        );

        assert!(entry2.verify_chain(&entry1));

        // Tampering should break chain
        let mut tampered = entry1.clone();
        tampered.actor.id = "attacker".into();
        assert!(!entry2.verify_chain(&tampered));
    }

    /// Test: ONNX model integrity verification
    #[test]
    fn test_model_integrity() {
        let config = ONNXSecurityConfig {
            expected_model_hash: "known_good_hash_here".into(),
            ..Default::default()
        };

        // Loading model with wrong hash should fail
        // let result = SecureONNXRuntime::load(Path::new("tampered_model.onnx"), config);
        // assert!(matches!(result, Err(ModelSecurityError::IntegrityViolation { .. })));
    }
}
```

## Consequences

### Positive

1. **Regulatory Compliance**: Classification system enables ESA/CITES compliance
2. **Research Integrity**: RAB provenance tracking supports scientific reproducibility
3. **Defense in Depth**: Multiple security layers prevent single-point failures
4. **Memory Safety**: Rust eliminates buffer overflows, use-after-free, data races
5. **Auditability**: Hash-chained logs provide tamper-evident audit trail
6. **Performance**: Security checks are designed for minimal latency impact

### Negative

1. **Development Overhead**: Security validation adds code complexity
2. **Operational Burden**: Classification management requires ongoing curation
3. **Access Friction**: Researchers may face additional hurdles for protected data
4. **Storage Overhead**: Audit logs and provenance data increase storage requirements

### Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Security configuration drift | Medium | High | Automated security policy enforcement, regular audits |
| Classification errors | Medium | High | Human review workflow, conservative default classification |
| Key compromise | Low | Critical | Key rotation, HSM for production keys, breach response plan |
| Insider threat | Low | High | Principle of least privilege, comprehensive audit logging |

## References

- [OWASP Top 10 2021](https://owasp.org/Top10/)
- [NIST Cybersecurity Framework](https://www.nist.gov/cyberframework)
- [Endangered Species Act Data Requirements](https://www.fws.gov/endangered/)
- [Perch 2.0 Model Documentation](https://arxiv.org/abs/2508.04665)
- [RuVector Security Architecture](https://github.com/ruvnet/ruvector)
- [Argon2 Password Hashing](https://www.password-hashing.net/)
- [ONNX Runtime Security Best Practices](https://onnxruntime.ai/)

## Appendix A: Security Checklist

### Pre-Deployment

- [ ] All dependencies audited and pinned
- [ ] ONNX model hash verified and documented
- [ ] Encryption keys generated and stored in vault
- [ ] Rate limiting configured for production load
- [ ] Audit logging enabled and tested
- [ ] Classification policies defined for all data types
- [ ] Access control policies reviewed by stakeholders
- [ ] Penetration testing completed

### Operational

- [ ] Security monitoring dashboards configured
- [ ] Alert thresholds set for anomalous access patterns
- [ ] Incident response runbook documented
- [ ] Key rotation schedule established
- [ ] Audit log retention policy configured
- [ ] Backup encryption verified

### Compliance

- [ ] Data classification inventory complete
- [ ] Regulatory framework mapping documented
- [ ] Data use agreements templated and reviewed
- [ ] Privacy impact assessment completed
- [ ] Security training materials prepared
