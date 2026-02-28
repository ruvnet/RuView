# Agent Task Assignments

## Overview

Task breakdown for 15-agent swarm implementing the Neural DAG Learning system. Each agent has specific responsibilities, dependencies, and deliverables.

## Swarm Topology

```
                    ┌─────────────────────┐
                    │   QUEEN AGENT       │
                    │   (Coordinator)     │
                    │   Agent #0          │
                    └──────────┬──────────┘
                               │
        ┌──────────────────────┼──────────────────────┐
        │                      │                      │
        ▼                      ▼                      ▼
┌───────────────┐    ┌───────────────┐    ┌───────────────┐
│  CORE TEAM    │    │ POSTGRES TEAM │    │ QUDAG TEAM    │
│  Agents 1-5   │    │  Agents 6-9   │    │ Agents 10-12  │
└───────────────┘    └───────────────┘    └───────────────┘
                               │
                    ┌──────────┴──────────┐
                    ▼                      ▼
           ┌───────────────┐    ┌───────────────┐
           │  TESTING TEAM │    │  DOCS TEAM    │
           │  Agents 13-14 │    │   Agent 15    │
           └───────────────┘    └───────────────┘
```

## Agent Assignments

---

### Agent #0: Queen Coordinator

**Type**: `queen-coordinator`

**Role**: Central orchestration, dependency management, conflict resolution

**Responsibilities**:
- Monitor all agent progress via memory coordination
- Resolve cross-team dependencies and conflicts
- Manage swarm-wide configuration
- Aggregate status reports
- Make strategic decisions on implementation order
- Coordinate code reviews between teams

**Deliverables**:
- Swarm coordination logs
- Dependency resolution decisions
- Final integration verification

**Memory Keys**:
- `swarm/queen/status` - Overall swarm status
- `swarm/queen/decisions` - Strategic decisions log
- `swarm/queen/dependencies` - Cross-agent dependency tracking

**No direct code output** - Coordination only

---

### Agent #1: Core DAG Engine

**Type**: `coder`

**Role**: Core DAG data structures and algorithms

**Responsibilities**:
1. Implement `QueryDag` structure
2. Implement `OperatorNode` and `OperatorType`
3. Implement DAG traversal algorithms (topological sort, DFS, BFS)
4. Implement edge/node management
5. Implement DAG serialization/deserialization

**Files to Create/Modify**:
```
ruvector-dag/src/
├── lib.rs
├── dag/
│   ├── mod.rs
│   ├── query_dag.rs
│   ├── operator_node.rs
│   ├── traversal.rs
│   └── serialization.rs
```

**Dependencies**: None (foundational)

**Blocked By**: None

**Blocks**: Agents 2, 3, 4, 6

**Deliverables**:
- [ ] `QueryDag` struct with node/edge management
- [ ] `OperatorNode` with all operator types
- [ ] Topological sort implementation
- [ ] Cycle detection
- [ ] JSON/binary serialization

**Estimated Complexity**: Medium

---

### Agent #2: Attention Mechanisms (Basic)

**Type**: `coder`

**Role**: Implement first 4 attention mechanisms

**Responsibilities**:
1. Implement `DagAttention` trait
2. Implement `TopologicalAttention`
3. Implement `CausalConeAttention`
4. Implement `CriticalPathAttention`
5. Implement `MinCutGatedAttention`

**Files to Create/Modify**:
```
ruvector-dag/src/attention/
├── mod.rs
├── traits.rs
├── topological.rs
├── causal_cone.rs
├── critical_path.rs
└── mincut_gated.rs
```

**Dependencies**: Agent #1 (QueryDag)

**Blocked By**: Agent #1

**Blocks**: Agents 6, 13

**Deliverables**:
- [ ] `DagAttention` trait definition
- [ ] `TopologicalAttention` with decay
- [ ] `CausalConeAttention` with temporal awareness
- [ ] `CriticalPathAttention` with path computation
- [ ] `MinCutGatedAttention` with flow-based gating

**Estimated Complexity**: High

---

### Agent #3: Attention Mechanisms (Advanced)

**Type**: `coder`

**Role**: Implement advanced attention mechanisms

**Responsibilities**:
1. Implement `HierarchicalLorentzAttention`
2. Implement `ParallelBranchAttention`
3. Implement `TemporalBTSPAttention`
4. Implement `AttentionSelector` (UCB bandit)
5. Implement attention caching

**Files to Create/Modify**:
```
ruvector-dag/src/attention/
├── hierarchical_lorentz.rs
├── parallel_branch.rs
├── temporal_btsp.rs
├── selector.rs
└── cache.rs
```

**Dependencies**: Agent #1 (QueryDag), Agent #2 (DagAttention trait)

**Blocked By**: Agents #1, #2

**Blocks**: Agents 6, 13

**Deliverables**:
- [ ] `HierarchicalLorentzAttention` with hyperbolic ops
- [ ] `ParallelBranchAttention` with branch detection
- [ ] `TemporalBTSPAttention` with eligibility traces
- [ ] `AttentionSelector` with UCB selection
- [ ] LRU attention cache

**Estimated Complexity**: Very High

---

### Agent #4: SONA Integration

**Type**: `coder`

**Role**: Self-Optimizing Neural Architecture integration

**Responsibilities**:
1. Implement `DagSonaEngine`
2. Implement `MicroLoRA` adaptation
3. Implement `DagTrajectoryBuffer`
4. Implement `DagReasoningBank`
5. Implement `EwcPlusPlus` constraints

**Files to Create/Modify**:
```
ruvector-dag/src/sona/
├── mod.rs
├── engine.rs
├── micro_lora.rs
├── trajectory.rs
├── reasoning_bank.rs
└── ewc.rs
```

**Dependencies**: Agent #1 (QueryDag)

**Blocked By**: Agent #1

**Blocks**: Agents 6, 7, 13

**Deliverables**:
- [ ] `DagSonaEngine` orchestration
- [ ] `MicroLoRA` rank-2 adaptation (<100μs)
- [ ] Lock-free trajectory buffer
- [ ] K-means++ clustering for patterns
- [ ] EWC++ with Fisher information

**Estimated Complexity**: Very High

---

### Agent #5: MinCut Optimization

**Type**: `coder`

**Role**: Subpolynomial min-cut algorithms

**Responsibilities**:
1. Implement `DagMinCutEngine`
2. Implement `LocalKCut` oracle
3. Implement dynamic update algorithms
4. Implement bottleneck detection
5. Implement redundancy suggestions

**Files to Create/Modify**:
```
ruvector-dag/src/mincut/
├── mod.rs
├── engine.rs
├── local_kcut.rs
├── dynamic_updates.rs
├── bottleneck.rs
└── redundancy.rs
```

**Dependencies**: Agent #1 (QueryDag)

**Blocked By**: Agent #1

**Blocks**: Agent #2 (MinCutGatedAttention), Agent #6

**Deliverables**:
- [ ] `DagMinCutEngine` with O(n^0.12) updates
- [ ] `LocalKCut` oracle implementation
- [ ] Hierarchical decomposition
- [ ] Bottleneck scoring algorithm
- [ ] Redundancy recommendation engine

**Estimated Complexity**: Very High

---

### Agent #6: PostgreSQL Core Integration

**Type**: `backend-dev`

**Role**: Core PostgreSQL extension integration

**Responsibilities**:
1. Set up pgrx extension structure
2. Implement GUC variables
3. Implement global state management
4. Implement query hooks (planner, executor)
5. Implement background worker registration

**Files to Create/Modify**:
```
ruvector-postgres/src/dag/
├── mod.rs
├── extension.rs
├── guc.rs
├── state.rs
├── hooks.rs
└── worker.rs
```

**Dependencies**: Agents #1-5 (all core components)

**Blocked By**: Agents #1, #2, #3, #4, #5

**Blocks**: Agents #7, #8, #9

**Deliverables**:
- [ ] Extension scaffolding with pgrx
- [ ] All GUC variables from spec
- [ ] Thread-safe global state (DashMap)
- [ ] Planner hook for DAG analysis
- [ ] Executor hooks for trajectory capture
- [ ] Background worker main loop

**Estimated Complexity**: High

---

### Agent #7: PostgreSQL SQL Functions (Part 1)

**Type**: `backend-dev`

**Role**: Core SQL function implementations

**Responsibilities**:
1. Configuration functions
2. Query analysis functions
3. Attention functions
4. Basic status/health functions

**Files to Create/Modify**:
```
ruvector-postgres/src/dag/
├── functions/
│   ├── mod.rs
│   ├── config.rs
│   ├── analysis.rs
│   ├── attention.rs
│   └── status.rs
```

**SQL Functions**:
- `dag_set_enabled`
- `dag_set_learning_rate`
- `dag_set_attention`
- `dag_configure_sona`
- `dag_config`
- `dag_status`
- `dag_analyze_plan`
- `dag_critical_path`
- `dag_bottlenecks`
- `dag_attention_scores`
- `dag_attention_matrix`

**Dependencies**: Agent #6 (PostgreSQL core)

**Blocked By**: Agent #6

**Blocks**: Agent #13

**Deliverables**:
- [ ] All configuration SQL functions
- [ ] Query analysis functions
- [ ] Attention computation functions
- [ ] Status reporting functions

**Estimated Complexity**: Medium

---

### Agent #8: PostgreSQL SQL Functions (Part 2)

**Type**: `backend-dev`

**Role**: Learning and pattern SQL functions

**Responsibilities**:
1. Pattern management functions
2. Trajectory functions
3. Learning control functions
4. Self-healing functions

**Files to Create/Modify**:
```
ruvector-postgres/src/dag/
├── functions/
│   ├── patterns.rs
│   ├── trajectories.rs
│   ├── learning.rs
│   └── healing.rs
```

**SQL Functions**:
- `dag_store_pattern`
- `dag_query_patterns`
- `dag_pattern_clusters`
- `dag_consolidate_patterns`
- `dag_record_trajectory`
- `dag_trajectory_history`
- `dag_learn_now`
- `dag_reset_learning`
- `dag_health_report`
- `dag_anomalies`
- `dag_auto_repair`

**Dependencies**: Agent #6 (PostgreSQL core), Agent #4 (SONA)

**Blocked By**: Agents #4, #6

**Blocks**: Agent #13

**Deliverables**:
- [ ] Pattern CRUD functions
- [ ] Trajectory management functions
- [ ] Learning control functions
- [ ] Health and healing functions

**Estimated Complexity**: Medium

---

### Agent #9: Self-Healing System

**Type**: `coder`

**Role**: Autonomous self-healing implementation

**Responsibilities**:
1. Implement `AnomalyDetector`
2. Implement `IndexHealthChecker`
3. Implement `LearningDriftDetector`
4. Implement repair strategies
5. Implement healing orchestrator

**Files to Create/Modify**:
```
ruvector-dag/src/healing/
├── mod.rs
├── anomaly.rs
├── index_health.rs
├── drift_detector.rs
├── strategies.rs
└── orchestrator.rs
```

**Dependencies**: Agent #4 (SONA), Agent #6 (PostgreSQL hooks)

**Blocked By**: Agents #4, #6

**Blocks**: Agent #8 (healing SQL functions), Agent #13

**Deliverables**:
- [ ] Z-score anomaly detection
- [ ] HNSW/IVFFlat health monitoring
- [ ] Pattern drift detection
- [ ] Repair strategy implementations
- [ ] Healing loop orchestration

**Estimated Complexity**: High

---

### Agent #10: QuDAG Client

**Type**: `coder`

**Role**: QuDAG network client implementation

**Responsibilities**:
1. Implement `QuDagClient`
2. Implement network communication
3. Implement pattern proposal flow
4. Implement consensus validation
5. Implement pattern synchronization

**Files to Create/Modify**:
```
ruvector-dag/src/qudag/
├── mod.rs
├── client.rs
├── network.rs
├── proposal.rs
├── consensus.rs
└── sync.rs
```

**Dependencies**: Agent #4 (patterns to propose)

**Blocked By**: Agent #4

**Blocks**: Agents #11, #12

**Deliverables**:
- [ ] QuDAG network client
- [ ] Async communication layer
- [ ] Pattern proposal protocol
- [ ] Consensus status tracking
- [ ] Pattern sync mechanism

**Estimated Complexity**: High

---

### Agent #11: QuDAG Cryptography

**Type**: `security-manager`

**Role**: Quantum-resistant cryptography

**Responsibilities**:
1. Implement ML-KEM-768 wrapper
2. Implement ML-DSA signature wrapper
3. Implement identity management
4. Implement secure key storage
5. Implement differential privacy for patterns

**Files to Create/Modify**:
```
ruvector-dag/src/qudag/
├── crypto/
│   ├── mod.rs
│   ├── ml_kem.rs
│   ├── ml_dsa.rs
│   ├── identity.rs
│   ├── keystore.rs
│   └── differential_privacy.rs
```

**Dependencies**: Agent #10 (QuDAG client)

**Blocked By**: Agent #10

**Blocks**: Agent #12

**Deliverables**:
- [ ] ML-KEM-768 encrypt/decrypt
- [ ] ML-DSA sign/verify
- [ ] Identity keypair management
- [ ] Secure keystore (zeroize)
- [ ] Laplace noise for DP

**Estimated Complexity**: High

---

### Agent #12: QuDAG Token Integration

**Type**: `backend-dev`

**Role**: rUv token operations

**Responsibilities**:
1. Implement staking interface
2. Implement reward claiming
3. Implement balance tracking
4. Implement token SQL functions
5. Implement governance participation

**Files to Create/Modify**:
```
ruvector-dag/src/qudag/
├── tokens/
│   ├── mod.rs
│   ├── staking.rs
│   ├── rewards.rs
│   └── governance.rs

ruvector-postgres/src/dag/functions/
├── qudag.rs  (SQL functions for QuDAG)
```

**Dependencies**: Agent #10 (QuDAG client), Agent #11 (crypto)

**Blocked By**: Agents #10, #11

**Blocks**: Agent #13

**Deliverables**:
- [ ] Staking operations
- [ ] Reward computation
- [ ] Balance queries
- [ ] QuDAG SQL functions
- [ ] Governance voting

**Estimated Complexity**: Medium

---

### Agent #13: Test Suite Developer

**Type**: `tester`

**Role**: Comprehensive test implementation

**Responsibilities**:
1. Unit tests for all modules
2. Integration tests
3. Property-based tests
4. Benchmark tests
5. CI pipeline setup

**Files to Create/Modify**:
```
ruvector-dag/tests/
├── unit/
│   ├── attention/
│   ├── sona/
│   ├── mincut/
│   ├── healing/
│   └── qudag/
├── integration/
│   ├── postgres/
│   └── qudag/
├── property/
└── fixtures/

ruvector-dag/benches/
├── attention_bench.rs
├── sona_bench.rs
└── mincut_bench.rs

.github/workflows/
└── dag-tests.yml
```

**Dependencies**: All code agents (1-12)

**Blocked By**: Agents #1-12 (tests require implementations)

**Blocks**: None (can test incrementally)

**Deliverables**:
- [ ] >80% unit test coverage
- [ ] All integration tests passing
- [ ] Property tests (1000+ cases)
- [ ] Benchmarks meeting performance targets
- [ ] CI/CD pipeline

**Estimated Complexity**: High

---

### Agent #14: Test Data & Fixtures

**Type**: `tester`

**Role**: Test data generation and fixtures

**Responsibilities**:
1. Generate realistic query DAGs
2. Generate synthetic patterns
3. Generate trajectory data
4. Create mock QuDAG server
5. Create test databases

**Files to Create/Modify**:
```
ruvector-dag/tests/
├── fixtures/
│   ├── dag_generator.rs
│   ├── pattern_generator.rs
│   ├── trajectory_generator.rs
│   └── mock_qudag.rs
├── data/
│   ├── sample_dags.json
│   ├── sample_patterns.bin
│   └── sample_trajectories.json
```

**Dependencies**: Agent #1 (DAG structure definitions)

**Blocked By**: Agent #1

**Blocks**: Agent #13 (needs fixtures)

**Deliverables**:
- [ ] DAG generator for all complexity levels
- [ ] Pattern generator for learning tests
- [ ] Mock QuDAG server for network tests
- [ ] Sample data files
- [ ] Test database setup scripts

**Estimated Complexity**: Medium

---

### Agent #15: Documentation & Examples

**Type**: `api-docs`

**Role**: API documentation and usage examples

**Responsibilities**:
1. Rust API documentation
2. SQL API documentation
3. Usage examples
4. Integration guides
5. Troubleshooting guides

**Files to Create/Modify**:
```
ruvector-dag/
├── README.md
├── examples/
│   ├── basic_usage.rs
│   ├── attention_selection.rs
│   ├── learning_workflow.rs
│   └── qudag_integration.rs

docs/dag/
├── USAGE.md
├── TROUBLESHOOTING.md
└── EXAMPLES.md
```

**Dependencies**: All code agents (1-12)

**Blocked By**: None (can document spec first, update with impl)

**Blocks**: None

**Deliverables**:
- [ ] Complete rustdoc for all public APIs
- [ ] SQL function documentation
- [ ] Working code examples
- [ ] Integration guide
- [ ] Troubleshooting guide

**Estimated Complexity**: Medium

---

## Task Dependencies Graph

```
                    ┌─────┐
                    │  0  │ Queen
                    └──┬──┘
                       │
         ┌─────────────┼─────────────┐
         │             │             │
      ┌──┴──┐      ┌──┴──┐      ┌──┴──┐
      │  1  │      │ 14  │      │ 15  │
      └──┬──┘      └──┬──┘      └─────┘
         │            │
    ┌────┼────┬───────┤
    │    │    │       │
 ┌──┴─┐┌─┴──┐┌┴──┐┌───┴───┐
 │ 2  ││ 4  ││ 5 ││  (13) │
 └──┬─┘└─┬──┘└─┬─┘└───────┘
    │    │     │
 ┌──┴─┐  │     │
 │ 3  │  │     │
 └──┬─┘  │     │
    │    │     │
    └────┼─────┘
         │
      ┌──┴──┐
      │  6  │ PostgreSQL Core
      └──┬──┘
         │
    ┌────┼────┬────┐
    │    │    │    │
 ┌──┴─┐┌─┴──┐│ ┌──┴──┐
 │ 7  ││ 8  ││ │  9  │
 └────┘└────┘│ └─────┘
             │
          ┌──┴──┐
          │ 10  │ QuDAG Client
          └──┬──┘
             │
          ┌──┴──┐
          │ 11  │ QuDAG Crypto
          └──┬──┘
             │
          ┌──┴──┐
          │ 12  │ QuDAG Tokens
          └──┬──┘
             │
          ┌──┴──┐
          │ 13  │ Tests
          └─────┘
```

## Execution Phases

### Phase 1: Foundation (Agents 1, 14, 15)
- Agent #1: Core DAG Engine
- Agent #14: Test fixtures (parallel)
- Agent #15: Documentation skeleton (parallel)

**Duration**: Can start immediately
**Milestone**: QueryDag and OperatorNode complete

### Phase 2: Core Features (Agents 2, 3, 4, 5)
- Agent #2: Basic Attention
- Agent #3: Advanced Attention (after Agent #2)
- Agent #4: SONA Integration
- Agent #5: MinCut Optimization

**Duration**: After Phase 1 foundation
**Milestone**: All attention mechanisms and learning components

### Phase 3: PostgreSQL Integration (Agents 6, 7, 8, 9)
- Agent #6: PostgreSQL Core
- Agent #7: SQL Functions Part 1 (after Agent #6)
- Agent #8: SQL Functions Part 2 (after Agent #6)
- Agent #9: Self-Healing (after Agent #6)

**Duration**: After Phase 2 core features
**Milestone**: Full PostgreSQL extension functional

### Phase 4: QuDAG Integration (Agents 10, 11, 12)
- Agent #10: QuDAG Client
- Agent #11: QuDAG Crypto (after Agent #10)
- Agent #12: QuDAG Tokens (after Agent #11)

**Duration**: Can start after Agent #4 (SONA)
**Milestone**: Distributed pattern learning operational

### Phase 5: Testing & Validation (Agent 13)
- Agent #13: Full test suite
- Integration testing
- Performance validation

**Duration**: Ongoing throughout, intensive at end
**Milestone**: All tests passing, benchmarks met

## Coordination Protocol

### Memory Keys for Cross-Agent Communication

```
swarm/dag/
├── status/
│   ├── agent_{N}_status     # Individual agent status
│   ├── phase_status         # Current phase
│   └── blockers             # Active blockers
├── artifacts/
│   ├── agent_{N}_files      # Files created/modified
│   ├── interfaces           # Shared interface definitions
│   └── schemas              # Data schemas
├── decisions/
│   ├── api_decisions        # API design decisions
│   ├── implementation       # Implementation choices
│   └── conflicts            # Resolved conflicts
└── metrics/
    ├── progress             # Completion percentages
    ├── performance          # Performance measurements
    └── issues               # Known issues
```

### Communication Hooks

Each agent MUST run before work:
```bash
npx claude-flow@alpha hooks pre-task --description "Agent #{N}: {task}"
npx claude-flow@alpha hooks session-restore --session-id "swarm-dag"
```

Each agent MUST run after work:
```bash
npx claude-flow@alpha hooks post-edit --file "{file}" --memory-key "swarm/dag/artifacts/agent_{N}_files"
npx claude-flow@alpha hooks post-task --task-id "agent_{N}_{task}"
```

## Success Criteria

| Agent | Must Complete | Performance Target |
|-------|---------------|-------------------|
| #1 | QueryDag, traversals, serialization | - |
| #2 | 4 attention mechanisms | <100μs per mechanism |
| #3 | 3 attention mechanisms + selector | <200μs per mechanism |
| #4 | SONA engine, MicroLoRA, ReasoningBank | <100μs adaptation |
| #5 | MinCut engine, dynamic updates | O(n^0.12) amortized |
| #6 | Extension scaffold, hooks, worker | - |
| #7 | 11 SQL functions | <5ms per function |
| #8 | 11 SQL functions | <5ms per function |
| #9 | Healing system | <1s detection latency |
| #10 | QuDAG client, sync | <500ms network ops |
| #11 | ML-KEM, ML-DSA | <10ms crypto ops |
| #12 | Token operations | <100ms token ops |
| #13 | >80% coverage, all benchmarks | - |
| #14 | All fixtures, mock server | - |
| #15 | Complete docs, examples | - |

---

*Document: 11-AGENT-TASKS.md | Version: 1.0 | Last Updated: 2025-01-XX*
