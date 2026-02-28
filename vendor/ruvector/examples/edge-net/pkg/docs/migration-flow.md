# Edge-Net P2P Migration Flow

This document describes the hybrid bootstrap migration flow in @ruvector/edge-net, which enables gradual transition from Firebase-based signaling to a fully decentralized P2P network.

## Overview

The Edge-Net network uses a hybrid approach to bootstrap peer-to-peer connections:

1. **Firebase Mode**: Initial state using Firebase for peer discovery and WebRTC signaling
2. **Hybrid Mode**: Transitional state using both Firebase and DHT
3. **P2P Mode**: Fully decentralized using DHT and direct WebRTC connections

## Architecture

```
                    +----------------+
                    |   New Node     |
                    +-------+--------+
                            |
                            v
                    +----------------+
                    | Firebase Mode  |
                    | (Bootstrap)    |
                    +-------+--------+
                            |
                            | DHT peers >= threshold
                            v
                    +----------------+
                    | Hybrid Mode    |
                    | (Transition)   |
                    +-------+--------+
                            |
                            | Direct peers >= threshold
                            v
                    +----------------+
                    |   P2P Mode     |
                    | (Full Decentr) |
                    +----------------+
```

## State Machine

### States

| State    | Description                                      | Signaling Method        |
|----------|--------------------------------------------------|------------------------|
| firebase | Bootstrap phase using Firebase infrastructure    | Firebase Firestore     |
| hybrid   | Transition phase using both Firebase and DHT     | Firebase + DHT         |
| p2p      | Fully decentralized using DHT only              | Direct P2P + DHT       |

### Transitions

```
firebase ----[dhtPeers >= dhtPeerThreshold]----> hybrid
hybrid ----[connectedPeers >= p2pPeerThreshold]----> p2p
hybrid ----[dhtPeers < dhtPeerThreshold/2]----> firebase (fallback)
p2p ----[connectedPeers < p2pPeerThreshold/2]----> hybrid (fallback)
```

## Default Thresholds

| Parameter         | Default | Description                                    |
|-------------------|---------|------------------------------------------------|
| dhtPeerThreshold  | 5       | DHT peers needed to enter hybrid mode          |
| p2pPeerThreshold  | 10      | Direct peers needed to enter full P2P mode     |
| Migration check   | 30s     | Interval between migration state checks        |

### Fallback Thresholds

Fallback occurs at **half** the original threshold to prevent oscillation:

- **Hybrid -> Firebase**: When DHT peers drop below `dhtPeerThreshold / 2` (default: 2.5)
- **P2P -> Hybrid**: When direct peers drop below `p2pPeerThreshold / 2` (default: 5)

## Configuration

```javascript
import { HybridBootstrap } from '@ruvector/edge-net/firebase-signaling';

const bootstrap = new HybridBootstrap({
    peerId: 'unique-node-id',
    dhtPeerThreshold: 5,    // Custom threshold for hybrid transition
    p2pPeerThreshold: 10,   // Custom threshold for P2P transition
    firebaseConfig: {
        apiKey: '...',
        projectId: '...',
    },
});
```

## Migration Behavior

### 1. Firebase Mode (Bootstrap)

In this mode:
- Peer discovery via Firebase Firestore
- WebRTC signaling through Firebase
- No DHT operations

**Pros**:
- Reliable discovery for new nodes
- Works with any network configuration
- Low latency initial connections

**Cons**:
- Centralized dependency
- Firebase costs at scale
- Single point of failure

### 2. Hybrid Mode (Transition)

In this mode:
- Both Firebase and DHT active
- Signaling prefers P2P when available
- Falls back to Firebase for unknown peers

**Pros**:
- Graceful degradation
- Redundant discovery
- Smooth transition

**Cons**:
- Higher resource usage
- Complexity in routing decisions

### 3. P2P Mode (Full Decentralization)

In this mode:
- DHT-only peer discovery
- Direct WebRTC signaling via data channels
- Firebase maintained as emergency fallback

**Pros**:
- Fully decentralized
- No Firebase dependency
- Lower operating costs
- True P2P resilience

**Cons**:
- Requires sufficient network size
- NAT traversal challenges

## Signaling Fallback

The system implements intelligent signaling fallback:

```javascript
async signal(toPeerId, type, data) {
    // Prefer P2P if available
    if (this.webrtc?.isConnected(toPeerId)) {
        this.webrtc.sendToPeer(toPeerId, { type, data });
        return;
    }

    // Fall back to Firebase
    if (this.firebase?.isConnected) {
        await this.firebase.sendSignal(toPeerId, type, data);
        return;
    }

    throw new Error('No signaling path available');
}
```

## DHT Routing Table

The DHT uses a Kademlia-style routing table:

- **K-buckets**: 160 buckets (SHA-1 ID space)
- **Bucket size (K)**: 20 peers per bucket
- **Alpha**: 3 parallel lookups
- **Refresh interval**: 60 seconds
- **Peer timeout**: 5 minutes

### Population

The routing table is populated from:
1. Firebase peer discoveries
2. DHT FIND_NODE responses
3. Direct WebRTC connections

## Network Partition Recovery

When network partitions occur:

1. Nodes continue operating in their partition
2. Migration state may degrade (p2p -> hybrid -> firebase)
3. When partition heals, connections re-establish
4. Migration state recovers automatically

## Recommended Threshold Adjustments

### Small Networks (< 20 nodes)

```javascript
{
    dhtPeerThreshold: 3,
    p2pPeerThreshold: 6,
}
```

**Rationale**: Lower thresholds allow faster P2P transition in small deployments.

### Medium Networks (20-100 nodes)

```javascript
{
    dhtPeerThreshold: 5,
    p2pPeerThreshold: 10,
}
```

**Rationale**: Default values balance reliability with decentralization speed.

### Large Networks (100+ nodes)

```javascript
{
    dhtPeerThreshold: 10,
    p2pPeerThreshold: 25,
}
```

**Rationale**: Higher thresholds ensure network stability before reducing Firebase dependency.

### High Churn Networks

```javascript
{
    dhtPeerThreshold: 8,
    p2pPeerThreshold: 15,
}
```

**Rationale**: Buffer against rapid node departures with higher thresholds.

## Monitoring

### Key Metrics

| Metric                | Description                           |
|----------------------|---------------------------------------|
| mode                 | Current migration state               |
| firebaseDiscoveries  | Peers discovered via Firebase         |
| dhtDiscoveries       | Peers discovered via DHT              |
| directConnections    | Active WebRTC connections             |
| firebaseSignals      | Signals sent via Firebase             |
| p2pSignals          | Signals sent via P2P                  |

### Health Indicators

- **Healthy**: P2P mode with 10+ connected peers
- **Degraded**: Hybrid mode with 5-10 peers
- **Bootstrap**: Firebase mode with < 5 peers

## Testing

Run the migration test suite:

```bash
node tests/p2p-migration-test.js
```

### Test Scenarios

1. **Happy Path**: Gradual network growth
2. **Nodes Leaving**: Network shrinkage handling
3. **Nodes Rejoining**: Re-migration after recovery
4. **Network Partition**: Split and heal scenarios
5. **Signaling Fallback**: Route verification
6. **DHT Population**: Routing table validation
7. **Migration Timing**: Performance measurement
8. **Threshold Config**: Custom configuration testing

## Security Considerations

### WASM Cryptographic Identity

The migration system uses WASM-based cryptographic identity:
- Ed25519 key pairs generated in WASM
- All signaling messages are signed
- Peer verification before accepting connections

### Firebase Security

Firebase is secured via:
- Firestore security rules
- WASM signature verification
- No Firebase Auth dependency

## Future Improvements

1. **Adaptive Thresholds**: Dynamic threshold adjustment based on network conditions
2. **Reputation System**: Prefer reliable peers for DHT routing
3. **Geographic Awareness**: Consider latency in peer selection
4. **Predictive Migration**: Anticipate mode changes based on trends
5. **Multi-Firebase**: Support multiple Firebase projects for redundancy
