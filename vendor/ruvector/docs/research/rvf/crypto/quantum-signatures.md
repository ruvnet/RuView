# RVF Quantum-Resistant Cryptography

## 1. Threat Model

RVF files may contain high-value intelligence (medical genomics, proprietary
embeddings, classified networks). The cryptographic design must:

1. **Authenticate**: Prove a segment was written by an authorized producer
2. **Integrity**: Detect any modification to segment payloads
3. **Quantum resistance**: Survive attacks by future quantum computers
4. **Performance**: Not bottleneck streaming ingest or query paths
5. **Compactness**: Signatures must fit in segment footers without bloating

### Harvest-Now, Decrypt-Later

Adversaries may archive RVF files today and break classical signatures later
with quantum computers. Post-quantum signatures protect against this from day one.

## 2. Algorithm Selection

### NIST Post-Quantum Standards (FIPS 204, 205, 206)

| Algorithm | Standard | Type | Sig Size | PK Size | SK Size | Sign/s | Verify/s | Level |
|-----------|----------|------|----------|---------|---------|--------|----------|-------|
| ML-DSA-44 | FIPS 204 | Lattice | 2,420 B | 1,312 B | 2,560 B | ~9,000 | ~42,000 | 2 |
| ML-DSA-65 | FIPS 204 | Lattice | 3,309 B | 1,952 B | 4,032 B | ~4,500 | ~17,000 | 3 |
| ML-DSA-87 | FIPS 204 | Lattice | 4,627 B | 2,592 B | 4,896 B | ~2,800 | ~10,000 | 5 |
| SLH-DSA-128s | FIPS 205 | Hash | 7,856 B | 32 B | 64 B | ~350 | ~15,000 | 1 |
| SLH-DSA-128f | FIPS 205 | Hash | 17,088 B | 32 B | 64 B | ~3,000 | ~90,000 | 1 |
| FN-DSA-512 | FIPS 206 | Lattice | 666 B | 897 B | ~1.3 KB | ~5,000 | ~25,000 | 1 |

### RVF Default: ML-DSA-65

**Why ML-DSA-65**:
- NIST Level 3 security (128-bit post-quantum)
- 3,309 byte signatures (manageable in segment footer)
- ~4,500 sign/s (sufficient for streaming ingest at segment level)
- ~17,000 verify/s (fast enough for progressive load verification)
- Well-studied lattice assumption (Module-LWE)

**Alternative for size-constrained environments (Core Profile)**:
FN-DSA-512 with 666 byte signatures — but FIPS 206 is newer and less deployed.

**Alternative for maximum conservatism**:
SLH-DSA-128s (hash-based, stateless, minimal assumptions) — 7,856 byte
signatures but the smallest keys and strongest theoretical foundation.

## 3. Signature Scheme

### What Gets Signed

Each signed segment's signature covers:

```
signed_data = segment_header[0:40]    # Header minus content_hash and padding
            || content_hash           # The payload hash
            || segment_id_bytes       # Prevent replay
            || context_string         # Domain separation
```

The signature does NOT cover the raw payload directly — it covers the payload's
hash. This means:
- Signing is O(1) regardless of payload size
- The hash is computed during write anyway (required for integrity)
- Verification requires only the header + hash, not the full payload

### Context String

```
context = "RVF-v1-" || seg_type_name || "-" || profile_name
```

Examples:
- `"RVF-v1-VEC_SEG-rvdna"`
- `"RVF-v1-MANIFEST_SEG-generic"`

Domain separation prevents cross-type signature confusion.

### Key Management

Keys are stored in CRYPTO_SEG segments:

```
CRYPTO_SEG Payload:
  key_type: u8
    0 = signing public key
    1 = verification certificate chain
    2 = encryption public key (for ENCRYPTED segments)
    3 = key rotation record

  algorithm: u8
    0 = Ed25519 (classical)
    1 = ML-DSA-65 (post-quantum)
    2 = SLH-DSA-128s (hash-based PQ)
    3 = X25519 (classical KEM)
    4 = ML-KEM-768 (post-quantum KEM)

  key_id: [u8; 16]    Unique key identifier (hash of public key)
  key_data: [u8; var]  The actual key material
  valid_from: u64      Timestamp (ns) when key becomes valid
  valid_until: u64     Timestamp (ns) when key expires (0 = no expiry)
```

### Key Rotation

New keys are introduced by writing a new CRYPTO_SEG with `key_type=3`
(rotation record) that references both old and new key IDs. Segments
signed with either key are valid during the transition period.

```
CRYPTO_SEG (rotation):
  old_key_id: [u8; 16]
  new_key_id: [u8; 16]
  rotation_timestamp: u64
  cross_signature: [u8; var]   New key signed by old key
```

## 4. Hash Functions

### SHAKE-256 (Primary)

SHAKE-256 from the SHA-3 family is used for:
- Content hashes in segment headers (128-bit truncation for compactness)
- Min-cut witness hashes (256-bit for cryptographic binding)
- Key derivation
- Domain separation

**Why SHAKE-256**:
- Post-quantum safe (Keccak is not vulnerable to Grover's algorithm at 256-bit output)
- Extendable output function (XOF) — can produce any hash length
- No length extension attacks
- ~1 GB/s in software, faster with hardware SHA-3 extensions

### XXH3-128 (Fast Path)

XXH3 is used for non-cryptographic content hashing where speed matters more
than collision resistance:
- Segment content hashes when crypto verification is not required
- Block-level integrity checks in combination with CRC32C

**Performance**: ~50 GB/s with AVX2. This means hash computation is never
the bottleneck during streaming ingest.

### CRC32C (Block Level)

CRC32C is used for per-block integrity within segments:
- Detects random bit flips and truncation
- Hardware accelerated on x86 (SSE4.2) and ARM (CRC32 extension)
- ~3 GB/s throughput

### Hash Selection by Context

| Context | Algorithm | Output Size | Why |
|---------|-----------|------------|-----|
| Block integrity | CRC32C | 4 B | Fastest, HW accel |
| Segment content hash (fast) | XXH3-128 | 16 B | Very fast, good distribution |
| Segment content hash (crypto) | SHAKE-256 | 16 B | Post-quantum, collision resistant |
| Witness / proof hashes | SHAKE-256 | 32 B | Full crypto strength |
| Key derivation | SHAKE-256 | 32+ B | XOF flexibility |

## 5. Encryption (Optional)

For ENCRYPTED segments, RVF uses hybrid encryption:

### Key Encapsulation

```
Classical:      X25519 ECDH
Post-Quantum:   ML-KEM-768 (CRYSTALS-Kyber, NIST Level 3)
Hybrid:         X25519 || ML-KEM-768 (concatenated shared secrets)
```

### Payload Encryption

```
Algorithm:      AES-256-GCM (AEAD)
Key:            SHAKE-256(X25519_shared || ML-KEM_shared || context)
Nonce:          First 12 bytes of SHAKE-256(segment_id || timestamp)
AAD:            segment_header[0:40] (authenticated but not encrypted)
```

### Encrypted Segment Layout

```
Segment Header (64B, plaintext)
  flags: ENCRYPTED set
  content_hash: hash of PLAINTEXT payload (for integrity after decrypt)

Encapsulated Keys
  x25519_ephemeral_pk: [u8; 32]
  ml_kem_ciphertext: [u8; 1088]
  key_id_recipient: [u8; 16]

Encrypted Payload
  AES-256-GCM ciphertext (same size as plaintext + 16B auth tag)

Signature Footer (if also SIGNED)
  Signature covers header + encapsulated keys + encrypted payload
```

## 6. Capability Manifests (WITNESS_SEG)

WITNESS_SEGs provide cryptographic proof of provenance and computation:

### Witness Types

```
0x01  PROVENANCE      Who created this file and when
0x02  COMPUTATION     Proof that an index was correctly built
0x03  DELEGATION      Authorization chain for data access
0x04  AUDIT           Record of queries executed against this file
0x05  ATTESTATION     Hardware attestation (for Cognitum tiles)
```

### Provenance Witness

```
creator_key_id: [u8; 16]
creation_time: u64
tool_name: [u8; 64]
tool_version: [u8; 16]
input_hashes: [(hash256, description)]   Hashes of source data
transform_description: [u8; var]         What was done to create vectors
signature: [u8; var]                     Creator's signature over all above
```

### Computation Witness

```
computation_type: u8
  0 = HNSW construction
  1 = Quantization training
  2 = Temperature compaction
  3 = Overlay rebalance
  4 = Index merge

input_segments: [segment_id]
output_segments: [segment_id]
parameters: [(key, value)]
result_hash: hash256
duration_ns: u64
signature: [u8; var]
```

This allows any reader to verify that the index was built from the declared
vectors using the declared parameters — without re-running the computation.

## 7. Signing Performance Budget

For streaming ingest at 100K vectors/second with 1024-vector blocks:

```
Segment write rate:  ~100 segments/second (1024 vectors per VEC_SEG)
Manifest writes:     ~1/second (batched)

ML-DSA-65 signing:   ~4,500/second
Signing budget:      100 segment sigs + 1 manifest sig = 101/second
Utilization:         101 / 4,500 = 2.2%
```

Signing is not a bottleneck. Even at 10x the ingest rate, ML-DSA-65 has
headroom.

For verification during progressive load (reading 1000 segments):

```
ML-DSA-65 verify:    ~17,000/second
Verification budget: 1000 segments / 17,000 = 59 ms
```

All segments verified in under 60 ms. This runs concurrently with data
loading, so it adds minimal latency to the progressive boot sequence.

## 8. Core Profile Crypto

For the Core Profile (8 KB code budget), full ML-DSA-65 verification is
too large (~15 KB of code). Options:

1. **Hub verifies, tile trusts**: Hub checks all signatures before sending
   blocks to tiles. Tile only needs CRC32C for transport integrity.

2. **Truncated verification**: Tile verifies only the CRC32C of received
   blocks. Hub provides a signed attestation that the source segments
   were verified.

3. **FN-DSA-512**: Smaller verification code (~3 KB), 666 byte signatures.
   Fits in tile code budget but is less mature.

Recommended: Option 1 (hub verifies, tile trusts) for the initial release.
The hub is a trusted component in the Cognitum architecture, and the
tile-hub channel is physically secure (on-chip mesh).

## 9. Algorithm Agility

The `sig_algo` and `checksum_algo` fields in segment headers and footers
allow algorithm migration without format changes:

```
Today:       ML-DSA-65 signatures, SHAKE-256 hashes
Future:      May migrate to ML-DSA-87 or newer NIST standards
Transition:  Write new segments with new algo, old segments remain valid
Verification: Reader tries algo from header field, no guessing needed
```

New algorithms are introduced by:
1. Assigning a new enum value
2. Writing a CRYPTO_SEG with the new key type
3. Signing new segments with the new algorithm
4. Old segments with old signatures remain verifiable

No file rewrite needed. No flag day. Gradual migration through the
append-only segment model.
