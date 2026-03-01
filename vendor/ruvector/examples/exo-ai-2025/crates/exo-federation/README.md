# exo-federation

Federated cognitive mesh with post-quantum cryptographic sovereignty for
distributed AI consciousness. Lets multiple EXO-AI substrates collaborate
across trust boundaries without sacrificing autonomy or security.

## Features

- **CRDT-based state replication** -- uses Last-Writer-Wins Maps
  (LWW-Map) and Grow-Only Sets (G-Set) for conflict-free convergence
  of shared cognitive state across nodes.
- **Byzantine consensus (PBFT)** -- Practical Byzantine Fault Tolerance
  ensures agreement even when up to f of 3f+1 nodes are faulty or
  adversarial.
- **Kyber post-quantum key exchange** -- establishes shared secrets
  resilient to quantum attacks using the NIST-standardised ML-KEM
  (Kyber) scheme.
- **Onion-routed messaging** -- wraps messages in layered encryption so
  intermediate relay nodes cannot observe payload or final destination.
- **Transfer CRDT** -- a purpose-built CRDT that merges cross-domain
  knowledge transfer records without coordination.

## Quick Start

Add the dependency to your `Cargo.toml`:

```toml
[dependencies]
exo-federation = "0.1"
```

Basic usage:

```rust
use exo_federation::{FederatedMesh, FederationScope, PeerAddress};

#[tokio::main]
async fn main() -> Result<()> {
    let substrate = SubstrateInstance {};
    let mut mesh = FederatedMesh::new(substrate)?;

    // Join a federation via post-quantum handshake
    let peer = PeerAddress::new("peer.example.com", 8080, peer_key);
    let token = mesh.join_federation(&peer).await?;

    // Execute a federated query across the mesh
    let results = mesh.federated_query(
        query_data,
        FederationScope::Global { max_hops: 5 },
    ).await?;

    // Commit state update with Byzantine consensus
    let proof = mesh.byzantine_commit(update).await?;
    Ok(())
}
```

## Crate Layout

| Module       | Purpose                                     |
|--------------|---------------------------------------------|
| `crdt`       | LWW-Map, G-Set, and transfer CRDT impls     |
| `consensus`  | PBFT protocol engine                         |
| `crypto`     | Kyber key exchange and onion routing         |
| `handshake`  | Federation joining protocol                  |
| `mesh`       | Peer discovery and connection management     |

## Requirements

- Rust 1.78+
- Depends on `exo-core`, `tokio`

## Links

- [GitHub](https://github.com/ruvnet/ruvector)
- [EXO-AI Documentation](https://github.com/ruvnet/ruvector/tree/main/examples/exo-ai-2025)

## License

MIT OR Apache-2.0
