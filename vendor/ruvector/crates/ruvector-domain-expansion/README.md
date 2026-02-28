# ruvector-domain-expansion

[![Crates.io](https://img.shields.io/crates/v/ruvector-domain-expansion.svg)](https://crates.io/crates/ruvector-domain-expansion)
[![docs.rs](https://docs.rs/ruvector-domain-expansion/badge.svg)](https://docs.rs/ruvector-domain-expansion)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.77%2B-orange.svg)](https://www.rust-lang.org)

**Cross-domain transfer learning — train on one problem, get better at a different one automatically.**

```toml
ruvector-domain-expansion = "0.1"
```

Most AI systems learn one task at a time. Train a model on genomics and it can't trade stocks. Teach it quantum circuits and it won't plan workflows. `ruvector-domain-expansion` changes that: knowledge learned in one domain automatically transfers to other domains — and it **proves** the transfer actually helped before committing it. Genomics priors seed molecular design. Trading risk models improve resource allocation. Quantum noise detection accelerates signal processing. This is how real generalization works. Part of the [RuVector](https://github.com/ruvnet/ruvector) ecosystem.

| | ruvector-domain-expansion | Traditional Fine-Tuning |
|---|---|---|
| **Learning scope** | Learns across 13+ domains — genomics, trading, quantum, code, planning | One task at a time |
| **Transfer** | Automatic: priors from Domain 1 seed Domain 2 | Manual: retrain from scratch per domain |
| **Verification** | Transfer only accepted if it helps target without hurting source | No verification — hope it works |
| **Strategy selection** | Thompson Sampling picks the best approach per context | Fixed strategy for all inputs |
| **Population search** | 8 policy variants evolve in parallel, best survives | Single model, single strategy |
| **Curiosity** | Explores under-visited areas automatically | Only learns from data you provide |

## Quick Start

```rust
use ruvector_domain_expansion::{
    DomainExpansionEngine, DomainId, ContextBucket, ArmId,
};

let mut engine = DomainExpansionEngine::new();

// Generate training tasks in any domain
let domain = DomainId("rust_synthesis".into());
let tasks = engine.generate_tasks(&domain, 10, 0.5); // 10 tasks, medium difficulty

// Select strategy using Thompson Sampling
let bucket = ContextBucket { difficulty_tier: "medium".into(), category: "algorithm".into() };
let arm = engine.select_arm(&domain, &bucket).unwrap();

// Evaluate and learn
let eval = engine.evaluate_and_record(&domain, &tasks[0], &solution, bucket, arm);

// Transfer knowledge to a completely different domain
let target = DomainId("structured_planning".into());
engine.initiate_transfer(&domain, &target);
// Planning now starts at 0.70 accuracy instead of 0.30 — transfer verified and promoted
```

## Key Features

| Feature | What It Does | Why It Matters |
|---------|-------------|----------------|
| **Meta Thompson Sampling** | Picks the best strategy per context using uncertainty-aware selection | Explores when unsure, exploits when confident — no manual tuning |
| **Cross-Domain Transfer** | Extracts compact priors from one domain, seeds another | New domains learn faster by starting with knowledge from related domains |
| **Transfer Verification** | Accepts a transfer only if target improves without source regressing | Guarantees generalization — no silent regressions |
| **Population-Based Search** | Evolves 8 policy kernel variants in parallel | Finds optimal strategies faster than single-model training |
| **Curiosity-Driven Exploration** | UCB-style bonus for under-visited contexts | Automatically explores blind spots instead of getting stuck |
| **Pareto Front Tracking** | Tracks non-dominated kernels across accuracy, cost, and robustness | See the best tradeoffs, not just the single "best" model |
| **Plateau Detection** | Detects when learning stalls and recommends actions | Automatically switches strategies instead of wasting compute |
| **Counterexample Tracking** | Records failed solutions to inform future decisions | Learns from mistakes, not just successes |
| **Cost Curve & Scoreboard** | Tracks convergence speed per domain with acceleration metrics | Proves that transfer actually accelerated learning |
| **RVF Integration** | Package trained models as cognitive containers (optional `rvf` feature) | Ship a trained domain expansion engine as a single `.rvf` file |

## Domain Ecosystem

Domain expansion draws on the full RuVector capability stack. Each domain contributes unique knowledge that transfers to others through shared embedding spaces.

### Core Domains (Built-In)

| Domain | What It Generates | What It Evaluates |
|--------|------------------|-------------------|
| **Rust Synthesis** | Rust function specs (transforms, filters, searches) | Correctness, efficiency, idiomatic style |
| **Structured Planning** | Multi-step plans with dependencies and resources | Feasibility, completeness, dependency ordering |
| **Tool Orchestration** | Tool coordination tasks (parallel, error handling) | Correct sequencing, parallelism, failure recovery |

### Specialized Domains (via RuVector Crates & Examples)

| Domain | Crate / Example | What It Brings | Transfer Value |
|--------|----------------|----------------|----------------|
| **Genomics** | [rvDNA](../../examples/dna/) | Variant calling, k-mer HNSW embeddings, 64-dim SNP risk profiles | Sparse structured features seed any domain needing compact representations |
| **Algorithmic Trading** | [neural-trader](../../examples/neural-trader/) | Kelly sizing, LSTM-Transformer prediction, DRL portfolio ensembles | Rich reward signals (Sharpe, drawdown) map directly to evaluation scoring |
| **Quantum Computing** | [ruQu](../../crates/ruQu/) | Coherence gating, circuit optimization, noise drift detection | Verification methodology — "is it safe to act?" — inspired TransferVerification |
| **Neuromorphic AI** | [spiking-neural](../../examples/meta-cognition-spiking-neural-network/) | STDP learning, meta-plasticity, hyperbolic attention | Proves cross-domain acceleration is biologically real and measurable |
| **Graph Intelligence** | [graph-transformer](../ruvector-graph-transformer/) | Proof-gated mutation, Nash equilibrium attention, causal Granger layers | Formal proofs before committing changes — same pattern as transfer acceptance |
| **Nervous Systems** | [nervous-system](../ruvector-nervous-system/) | One-shot BTSP learning, hyperdimensional computing, circadian duty cycles | Cold-start acceleration — learn from single examples, like transfer priors |
| **Scientific OCR** | [scipix](../../examples/scipix/) | LaTeX/MathML extraction, equation vectorization at 50ms/image | Structured mathematical knowledge bootstraps reasoning patterns |
| **Knowledge Graphs** | [graph](../../examples/graph/) | Cypher queries, hybrid vector+graph search, community detection | Graph structure reveals which domain clusters should share priors |
| **Self-Learning Search** | [ruvector-gnn](../ruvector-gnn/) | GCN/GAT/GraphSAGE on HNSW topology | GraphSAGE handles new domains without retraining — inductive generalization |
| **Online Adaptation** | [sona](../sona/) | MicroLoRA (<1ms), EWC++ memory preservation, trajectory tracking | Fast-path arm updates + slow-path prior consolidation without forgetting |

### How Domains Connect

```
                    ┌──────────────┐
                    │   Domain     │
                    │  Expansion   │
                    │   Engine     │
                    └──────┬───────┘
                           │
            ┌──────────────┼──────────────┐
            │              │              │
     ┌──────▼──────┐ ┌────▼─────┐ ┌──────▼──────┐
     │  Genomics   │ │ Trading  │ │  Quantum    │
     │  64-dim SNP │ │ Sharpe   │ │  Coherence  │
     │  profiles   │ │ rewards  │ │  gates      │
     └──────┬──────┘ └────┬─────┘ └──────┬──────┘
            │              │              │
            └──────┬───────┘──────┬───────┘
                   │              │
            ┌──────▼──────┐ ┌────▼──────────┐
            │  Shared     │ │  Transfer     │
            │  Embedding  │ │  Verification │
            │  Space      │ │  Gate         │
            └──────┬──────┘ └────┬──────────┘
                   │              │
            ┌──────▼──────────────▼──────┐
            │  SONA (MicroLoRA + EWC++) │
            │  Live adaptation without  │
            │  forgetting old domains   │
            └───────────────────────────┘
```

Every domain produces embeddings in the same vector space. When you transfer from genomics to planning, the engine extracts compact priors (Beta posteriors from Thompson Sampling), seeds them into the target domain, and verifies the transfer helped — using the same coherence metrics that quantum computing uses to decide "is this circuit safe to run?"

## How Transfer Works

```
Domain 1 (Genomics)                Domain 2 (Drug Design)
┌─────────────────────┐            ┌─────────────────────┐
│ Train on 100 tasks   │            │ Start from scratch   │
│ Extract posteriors   │───prior──▶│ Seed with priors     │
│ Score: 0.85          │            │ Score after 45 runs: │
│                      │            │   0.70 (vs 0.30      │
│ k-mer embeddings     │            │   without transfer)  │
│ SNP risk profiles    │            │                      │
└─────────────────────┘            └─────────────────────┘
                                           │
                                    Verification Gate:
                                    ✓ Target improved (coherence check)
                                    ✓ Source didn't regress (EWC++ protected)
                                    ✓ Acceleration > 1.0 (scoreboard)
                                    → Transfer PROMOTED
```

### Cross-Domain Transfer Examples

| Source Domain | Target Domain | What Transfers | Why It Works |
|--------------|---------------|----------------|--------------|
| Genomics | Molecular Design | Sequence similarity priors, structural feature embeddings | Both work with sparse biological feature vectors |
| Trading | Resource Allocation | Risk/reward tradeoff models, Kelly-style sizing | Same math — allocate limited budget across uncertain options |
| Quantum | Signal Processing | Noise detection patterns, drift thresholds | Both need to separate signal from noise in noisy data |
| Spiking Neural | Attention Design | STDP timing rules, lateral inhibition patterns | Biological attention and AI attention share structural principles |
| Graph Transformer | Code Synthesis | Dependency ordering, proof-gated mutation logic | Code compilation and graph mutation both require valid ordering |
| Scientific OCR | Planning | Equation structure, logical step decomposition | Mathematical proofs and multi-step plans share sequential reasoning |

## Feature Flags

| Flag | Default | What It Enables |
|------|---------|-----------------|
| `rvf` | No | RVF cognitive container integration — serialize engines to `.rvf` format |

```toml
[dependencies]
ruvector-domain-expansion = { version = "0.1", features = ["rvf"] }
```

## API Overview

### Core Types

| Type | Description |
|------|-------------|
| `DomainExpansionEngine` | Main orchestrator — manages domains, transfer, population search |
| `Domain` (trait) | Implement to add custom domains — generate tasks, evaluate, embed |
| `DomainId` | Unique identifier for a domain |
| `Task` | A problem instance with difficulty, constraints, and spec |
| `Solution` | A candidate answer with content and structured data |
| `Evaluation` | Score (0.0–1.0) with correctness, efficiency, and elegance breakdown |

### Transfer & Strategy

| Type | Description |
|------|-------------|
| `MetaThompsonEngine` | Thompson Sampling with Beta priors across context buckets |
| `TransferPrior` | Compact posterior summary extracted from a trained domain |
| `TransferVerification` | Result of verifying a transfer — promotable only if both domains benefit |
| `PolicyKernel` | A strategy configuration with tunable knobs |
| `PopulationSearch` | Evolutionary search across policy kernel variants |

### Meta-Learning

| Type | Description |
|------|-------------|
| `MetaLearningEngine` | Regret tracking, plateau detection, Pareto front, curiosity bonuses |
| `CostCurve` | Convergence trajectory per domain |
| `AccelerationScoreboard` | Measures how much faster transfer makes learning |
| `ParetoFront` | Non-dominated set of kernels across accuracy/cost/robustness |

## Underlying Infrastructure

The domain expansion engine is built on top of these RuVector primitives:

| Layer | Crate | Role in Domain Expansion |
|-------|-------|--------------------------|
| **Retrieval** | [ruvector-gnn](../ruvector-gnn/) | GraphSAGE finds similar contexts across domains without retraining |
| **Adaptation** | [sona](../sona/) | MicroLoRA applies arm updates in <1ms; EWC++ prevents forgetting |
| **Verification** | [ruvector-coherence](../ruvector-coherence/) | Measures whether transfer preserved semantic quality (95% CI) |
| **Attention** | [ruvector-attn-mincut](../ruvector-attn-mincut/) | Min-cut prunes irrelevant domain connections before transfer |
| **Computation** | [ruvector-solver](../ruvector-solver/) | Forward Push PPR finds localized relevance across domain knowledge graphs |
| **Graph** | [ruvector-graph-transformer](../ruvector-graph-transformer/) | Proof-gated mutations ensure only verified knowledge transfers |
| **Packaging** | [rvf](../rvf/) | Ship a trained engine as a single `.rvf` cognitive container |

## License

**MIT License** — see [LICENSE](../../LICENSE) for details.

---

Part of [RuVector](https://github.com/ruvnet/ruvector) — the self-learning vector database.
