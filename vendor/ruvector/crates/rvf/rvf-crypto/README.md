# rvf-crypto

[![Crates.io](https://img.shields.io/crates/v/rvf-crypto.svg)](https://crates.io/crates/rvf-crypto)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](https://opensource.org/licenses/MIT)

**Tamper-proof hashing and signing for every RVF segment -- SHA-3 digests, Ed25519 signatures, and lineage witness chains.**

```toml
rvf-crypto = "0.1"
```

Every operation on an RVF file gets recorded in a cryptographic witness chain. `rvf-crypto` provides the primitives that make this possible: SHA-3 (SHAKE-256) content hashing for segment identity, Ed25519 digital signatures for provenance, and lineage verification functions that ensure no record in the chain has been altered. If you are building tools that read, write, or transform `.rvf` files, this crate handles all the cryptography so you do not have to.

| | rvf-crypto | Manual hashing + signing | No integrity checks |
|---|---|---|---|
| **Segment identity** | SHAKE-256-256 content-addressable IDs | Roll your own digest scheme | Rely on filenames |
| **Provenance** | Ed25519 signatures on every segment | Integrate a signing library yourself | Trust the source blindly |
| **Lineage verification** | One function call validates an entire chain | Write chain-walking logic from scratch | No verification possible |
| **no_std / WASM** | Hashing works without std; signing is feature-gated | Varies by library | N/A |

## Quick Start

```rust
use rvf_crypto::lineage::{lineage_record_to_bytes, lineage_record_from_bytes, verify_lineage_chain};
use rvf_types::{LineageRecord, DerivationType, FileIdentity};

// Serialize a lineage record to a fixed 128-byte array
let record = LineageRecord::new(
    [1u8; 16], [2u8; 16], [3u8; 32],
    DerivationType::Filter, 5, 1_700_000_000_000_000_000,
    "filtered by category",
);
let bytes = lineage_record_to_bytes(&record);
let decoded = lineage_record_from_bytes(&bytes).unwrap();
assert_eq!(decoded.description_str(), "filtered by category");

// Verify a parent-child lineage chain
let root = FileIdentity::new_root([1u8; 16]);
let root_hash = [0xAAu8; 32];
let child = FileIdentity {
    file_id: [2u8; 16],
    parent_id: [1u8; 16],
    parent_hash: root_hash,
    lineage_depth: 1,
};
verify_lineage_chain(&[(root, root_hash), (child, [0xBBu8; 32])]).unwrap();
```

## Key Features

| Feature | What It Does | Why It Matters |
|---|---|---|
| **SHA-3 (SHAKE-256)** | Content-addressable hashing for segment identifiers | Every segment gets a unique, collision-resistant ID |
| **Ed25519 signing** | Segment-level digital signatures via `ed25519-dalek` | Proves who created or modified a segment |
| **Lineage witness chains** | Cryptographic chain linking parent and child segments | Detects tampering anywhere in the derivation history |
| **Record serialization** | Fixed 128-byte binary codec for `LineageRecord` | Compact, deterministic encoding for witness entries |
| **Manifest hashing** | SHAKE-256-256 over 4096-byte manifests | Anchors `FileIdentity` parent references to real data |
| **Chain verification** | `verify_lineage_chain()` validates root-to-leaf integrity | One call proves the entire history is intact |

## Feature Flags

| Flag | Default | What It Enables |
|---|---|---|
| `std` | Yes | Standard library support |
| `ed25519` | Yes | Ed25519 signing via `ed25519-dalek` |

For `no_std` or WASM targets that only need hashing and witness chains (no signing), disable defaults:

```toml
[dependencies]
rvf-crypto = { version = "0.1", default-features = false }
```

## API Reference

| Function | Description |
|---|---|
| `lineage_record_to_bytes(record)` | Serialize a `LineageRecord` to a fixed 128-byte array |
| `lineage_record_from_bytes(bytes)` | Deserialize a `LineageRecord` from 128 bytes |
| `lineage_witness_entry(record, prev_hash)` | Create a `WitnessEntry` (type `0x09`) for a derivation event |
| `compute_manifest_hash(manifest)` | SHAKE-256-256 digest over a 4096-byte manifest |
| `verify_lineage_chain(chain)` | Validate parent-child integrity from root to leaf |

## License

MIT OR Apache-2.0

---

Part of [RuVector](https://github.com/ruvnet/ruvector) -- the self-learning vector database.
