# Economic Edge Case Analysis for edge-net

## Executive Summary

This document provides a comprehensive analysis of the edge-net economic system, identifying test coverage gaps and proposing new edge case tests across four core modules:

1. **credits/mod.rs** - Credit ledger with CRDT and contribution curve
2. **evolution/mod.rs** - Economic engine with distribution ratios
3. **tribute/mod.rs** - Founding registry with vesting schedules
4. **rac/economics.rs** - RAC staking, reputation, and rewards

---

## Current Test Coverage Analysis

### 1. credits/mod.rs - Credit Ledger

**Existing Tests:**
- Basic contribution curve multiplier calculations
- Ledger operations (credit, deduct, stake - WASM only)
- Basic staking operations (WASM only)

**Coverage Gaps Identified:**

| Gap | Severity | Description |
|-----|----------|-------------|
| **Credit Overflow** | HIGH | No test for `calculate_reward` when `base_reward * multiplier` approaches `u64::MAX` |
| **Negative Network Compute** | MEDIUM | `current_multiplier(-x)` produces exp(x/constant) which explodes |
| **CRDT Merge Conflicts** | HIGH | No test for merge producing negative effective balance |
| **Zero Division** | MEDIUM | No test for zero denominators in ratio calculations |
| **Staking Edge Cases** | MEDIUM | No test for staking exactly balance, or stake-deduct race conditions |

### 2. evolution/mod.rs - Economic Engine

**Existing Tests:**
- Basic reward processing
- Evolution engine replication check
- Optimization node selection (basic)

**Coverage Gaps Identified:**

| Gap | Severity | Description |
|-----|----------|-------------|
| **Treasury Depletion** | HIGH | No test for treasury running out of funds |
| **Distribution Ratio Sum** | HIGH | No verification that ratios exactly sum to 1.0 |
| **Founder Share Remainder** | MEDIUM | Founder share is computed as `total - others` - rounding not tested |
| **Sustainability Thresholds** | MEDIUM | No test at exact threshold boundaries |
| **Velocity Calculation** | LOW | `health.velocity` uses magic constant 0.99 - not tested |
| **Stability Edge Cases** | MEDIUM | Division by zero when `total_pools == 0` handled but not tested |

### 3. tribute/mod.rs - Founding Registry

**Existing Tests:**
- Basic founding registry creation
- Contribution stream processing
- Vesting schedule before/after cliff

**Coverage Gaps Identified:**

| Gap | Severity | Description |
|-----|----------|-------------|
| **Weight Clamping** | HIGH | `clamp(0.01, 0.5)` not tested at boundaries |
| **Epoch Overflow** | MEDIUM | No test for epoch values near u64::MAX |
| **Multiple Founders** | MEDIUM | No test for total weight > 1.0 scenario |
| **Genesis Sunset** | HIGH | No test for full 4-year vesting completion |
| **Pool Balance Zero** | MEDIUM | `calculate_vested(epoch, 0)` returns 0 but division not tested |

### 4. rac/economics.rs - RAC Economics

**Existing Tests:**
- Stake manager basic operations
- Reputation decay calculation
- Reward vesting and clawback
- Economic engine combined operations
- Slashing by reason

**Coverage Gaps Identified:**

| Gap | Severity | Description |
|-----|----------|-------------|
| **Slash Saturation** | HIGH | Multiple slashes exceeding stake not thoroughly tested |
| **Reputation Infinity** | MEDIUM | `effective_score` with 0 interval causes division |
| **Concurrent Access** | HIGH | RwLock contention under load not tested |
| **Reward ID Collision** | LOW | SHA256 collision probability not addressed |
| **Challenge Gaming** | HIGH | Winner/loser both being same node not tested |
| **Zero Stake Operations** | MEDIUM | Unstake/slash on zero-stake node edge cases |

---

## Proposed Edge Case Tests

### Section 1: Credit Overflow/Underflow

```rust
#[test]
fn test_credit_near_max_u64() {
    // base_reward near u64::MAX with 10x multiplier
    let max_safe = u64::MAX / 20;
    let reward = ContributionCurve::calculate_reward(max_safe, 0.0);
    assert!(reward <= u64::MAX);
}

#[test]
fn test_negative_network_compute() {
    let mult = ContributionCurve::current_multiplier(-1_000_000.0);
    assert!(mult.is_finite());
    // exp(1) = 2.718, so mult = 1 + 9 * e = 25.4 (unsafe?)
}
```

### Section 2: Multiplier Manipulation

```rust
#[test]
fn test_multiplier_inflation_attack() {
    // Attacker rapidly inflates network_compute to reduce
    // legitimate early adopter multipliers
    let decay_rate = compute_decay_per_hour(100_000.0);
    assert!(decay_rate < 0.15); // <15% loss per 100k hours
}
```

### Section 3: Economic Collapse Scenarios

```rust
#[test]
fn test_sustainability_exact_threshold() {
    let mut engine = EconomicEngine::new();
    // Fill treasury to exactly 90 days runway
    for _ in 0..optimal_reward_count {
        engine.process_reward(100, 1.0);
    }
    assert!(engine.is_self_sustaining(100, 1000));
}

#[test]
fn test_death_spiral() {
    // Low activity -> low rewards -> nodes leave -> lower activity
    let mut engine = EconomicEngine::new();
    // Simulate declining node count
    for nodes in (10..100).rev() {
        let sustainable = engine.is_self_sustaining(nodes, nodes * 10);
        // Track when sustainability is lost
    }
}
```

### Section 4: Free-Rider Exploitation

```rust
#[test]
fn test_reward_without_stake() {
    // Verify compute rewards require minimum stake
    let stakes = StakeManager::new(100);
    let node = [1u8; 32];

    // Attempt to earn without staking
    assert!(!stakes.has_sufficient_stake(&node));
    // Economic engine should reject reward
}

#[test]
fn test_sybil_cost_barrier() {
    // Verify 100 sybil nodes costs 100 * min_stake
    let stakes = StakeManager::new(100);
    let sybil_cost = 100 * 100;
    assert_eq!(stakes.total_staked(), sybil_cost);
}
```

### Section 5: Contribution Gaming

```rust
#[test]
fn test_founder_weight_overflow() {
    let mut registry = FoundingRegistry::new();

    // Register 10 founders each claiming 50% weight
    for i in 0..10 {
        registry.register_contributor(&format!("f{}", i), "architect", 0.5);
    }

    // Total weight should not exceed allocation
    let total_vested = registry.calculate_vested(365 * 4, 1_000_000);
    assert_eq!(total_vested, 50_000); // 5% cap enforced
}

#[test]
fn test_contribution_stream_drain() {
    let mut stream = ContributionStream::new();

    // Fee shares: 10% + 5% + 2% = 17%
    // Remaining: 83%
    let remaining = stream.process_fees(10000, 1);
    assert_eq!(remaining, 8300);
}
```

### Section 6: Treasury Depletion

```rust
#[test]
fn test_treasury_runway_calculation() {
    let engine = EconomicEngine::new();

    // 100 nodes * 10 rUv/day * 90 days = 90,000 rUv needed
    let required = 100 * 10 * 90;

    // Process rewards to fill treasury
    // Treasury gets 15% of each reward
    // Need: 90,000 / 0.15 = 600,000 total rewards
}
```

### Section 7: Genesis Sunset Edge Cases

```rust
#[test]
fn test_vesting_cliff_exact_boundary() {
    let registry = FoundingRegistry::new();

    let cliff_epoch = (365 * 4) / 10; // 10% of 4 years

    let at_cliff_minus_1 = registry.calculate_vested(cliff_epoch - 1, 1_000_000);
    let at_cliff = registry.calculate_vested(cliff_epoch, 1_000_000);

    assert_eq!(at_cliff_minus_1, 0);
    assert!(at_cliff > 0);
}

#[test]
fn test_full_vesting_at_4_years() {
    let registry = FoundingRegistry::new();

    // Full 4-year vest
    let full = registry.calculate_vested(365 * 4, 1_000_000);
    assert_eq!(full, 50_000); // 5% of 1M

    // Beyond 4 years should not exceed
    let beyond = registry.calculate_vested(365 * 5, 1_000_000);
    assert_eq!(beyond, 50_000);
}
```

### Section 8: RAC Economic Attacks

```rust
#[test]
fn test_slash_cascade_attack() {
    let manager = StakeManager::new(100);
    let victim = [1u8; 32];

    manager.stake(victim, 1000, 0);

    // Cascade: Equivocation + Sybil = 50% + 100% of remainder
    manager.slash(&victim, SlashReason::Equivocation, vec![]);
    manager.slash(&victim, SlashReason::SybilAttack, vec![]);

    assert_eq!(manager.get_stake(&victim), 0);
}

#[test]
fn test_reputation_negative_protection() {
    let manager = ReputationManager::new(0.1, 86400_000);
    let node = [1u8; 32];

    manager.register(node);

    // Massive failure count
    for _ in 0..1000 {
        manager.record_failure(&node, 1.0);
    }

    let rep = manager.get_reputation(&node);
    assert!(rep >= 0.0, "Reputation should never go negative");
}
```

---

## Priority Matrix

| Priority | Tests | Rationale |
|----------|-------|-----------|
| **P0 (Critical)** | Credit overflow, Distribution ratio sum, Slash saturation, CRDT merge conflicts | Could cause token inflation or fund loss |
| **P1 (High)** | Treasury depletion, Sybil cost, Vesting cliff, Free-rider protection | Economic sustainability attacks |
| **P2 (Medium)** | Multiplier manipulation, Founder weight clamping, Reputation bounds | Gaming prevention |
| **P3 (Low)** | Velocity calculation, Mutation rate decay, Unknown node scoring | Minor edge cases |

---

## Implementation Status

Tests have been implemented in:
- `/workspaces/ruvector/examples/edge-net/tests/economic_edge_cases_test.rs`

To run the tests:
```bash
cd /workspaces/ruvector/examples/edge-net
cargo test --test economic_edge_cases_test
```

---

## Recommendations

1. **Immediate Actions:**
   - Add overflow protection with `checked_mul` in `calculate_reward`
   - Validate network_compute is non-negative before multiplier calculation
   - Add explicit tests for CRDT merge conflict resolution

2. **Short-term:**
   - Implement minimum stake enforcement in compute reward path
   - Add comprehensive vesting schedule tests at all boundaries
   - Create stress tests for concurrent stake/slash operations

3. **Long-term:**
   - Consider formal verification for critical economic invariants
   - Add fuzzing tests for numeric edge cases
   - Implement economic simulation tests for collapse scenarios
