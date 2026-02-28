# ADR-007: Post-Quantum Cryptography for Secure Sensing

## Status
Proposed

## Date
2026-02-28

## Context

### Threat Model

WiFi-DensePose processes data that can reveal:
- **Human presence/absence** in private spaces (surveillance risk)
- **Health indicators** via breathing/heartbeat detection (medical privacy)
- **Movement patterns** (behavioral profiling)
- **Building occupancy** (physical security intelligence)

In disaster scenarios (wifi-densepose-mat), the stakes are even higher:
- **Triage classifications** affect rescue priority (life-or-death decisions)
- **Survivor locations** are operationally sensitive
- **Detection audit trails** may be used in legal proceedings (liability)
- **False negatives** (missed survivors) could be forensically investigated

Current security: The system uses standard JWT (HS256) for API authentication and has no cryptographic protection on data at rest, model integrity, or detection audit trails.

### Quantum Threat Timeline

NIST estimates cryptographically relevant quantum computers could emerge by 2030-2035. Data captured today with classical encryption may be decrypted retroactively ("harvest now, decrypt later"). For a system that may be deployed for decades in infrastructure, post-quantum readiness is prudent.

### RuVector's Crypto Stack

RuVector provides a layered cryptographic system:

| Algorithm | Purpose | Standard | Quantum Resistant |
|-----------|---------|----------|-------------------|
| ML-DSA-65 | Digital signatures | FIPS 204 | Yes (lattice-based) |
| Ed25519 | Digital signatures | RFC 8032 | No (classical fallback) |
| SLH-DSA-128s | Digital signatures | FIPS 205 | Yes (hash-based) |
| SHAKE-256 | Hashing | FIPS 202 | Yes |
| AES-256-GCM | Symmetric encryption | FIPS 197 | Yes (Grover's halves, still 128-bit) |

## Decision

We will integrate RuVector's cryptographic layer to provide defense-in-depth for WiFi-DensePose data, using a **hybrid classical+PQ** approach where both Ed25519 and ML-DSA-65 signatures are applied (belt-and-suspenders until PQ algorithms mature).

### Cryptographic Scope

```
┌──────────────────────────────────────────────────────────────────┐
│              Cryptographic Protection Layers                      │
├──────────────────────────────────────────────────────────────────┤
│                                                                   │
│  1. MODEL INTEGRITY                                              │
│     ┌─────────────────────────────────────────────────────┐      │
│     │ Model weights signed with ML-DSA-65 + Ed25519       │      │
│     │ Signature verified at load time → reject tampered   │      │
│     │ SONA adaptations co-signed with device key          │      │
│     └─────────────────────────────────────────────────────┘      │
│                                                                   │
│  2. DATA AT REST (RVF containers)                                │
│     ┌─────────────────────────────────────────────────────┐      │
│     │ CSI vectors encrypted with AES-256-GCM              │      │
│     │ Container integrity via SHAKE-256 Merkle tree       │      │
│     │ Key management: per-container keys, sealed to device │      │
│     └─────────────────────────────────────────────────────┘      │
│                                                                   │
│  3. DATA IN TRANSIT                                              │
│     ┌─────────────────────────────────────────────────────┐      │
│     │ API: TLS 1.3 with PQ key exchange (ML-KEM-768)      │      │
│     │ WebSocket: Same TLS channel                         │      │
│     │ Multi-AP sync: mTLS with device certificates        │      │
│     └─────────────────────────────────────────────────────┘      │
│                                                                   │
│  4. AUDIT TRAIL (witness chains - see ADR-010)                   │
│     ┌─────────────────────────────────────────────────────┐      │
│     │ Every detection event hash-chained with SHAKE-256   │      │
│     │ Chain anchors signed with ML-DSA-65                 │      │
│     │ Cross-device attestation via SLH-DSA-128s           │      │
│     └─────────────────────────────────────────────────────┘      │
│                                                                   │
│  5. DEVICE IDENTITY                                              │
│     ┌─────────────────────────────────────────────────────┐      │
│     │ Each sensing device has a key pair (ML-DSA-65)      │      │
│     │ Device attestation proves hardware integrity        │      │
│     │ Key rotation schedule: 90 days (or on compromise)   │      │
│     └─────────────────────────────────────────────────────┘      │
└──────────────────────────────────────────────────────────────────┘
```

### Hybrid Signature Scheme

```rust
/// Hybrid signature combining classical Ed25519 with PQ ML-DSA-65
pub struct HybridSignature {
    /// Classical Ed25519 signature (64 bytes)
    ed25519_sig: [u8; 64],

    /// Post-quantum ML-DSA-65 signature (3309 bytes)
    ml_dsa_sig: Vec<u8>,

    /// Signer's public key fingerprint (SHAKE-256, 32 bytes)
    signer_fingerprint: [u8; 32],

    /// Timestamp of signing
    timestamp: u64,
}

impl HybridSignature {
    /// Verify requires BOTH signatures to be valid
    pub fn verify(&self, message: &[u8], ed25519_pk: &Ed25519PublicKey,
                  ml_dsa_pk: &MlDsaPublicKey) -> Result<bool, CryptoError> {
        let ed25519_valid = ed25519_pk.verify(message, &self.ed25519_sig)?;
        let ml_dsa_valid = ml_dsa_pk.verify(message, &self.ml_dsa_sig)?;

        // Both must pass (defense in depth)
        Ok(ed25519_valid && ml_dsa_valid)
    }
}
```

### Model Integrity Verification

```rust
/// Verify model weights have not been tampered with
pub fn verify_model_integrity(model_container: &ModelContainer) -> Result<(), SecurityError> {
    // 1. Extract embedded signature from container
    let signature = model_container.crypto_segment().signature()?;

    // 2. Compute SHAKE-256 hash of weight data
    let weight_hash = shake256(model_container.weights_segment().data());

    // 3. Verify hybrid signature
    let publisher_keys = load_publisher_keys()?;
    if !signature.verify(&weight_hash, &publisher_keys.ed25519, &publisher_keys.ml_dsa)? {
        return Err(SecurityError::ModelTampered {
            expected_signer: publisher_keys.fingerprint(),
            container_path: model_container.path().to_owned(),
        });
    }

    Ok(())
}
```

### CSI Data Encryption

For privacy-sensitive deployments, CSI vectors can be encrypted at rest:

```rust
/// Encrypt CSI vectors for storage in RVF container
pub struct CsiEncryptor {
    /// AES-256-GCM key (derived from device key + container salt)
    key: Aes256GcmKey,
}

impl CsiEncryptor {
    /// Encrypt a CSI feature vector
    /// Note: HNSW search operates on encrypted vectors using
    /// distance-preserving encryption (approximate, configurable trade-off)
    pub fn encrypt_vector(&self, vector: &[f32]) -> EncryptedVector {
        let nonce = generate_nonce();
        let plaintext = bytemuck::cast_slice::<f32, u8>(vector);
        let ciphertext = aes_256_gcm_encrypt(&self.key, &nonce, plaintext);
        EncryptedVector { ciphertext, nonce }
    }
}
```

### Performance Impact

| Operation | Without Crypto | With Crypto | Overhead |
|-----------|---------------|-------------|----------|
| Model load | 50 ms | 52 ms | +2 ms (signature verify) |
| Vector insert | 0.1 ms | 0.15 ms | +0.05 ms (encrypt) |
| HNSW search | 0.3 ms | 0.35 ms | +0.05 ms (decrypt top-K) |
| Container open | 10 ms | 12 ms | +2 ms (integrity check) |
| Detection event logging | 0.01 ms | 0.5 ms | +0.49 ms (hash chain) |

### Feature Flags

```toml
[features]
default = []
crypto-classical = ["ed25519-dalek"]  # Ed25519 only
crypto-pq = ["pqcrypto-dilithium", "pqcrypto-sphincsplus"]  # ML-DSA + SLH-DSA
crypto-hybrid = ["crypto-classical", "crypto-pq"]  # Both (recommended)
crypto-encrypt = ["aes-gcm"]  # Data-at-rest encryption
crypto-full = ["crypto-hybrid", "crypto-encrypt"]
```

## Consequences

### Positive
- **Future-proof**: Lattice-based signatures resist quantum attacks
- **Tamper detection**: Model poisoning and data manipulation are detectable
- **Privacy compliance**: Encrypted CSI data meets GDPR/HIPAA requirements
- **Forensic integrity**: Signed audit trails are admissible as evidence
- **Low overhead**: <1ms per operation for most crypto operations

### Negative
- **Signature size**: ML-DSA-65 signatures are 3.3 KB vs 64 bytes for Ed25519
- **Key management complexity**: Device key provisioning, rotation, revocation
- **HNSW on encrypted data**: Distance-preserving encryption is approximate; search recall may degrade
- **Dependency weight**: PQ crypto libraries add ~2 MB to binary
- **Standards maturity**: FIPS 204/205 are finalized but implementations are evolving

## References

- [FIPS 204: ML-DSA (Module-Lattice Digital Signature)](https://csrc.nist.gov/pubs/fips/204/final)
- [FIPS 205: SLH-DSA (Stateless Hash-Based Digital Signature)](https://csrc.nist.gov/pubs/fips/205/final)
- [FIPS 202: SHA-3 / SHAKE](https://csrc.nist.gov/pubs/fips/202/final)
- [RuVector Crypto Implementation](https://github.com/ruvnet/ruvector)
- ADR-002: RuVector RVF Integration Strategy
- ADR-010: Witness Chains for Audit Trail Integrity
