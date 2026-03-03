# Domain Models

This folder contains Domain-Driven Design (DDD) specifications for each major subsystem in RuView.

DDD organizes the codebase around the problem being solved — not around technical layers. Each *bounded context* owns its own data, rules, and language. Contexts communicate through domain events, not by sharing mutable state. This makes the system easier to reason about, test, and extend — whether you're a person or an AI agent.

## Models

| Model | What it covers | Bounded Contexts |
|-------|---------------|------------------|
| [RuvSense](ruvsense-domain-model.md) | Multistatic WiFi sensing, pose tracking, vital signs, edge intelligence | 7 contexts: Sensing, Coherence, Tracking, Field Model, Longitudinal, Spatial Identity, Edge Intelligence |
| [WiFi-Mat](wifi-mat-domain-model.md) | Disaster response: survivor detection, START triage, mass casualty assessment | Scan Zone, Survivor Tracking, Triage |
| [CHCI](chci-domain-model.md) | Coherent Human Channel Imaging: sub-millimeter body surface reconstruction | Sounding, Channel Estimation, Imaging |

## How to read these

Each model defines:

- **Ubiquitous Language** — Terms with precise meanings used in both code and conversation
- **Bounded Contexts** — Independent subsystems with clear responsibilities and boundaries
- **Aggregates** — Clusters of objects that enforce business rules (e.g., a PoseTrack owns its keypoints)
- **Value Objects** — Immutable data with meaning (e.g., a CoherenceScore is not just a float)
- **Domain Events** — Things that happened that other contexts may care about
- **Invariants** — Rules that must always be true (e.g., "drift alert requires >2sigma for >3 days")
- **Anti-Corruption Layers** — Adapters that translate between contexts without leaking internals

## Related

- [Architecture Decision Records](../adr/README.md) — Why each technical choice was made
- [User Guide](../user-guide.md) — Setup and API reference
