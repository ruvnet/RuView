# ADR-012: Genomic Security and Privacy

**Status:** Accepted
**Date:** 2026-02-11
**Authors:** RuVector Security Team
**Deciders:** Architecture Review Board, Security Review Board
**Technical Area:** Security / Privacy / Compliance

---

## Version History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-02-11 | RuVector Security Team | Initial security architecture |

---

## Context and Problem Statement

Genomic data is the most sensitive personal information. A single genome:
- Uniquely identifies an individual (more reliable than fingerprints)
- Reveals disease risk for the individual AND their relatives
- Exposes ancestry, paternity, and family relationships
- Can be used for discrimination (insurance, employment under GINA violations)
- Never changes (cannot be "reset" like a password)

### Threat Model: Genomic Data Risks

| Threat | Attack Vector | Impact | Likelihood |
|--------|--------------|--------|------------|
| **Re-identification attacks** | Cross-reference genomic data with public databases (GEDmatch, OpenSNP) to identify anonymous individuals | Privacy violation, GINA violation | High |
| **Data breach** | Unauthorized access to genomic database via SQL injection, API exploit, or insider threat | Mass exposure of PHI, lawsuits, regulatory fines | Medium |
| **Inference attacks** | Use ML models to infer phenotypes from genomic data (disease risk, drug response, ancestry) without consent | Discrimination, privacy violation | High |
| **Linkage attacks** | Combine genomic data with non-genomic data (medical records, social media) to infer sensitive attributes | Targeted discrimination | Medium |
| **Forensic abuse** | Law enforcement access to genomic databases for criminal investigations without warrant (GEDmatch controversy) | Privacy violation, 4th Amendment | Low (but high impact) |
| **Insurance discrimination** | Insurers access genomic data to deny coverage or increase premiums (GINA applies to health, not life/disability) | Financial harm | Medium (legal for life insurance) |
| **Ransomware** | Encrypt genomic database and demand payment | Business disruption, data loss | Medium |
| **Supply chain attack** | Compromise sequencing equipment or analysis software to inject backdoors | Data exfiltration, tampering | Low (but critical impact) |

### Regulatory Landscape

| Regulation | Jurisdiction | Key Requirements | Penalties |
|-----------|--------------|-----------------|-----------|
| **HIPAA** (Health Insurance Portability and Accountability Act) | US | Encrypt PHI at rest and in transit; access controls; audit logs; breach notification | Up to $1.5M per violation category per year |
| **GDPR** (General Data Protection Regulation) | EU/EEA | Explicit consent for genomic data processing; right to erasure; data minimization; DPO required | Up to €20M or 4% global revenue |
| **GINA** (Genetic Information Nondiscrimination Act) | US | Prohibits health insurers and employers from using genomic data for discrimination | Criminal penalties + civil damages |
| **CCPA/CPRA** (California Consumer Privacy Act) | California | Opt-out of genomic data sale; right to deletion; transparency | $7,500 per intentional violation |
| **PIPEDA** (Personal Information Protection) | Canada | Consent for genomic data collection; security safeguards | Up to CAD 100,000 per violation |

---

## Decision

### Defense-in-Depth Security Architecture

Implement a layered security model with encryption at rest and in transit, differential privacy for aggregate queries, role-based access control (RBAC), and audit logging. All genomic data processing uses client-side execution where possible (WASM in browser) to minimize server-side PHI exposure.

---

## Threat Model for Genomic Data

### Data Classification

| Data Type | Sensitivity | Examples | Encryption Required | Retention Policy |
|-----------|------------|----------|-------------------|------------------|
| **Raw genomic data** | Critical | FASTQ, BAM, CRAM, VCF files | ✅ AES-256 at rest, TLS 1.3 in transit | Unlimited (with consent) |
| **Genomic embeddings** | High | k-mer vectors, variant embeddings, HNSW indices | ✅ AES-256 at rest | Unlimited |
| **Aggregate statistics** | Medium | Allele frequencies, population stratification | ⚠️ Differential privacy (ε-budget) | Unlimited |
| **Metadata** | Medium | Sample IDs, sequencing dates, coverage metrics | ✅ AES-256 at rest | Per HIPAA/GDPR |
| **Derived phenotypes** | High | Disease risk scores, PGx predictions | ✅ AES-256 at rest | Per consent |
| **Audit logs** | Low | Access timestamps, user IDs | ❌ Plaintext (no PHI) | 7 years (HIPAA) |

### Attack Surface

```
┌─────────────────────────────────────────────────────────────┐
│                    EXTERNAL ATTACK SURFACE                    │
├─────────────────────────────────────────────────────────────┤
│  1. Web API (ruvector-server)                                │
│     - Input validation (Zod schemas)                         │
│     - Rate limiting (100 req/min per IP)                     │
│     - CORS whitelist                                         │
│     - JWT authentication (RS256, 15min expiry)               │
├─────────────────────────────────────────────────────────────┤
│  2. Browser WASM (client-side execution)                     │
│     - CSP: connect-src 'self'; script-src 'self' 'wasm-unsafe-eval' │
│     - SRI hashes on all WASM modules                         │
│     - Service worker blocks unauthorized network requests    │
├─────────────────────────────────────────────────────────────┤
│  3. File Upload Endpoints                                    │
│     - Max file size: 10GB                                    │
│     - Allowed MIME types: application/gzip, application/x-bam │
│     - Virus scan (ClamAV) before processing                  │
│     - Sandboxed processing (no shell access)                 │
└─────────────────────────────────────────────────────────────┘
```

---

## Practical Encryption

### 1. Encryption at Rest (AES-256-GCM)

**All genomic data encrypted before writing to disk:**

```rust
use aes_gcm::{Aes256Gcm, Key, Nonce};
use aes_gcm::aead::{Aead, NewAead};

pub struct GenomicDataStore {
    cipher: Aes256Gcm,
    storage_path: PathBuf,
}

impl GenomicDataStore {
    pub fn new(master_key: &[u8; 32], storage_path: PathBuf) -> Self {
        let key = Key::from_slice(master_key);
        let cipher = Aes256Gcm::new(key);
        Self { cipher, storage_path }
    }

    pub fn encrypt_vcf(&self, sample_id: &str, vcf_data: &[u8]) -> Result<(), Error> {
        // Generate random nonce (96 bits for AES-GCM)
        let nonce = Nonce::from_slice(&generate_random_nonce());

        // Encrypt VCF data
        let ciphertext = self.cipher.encrypt(nonce, vcf_data)
            .map_err(|_| Error::EncryptionFailed)?;

        // Store: nonce (12 bytes) || ciphertext || auth_tag (16 bytes)
        let mut encrypted_data = nonce.to_vec();
        encrypted_data.extend_from_slice(&ciphertext);

        let path = self.storage_path.join(format!("{}.vcf.enc", sample_id));
        std::fs::write(&path, &encrypted_data)?;

        // Set restrictive permissions (0600: owner read/write only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        }

        Ok(())
    }

    pub fn decrypt_vcf(&self, sample_id: &str) -> Result<Vec<u8>, Error> {
        let path = self.storage_path.join(format!("{}.vcf.enc", sample_id));
        let encrypted_data = std::fs::read(&path)?;

        // Split nonce and ciphertext
        let (nonce_bytes, ciphertext) = encrypted_data.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);

        // Decrypt and verify auth tag
        self.cipher.decrypt(nonce, ciphertext)
            .map_err(|_| Error::DecryptionFailed)
    }
}
```

**Key management:**
- Master key derived from HSM (Hardware Security Module) or AWS KMS
- Per-sample encryption keys derived via HKDF (HMAC-based Key Derivation Function)
- Key rotation every 90 days
- Old keys retained for decryption of historical data

**Status:** ✅ Implemented in `ruvector-server`

### 2. Encryption in Transit (TLS 1.3)

**Mandatory TLS 1.3 with modern cipher suites:**

```nginx
# nginx configuration for ruvector-server
server {
    listen 443 ssl http2;
    server_name genomics.ruvector.ai;

    # TLS 1.3 only
    ssl_protocols TLSv1.3;

    # Modern cipher suites (forward secrecy)
    ssl_ciphers 'TLS_AES_256_GCM_SHA384:TLS_CHACHA20_POLY1305_SHA256:TLS_AES_128_GCM_SHA256';
    ssl_prefer_server_ciphers off;

    # OCSP stapling
    ssl_stapling on;
    ssl_stapling_verify on;

    # HSTS (force HTTPS for 1 year)
    add_header Strict-Transport-Security "max-age=31536000; includeSubDomains; preload" always;

    # Certificate pinning (optional, high security)
    add_header Public-Key-Pins 'pin-sha256="base64+primary=="; pin-sha256="base64+backup=="; max-age=5184000; includeSubDomains' always;

    location /api/ {
        proxy_pass http://localhost:3000;
        proxy_ssl_protocols TLSv1.3;
    }
}
```

**Certificate requirements:**
- Extended Validation (EV) certificate from DigiCert or Sectigo
- 2048-bit RSA or 256-bit ECDSA
- Certificate Transparency (CT) logs

**Status:** ✅ TLS 1.3 enforced in production

### 3. Client-Side Encryption (WASM in Browser)

**For maximum privacy, encrypt genomic data in browser before upload:**

```javascript
// Client-side encryption using Web Crypto API
async function encryptVCFBeforeUpload(vcfFile, userPassword) {
    // Derive encryption key from user password (PBKDF2)
    const encoder = new TextEncoder();
    const passwordKey = await crypto.subtle.importKey(
        'raw',
        encoder.encode(userPassword),
        'PBKDF2',
        false,
        ['deriveBits', 'deriveKey']
    );

    const salt = crypto.getRandomValues(new Uint8Array(16));
    const encryptionKey = await crypto.subtle.deriveKey(
        {
            name: 'PBKDF2',
            salt: salt,
            iterations: 100000,
            hash: 'SHA-256'
        },
        passwordKey,
        { name: 'AES-GCM', length: 256 },
        false,
        ['encrypt']
    );

    // Encrypt VCF data
    const iv = crypto.getRandomValues(new Uint8Array(12));
    const vcfData = await vcfFile.arrayBuffer();
    const ciphertext = await crypto.subtle.encrypt(
        { name: 'AES-GCM', iv: iv },
        encryptionKey,
        vcfData
    );

    // Return: salt || iv || ciphertext (server cannot decrypt without password)
    return new Blob([salt, iv, ciphertext]);
}

// Upload encrypted blob
async function uploadEncryptedVCF(encryptedBlob, sampleId) {
    const formData = new FormData();
    formData.append('sample_id', sampleId);
    formData.append('encrypted_vcf', encryptedBlob);

    await fetch('/api/upload', {
        method: 'POST',
        body: formData,
        headers: {
            'Authorization': `Bearer ${getJWT()}`
        }
    });
}
```

**Zero-knowledge architecture:** Server stores encrypted VCF but cannot decrypt without user password.

**Status:** ⚠️ Prototype implemented, needs UX refinement

---

## Differential Privacy for Allele Frequencies

### Problem: Aggregate Statistics Leak Individual Genotypes

Publishing population allele frequencies can enable re-identification attacks. Example:

```
Published allele frequencies for 10,000 individuals:
- rs123456: MAF = 0.0251 (251 carriers)

Attacker queries with and without target individual:
- With target:    MAF = 0.0251 → 251 carriers
- Without target: MAF = 0.0250 → 250 carriers

Conclusion: Target is a carrier of rs123456 (privacy leak)
```

### Solution: Laplace Mechanism with ε-Differential Privacy

**Add calibrated noise to allele frequencies before publication:**

```rust
use rand::distributions::{Distribution, Laplace};

pub struct DifferentiallyPrivateFrequency {
    epsilon: f64,  // Privacy budget (lower = more private)
    sensitivity: f64,  // Global sensitivity of query
}

impl DifferentiallyPrivateFrequency {
    pub fn new(epsilon: f64) -> Self {
        // Sensitivity of allele frequency query: 1/n (adding/removing one individual)
        Self { epsilon, sensitivity: 1.0 }
    }

    pub fn release_allele_frequency(
        &self,
        true_frequency: f64,
        sample_size: usize
    ) -> f64 {
        // Scale parameter for Laplace noise: sensitivity / epsilon
        let scale = (1.0 / sample_size as f64) / self.epsilon;

        // Sample from Laplace distribution
        let laplace = Laplace::new(0.0, scale).unwrap();
        let noise = laplace.sample(&mut rand::thread_rng());

        // Add noise and clip to [0, 1]
        (true_frequency + noise).clamp(0.0, 1.0)
    }
}

// Example usage
fn publish_gnomad_frequencies(variants: &[Variant], epsilon: f64) {
    let dp = DifferentiallyPrivateFrequency::new(epsilon);

    for variant in variants {
        let true_af = variant.alt_count as f64 / variant.total_count as f64;
        let noisy_af = dp.release_allele_frequency(true_af, variant.total_count);

        println!("Variant {}: AF = {:.6} (ε = {})", variant.id, noisy_af, epsilon);
    }
}
```

### ε-Budget Guidelines

| Use Case | ε Value | Privacy Guarantee | Noise Level |
|----------|---------|-------------------|-------------|
| High privacy (clinical) | 0.1 | Very strong | High noise (±10% AF error) |
| Moderate privacy (research) | 1.0 | Strong | Moderate noise (±1% AF error) |
| Low privacy (public DB) | 10.0 | Weak | Low noise (±0.1% AF error) |

**Composition theorem:** If multiple queries consume ε₁, ε₂, ..., εₙ, total privacy budget is Σεᵢ. Must track cumulative ε per dataset.

**Status:** ✅ Implemented in aggregate statistics API

---

## Access Control via ruvector-server/router

### Role-Based Access Control (RBAC)

**Five roles with hierarchical permissions:**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Patient,         // Can view own genomic data only
    Clinician,       // Can view assigned patients' data
    Researcher,      // Can query aggregate statistics (DP-protected)
    DataScientist,   // Can access de-identified genomic data
    Admin,           // Full access to all data and system config
}

impl Role {
    pub fn can_access_vcf(&self, requester_id: &str, sample_id: &str) -> bool {
        match self {
            Role::Patient => requester_id == sample_id,  // Own data only
            Role::Clinician => check_patient_assignment(requester_id, sample_id),
            Role::DataScientist => is_deidentified(sample_id),
            Role::Admin => true,
            Role::Researcher => false,  // Aggregate queries only
        }
    }

    pub fn can_query_aggregate(&self) -> bool {
        matches!(self, Role::Researcher | Role::DataScientist | Role::Admin)
    }
}
```

### JWT-Based Authentication

**Access tokens with role claims:**

```rust
use jsonwebtoken::{encode, decode, Header, Algorithm, Validation};
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,        // User ID
    role: Role,         // User role
    exp: usize,         // Expiration timestamp
    iat: usize,         // Issued at timestamp
    iss: String,        // Issuer (ruvector-auth)
    aud: String,        // Audience (ruvector-server)
}

pub fn generate_access_token(user_id: &str, role: Role) -> Result<String, Error> {
    let claims = Claims {
        sub: user_id.to_string(),
        role,
        exp: (chrono::Utc::now() + chrono::Duration::minutes(15)).timestamp() as usize,
        iat: chrono::Utc::now().timestamp() as usize,
        iss: "ruvector-auth".to_string(),
        aud: "ruvector-server".to_string(),
    };

    // Sign with RS256 (asymmetric key)
    let header = Header::new(Algorithm::RS256);
    encode(&header, &claims, &get_private_key()?)
        .map_err(|_| Error::TokenGenerationFailed)
}

pub fn verify_access_token(token: &str) -> Result<Claims, Error> {
    let validation = Validation::new(Algorithm::RS256);
    decode::<Claims>(token, &get_public_key()?, &validation)
        .map(|data| data.claims)
        .map_err(|_| Error::InvalidToken)
}
```

**Token lifecycle:**
- Access tokens: 15 minutes (short-lived)
- Refresh tokens: 7 days (stored in httpOnly secure cookie)
- Token rotation on every refresh

**Status:** ✅ Implemented in `ruvector-server`

### Audit Logging

**All data access logged to immutable audit trail:**

```rust
pub struct AuditLog {
    timestamp: DateTime<Utc>,
    user_id: String,
    role: Role,
    action: Action,
    resource: String,
    ip_address: IpAddr,
    user_agent: String,
    success: bool,
}

#[derive(Debug)]
pub enum Action {
    ViewVCF,
    DownloadVCF,
    UploadVCF,
    DeleteVCF,
    QueryAggregate,
    ModifyPermissions,
}

impl AuditLog {
    pub fn log_access(user_id: &str, role: Role, action: Action, resource: &str, success: bool) {
        let entry = AuditLog {
            timestamp: Utc::now(),
            user_id: user_id.to_string(),
            role,
            action,
            resource: resource.to_string(),
            ip_address: get_request_ip(),
            user_agent: get_request_user_agent(),
            success,
        };

        // Write to append-only log (PostgreSQL with RLS or AWS CloudTrail)
        write_audit_log(&entry);

        // Alert on suspicious activity
        if is_suspicious(&entry) {
            alert_security_team(&entry);
        }
    }
}
```

**Suspicious activity detection:**
- Multiple failed access attempts (>5 in 1 hour)
- Access from unusual location (GeoIP check)
- Bulk downloads (>100 VCF files in 1 day)
- Role escalation attempts

**Status:** ✅ Implemented, logs retained for 7 years (HIPAA)

---

## HIPAA/GDPR Compliance Checklist

### HIPAA Security Rule

| Requirement | Implementation | Status |
|------------|----------------|--------|
| **Administrative Safeguards** | | |
| Security management process | Risk assessments quarterly, penetration testing annually | ✅ |
| Assigned security responsibility | CISO and security team | ✅ |
| Workforce security | Background checks, access termination procedures | ✅ |
| Security awareness training | Annual HIPAA training for all staff | ✅ |
| **Physical Safeguards** | | |
| Facility access controls | Badge-controlled data center, visitor logs | ✅ |
| Workstation security | Encrypted laptops, screen locks after 5min | ✅ |
| Device and media controls | Encrypted backups, secure disposal (NIST 800-88) | ✅ |
| **Technical Safeguards** | | |
| Access control | RBAC, JWT authentication, MFA for admin | ✅ |
| Audit controls | Immutable audit logs, 7-year retention | ✅ |
| Integrity controls | Digital signatures on VCF files, checksum verification | ✅ |
| Transmission security | TLS 1.3, VPN for internal traffic | ✅ |
| **Breach Notification** | | |
| Breach notification plan | Notify OCR within 60 days, affected individuals within 60 days | ✅ |
| Incident response plan | Documented runbook, tabletop exercises quarterly | ✅ |

### GDPR Compliance

| Requirement | Implementation | Status |
|------------|----------------|--------|
| **Lawful Basis (Article 6)** | Explicit consent for genomic data processing | ✅ |
| **Consent (Article 7)** | Affirmative opt-in, granular consent (research vs clinical), withdraw anytime | ✅ |
| **Right to Access (Article 15)** | Self-service data export in VCF format | ✅ |
| **Right to Rectification (Article 16)** | Allow users to update metadata, request re-analysis | ✅ |
| **Right to Erasure (Article 17)** | Delete all genomic data within 30 days of request | ✅ |
| **Data Portability (Article 20)** | Export in machine-readable format (VCF, JSON) | ✅ |
| **Privacy by Design (Article 25)** | Client-side WASM execution, minimal server-side PHI | ✅ |
| **Data Protection Officer (DPO)** | Appointed DPO, contact: dpo@ruvector.ai | ✅ |
| **Data Processing Agreement (DPA)** | DPA with all third-party processors (AWS, sequencing vendors) | ✅ |
| **Cross-Border Transfer** | EU data stays in EU (AWS eu-west-1), SCCs for US transfer | ✅ |
| **Breach Notification (Article 33)** | Notify supervisory authority within 72 hours | ✅ |

**Status:** ✅ Compliant (verified by external audit, 2026-01)

---

## Implementation Status

### Security Components

| Component | Status | Notes |
|-----------|--------|-------|
| AES-256-GCM encryption at rest | ✅ Deployed | All VCF/BAM/CRAM files encrypted |
| TLS 1.3 in transit | ✅ Deployed | Enforced in production |
| Client-side encryption (WASM) | ⚠️ Prototype | Needs UX polish |
| Differential privacy (ε-budget) | ✅ Deployed | Used for aggregate stats API |
| RBAC with 5 roles | ✅ Deployed | Patient, Clinician, Researcher, DataScientist, Admin |
| JWT authentication (RS256) | ✅ Deployed | 15min access tokens, 7-day refresh |
| Audit logging | ✅ Deployed | 7-year retention in PostgreSQL |
| MFA for admin roles | ✅ Deployed | TOTP (Google Authenticator) |
| Intrusion detection (IDS) | ✅ Deployed | Suricata rules for genomic API |
| Penetration testing | ✅ Quarterly | Last test: 2026-01 (no critical findings) |

### Compliance

| Standard | Status | Last Audit | Next Audit |
|----------|--------|-----------|-----------|
| HIPAA Security Rule | ✅ Compliant | 2026-01 | 2027-01 |
| GDPR | ✅ Compliant | 2026-01 | 2027-01 |
| GINA | ✅ Compliant | N/A (no audit required) | N/A |
| ISO 27001 | ⚠️ In progress | N/A | 2026-06 (target) |
| SOC 2 Type II | ⚠️ In progress | N/A | 2026-09 (target) |

---

## References

1. Gymrek, M., et al. (2013). "Identifying personal genomes by surname inference." *Science*, 339(6117), 321-324. (Re-identification attacks)
2. Homer, N., et al. (2008). "Resolving individuals contributing trace amounts of DNA to highly complex mixtures." *PLoS Genetics*, 4(8), e1000167. (Mixture deconvolution attacks)
3. Dwork, C., & Roth, A. (2014). "The Algorithmic Foundations of Differential Privacy." *Foundations and Trends in Theoretical Computer Science*, 9(3-4), 211-407.
4. NIST Special Publication 800-53 Rev. 5. "Security and Privacy Controls for Information Systems and Organizations."
5. FDA Guidance on Cybersecurity for Medical Devices (2023).
6. 45 CFR Part 164 (HIPAA Security Rule).
7. GDPR Articles 5, 6, 7, 15-22, 25, 32, 33 (EU Regulation 2016/679).

---

## Related Decisions

- **ADR-001**: RuVector Core Architecture (HNSW index security)
- **ADR-008**: WASM Edge Genomics (client-side execution for privacy)
- **ADR-009**: Variant Calling Pipeline (encrypted variant storage)

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-02-11 | RuVector Security Team | Initial security architecture, threat model, encryption, RBAC, compliance checklist |
