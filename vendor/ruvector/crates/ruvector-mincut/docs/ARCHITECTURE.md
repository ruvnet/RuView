# Architecture: Bounded-Range Dynamic Minimum Cut

## Overview

This crate implements the first deterministic exact fully-dynamic minimum cut
algorithm with subpolynomial update time, based on arxiv:2512.13105 (December 2024).

## System Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        MinCutWrapper                             │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │           O(log n) Bounded-Range Instances                   │ │
│  │  ┌─────────┐ ┌─────────┐ ┌─────────┐      ┌─────────┐       │ │
│  │  │ [1,1]   │ │ [1,1]   │ │ [2,2]   │ ... │ [λ,1.2λ]│       │ │
│  │  └────┬────┘ └────┬────┘ └────┬────┘      └────┬────┘       │ │
│  │       │           │           │                │             │ │
│  │       ▼           ▼           ▼                ▼             │ │
│  │  ┌──────────────────────────────────────────────────────┐   │ │
│  │  │              ProperCutInstance Trait                  │   │ │
│  │  │   - apply_inserts(edges)                             │   │ │
│  │  │   - apply_deletes(edges)                             │   │ │
│  │  │   - query() -> ValueInRange | AboveRange             │   │ │
│  │  └──────────────────────────────────────────────────────┘   │ │
│  └─────────────────────────────────────────────────────────────┘ │
│                              │                                    │
│  ┌───────────────────────────┴───────────────────────────────┐   │
│  │              DynamicConnectivity                           │   │
│  │   (Union-Find with rebuild on delete)                      │   │
│  └───────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│                    Supporting Components                         │
├─────────────────────────────────────────────────────────────────┤
│  LocalKCutOracle          │  DeterministicLocalKCut             │
│  - search(graph, query)   │  - BFS exploration                  │
│  - deterministic          │  - Early termination                │
├───────────────────────────┼─────────────────────────────────────┤
│  ClusterHierarchy         │  FragmentingAlgorithm               │
│  - O(log n) levels        │  - Connected components             │
│  - Recursive decomposition│  - Merge/split handling             │
├───────────────────────────┼─────────────────────────────────────┤
│  CutCertificate           │  AuditLogger                        │
│  - Witness tracking       │  - Provenance logging               │
│  - JSON export            │  - Thread-safe                      │
└───────────────────────────┴─────────────────────────────────────┘
```

## Component Responsibilities

### MinCutWrapper
- Manages O(log n) bounded-range instances
- Geometric ranges with factor 1.2
- Lazy instantiation
- Order invariant: inserts before deletes

### ProperCutInstance
- Abstract interface for cut maintenance
- Implementations: StubInstance, BoundedInstance

### DeterministicLocalKCut
- BFS-based local minimum cut search
- Fully deterministic (no randomness)
- Configurable radius and budget

### ClusterHierarchy
- Multi-level vertex clustering
- Fast boundary updates on edge changes

### FragmentingAlgorithm
- Handles graph disconnection
- Tracks connected components

## Data Flow

1. **Update arrives** (insert/delete edge)
2. **Wrapper buffers** the update with timestamp
3. **On query**:
   a. Check connectivity (fast path for disconnected)
   b. Process instances in order
   c. Apply buffered updates (inserts then deletes)
   d. Query each instance
   e. Stop at first ValueInRange or end
4. **Return result** with witness

## Invariants

1. **Range invariant**: Instance never sees λ < λ_min during update
2. **Order invariant**: Inserts applied before deletes
3. **Certificate invariant**: Every answer can be verified
4. **Determinism invariant**: Same sequence → same output

## Complexity

- **Update**: O(n^{o(1)}) amortized
- **Query**: O(log n) instances × O(n^{o(1)}) per instance
- **Space**: O(n + m) per instance
