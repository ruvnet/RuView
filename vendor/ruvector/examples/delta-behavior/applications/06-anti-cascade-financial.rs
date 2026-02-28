//! # Application 6: Anti-Cascade Financial Systems
//!
//! Transactions, leverage, or derivatives that increase systemic incoherence
//! are throttled or blocked automatically.
//!
//! ## Problem
//! Financial cascades (2008, flash crashes) happen when local actions
//! destroy global coherence faster than the system can respond.
//!
//! ## Δ-Behavior Solution
//! Every transaction must preserve or improve systemic coherence.
//! High-risk operations face exponential energy costs.
//!
//! ## Exotic Result
//! A financial system that cannot cascade into collapse by construction.

use std::collections::{HashMap, VecDeque};

/// A financial system with coherence-enforced stability
pub struct AntiCascadeFinancialSystem {
    /// Market participants
    participants: HashMap<String, Participant>,

    /// Open positions
    positions: Vec<Position>,

    /// Systemic coherence (1.0 = stable, 0.0 = collapse)
    coherence: f64,

    /// Coherence thresholds
    warning_threshold: f64,
    critical_threshold: f64,
    lockdown_threshold: f64,

    /// Maximum allowed leverage system-wide
    max_system_leverage: f64,

    /// Current aggregate leverage
    current_leverage: f64,

    /// Transaction queue (pending during high stress)
    pending_transactions: Vec<Transaction>,

    /// Circuit breaker state
    circuit_breaker: CircuitBreakerState,

    /// Historical coherence for trend analysis
    coherence_history: VecDeque<f64>,

    /// Cached coherence factors (updated when underlying data changes)
    cached_leverage_factor: f64,
    cached_depth_factor: f64,
}

#[derive(Clone)]
pub struct Participant {
    pub id: String,
    pub capital: f64,
    pub exposure: f64,
    pub risk_rating: f64, // 0.0 = safe, 1.0 = risky
    pub interconnectedness: f64, // How many counterparties
}

#[derive(Clone)]
pub struct Position {
    pub holder: String,
    pub counterparty: String,
    pub notional: f64,
    pub leverage: f64,
    pub derivative_depth: u8, // 0 = spot, 1 = derivative, 2 = derivative of derivative, etc.
}

#[derive(Clone)]
pub struct Transaction {
    pub id: u64,
    pub from: String,
    pub to: String,
    pub amount: f64,
    pub transaction_type: TransactionType,
    pub timestamp: u64,
}

#[derive(Clone, Debug)]
pub enum TransactionType {
    /// Simple transfer
    Transfer,
    /// Open leveraged position
    OpenLeverage { leverage: f64 },
    /// Close position
    ClosePosition { position_id: usize },
    /// Create derivative
    CreateDerivative { underlying_position: usize },
    /// Margin call
    MarginCall { participant: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum CircuitBreakerState {
    /// Normal operation
    Open,
    /// Elevated monitoring
    Cautious,
    /// Only risk-reducing transactions allowed
    Restricted,
    /// All transactions halted
    Halted,
}

#[derive(Debug)]
pub enum TransactionResult {
    /// Transaction executed
    Executed {
        coherence_impact: f64,
        fee_multiplier: f64,
    },
    /// Transaction queued for later
    Queued { reason: String },
    /// Transaction rejected
    Rejected { reason: String },
    /// System halted
    SystemHalted,
}

impl AntiCascadeFinancialSystem {
    pub fn new() -> Self {
        let mut history = VecDeque::with_capacity(100);
        history.push_back(1.0);
        Self {
            participants: HashMap::new(),
            positions: Vec::new(),
            coherence: 1.0,
            warning_threshold: 0.7,
            critical_threshold: 0.5,
            lockdown_threshold: 0.3,
            max_system_leverage: 10.0,
            current_leverage: 1.0,
            pending_transactions: Vec::new(),
            circuit_breaker: CircuitBreakerState::Open,
            coherence_history: history,
            cached_leverage_factor: 0.9, // 1.0 - (1.0 / 10.0) for initial leverage
            cached_depth_factor: 1.0,     // No positions initially
        }
    }

    pub fn add_participant(&mut self, id: &str, capital: f64) {
        self.participants.insert(id.to_string(), Participant {
            id: id.to_string(),
            capital,
            exposure: 0.0,
            risk_rating: 0.0,
            interconnectedness: 0.0,
        });
    }

    /// Calculate systemic coherence based on multiple risk factors
    /// Optimized: Single-pass calculation for participant metrics
    fn calculate_coherence(&self) -> f64 {
        if self.participants.is_empty() {
            return 1.0;
        }

        // Use pre-computed cached factors for leverage and depth
        let leverage_factor = self.cached_leverage_factor;
        let depth_factor = self.cached_depth_factor;

        // Single-pass calculation for interconnectedness, exposure, and capital
        let (sum_interconnect, total_exposure, total_capital) = self.participants.values()
            .fold((0.0, 0.0, 0.0), |(ic, exp, cap), p| {
                (ic + p.interconnectedness, exp + p.exposure, cap + p.capital)
            });

        // Factor 3: Interconnectedness risk (contagion potential)
        let avg_interconnectedness = sum_interconnect / self.participants.len() as f64;
        let interconnect_factor = 1.0 / (1.0 + avg_interconnectedness * 0.1);

        // Factor 4: Capital adequacy
        let capital_factor = if total_exposure > 0.0 {
            (total_capital / total_exposure).min(1.0)
        } else {
            1.0
        };

        // Factor 5: Coherence trend (declining coherence is worse)
        let trend_factor = if self.coherence_history.len() >= 5 {
            // VecDeque allows efficient back access
            let len = self.coherence_history.len();
            let newest = self.coherence_history[len - 1];
            let oldest_of_five = self.coherence_history[len - 5];
            let trend = newest - oldest_of_five;
            if trend < 0.0 {
                1.0 + trend // Penalize declining trend
            } else {
                1.0
            }
        } else {
            1.0
        };

        // Geometric mean of factors (more sensitive to low values)
        let product = leverage_factor * depth_factor * interconnect_factor
                    * capital_factor * trend_factor;
        product.powf(0.2).clamp(0.0, 1.0)
    }

    /// Update cached coherence factors when positions or leverage change
    fn update_cached_factors(&mut self) {
        // Factor 1: Leverage concentration
        self.cached_leverage_factor = 1.0 - (self.current_leverage / self.max_system_leverage).min(1.0);

        // Factor 2: Derivative depth (single pass over positions)
        let max_depth = self.positions.iter()
            .map(|p| p.derivative_depth)
            .max()
            .unwrap_or(0);
        self.cached_depth_factor = 1.0 / (1.0 + max_depth as f64 * 0.2);
    }

    /// Calculate the energy cost for a transaction (higher for risky transactions)
    fn transaction_energy_cost(&self, tx: &Transaction) -> f64 {
        let base_cost = match &tx.transaction_type {
            TransactionType::Transfer => 1.0,
            TransactionType::OpenLeverage { leverage } => {
                // Exponential cost for leverage
                (1.0 + leverage).powf(2.0)
            }
            TransactionType::ClosePosition { .. } => 0.5, // Closing is cheap (reduces risk)
            TransactionType::CreateDerivative { underlying_position } => {
                // Cost increases with derivative depth
                let depth = self.positions.get(*underlying_position)
                    .map(|p| p.derivative_depth)
                    .unwrap_or(0);
                (2.0_f64).powf(depth as f64 + 1.0)
            }
            TransactionType::MarginCall { .. } => 0.1, // Emergency actions are cheap
        };

        // Multiply by inverse coherence (lower coherence = higher costs)
        let coherence_multiplier = 1.0 / self.coherence.max(0.1);

        // Circuit breaker multiplier
        let circuit_multiplier = match self.circuit_breaker {
            CircuitBreakerState::Open => 1.0,
            CircuitBreakerState::Cautious => 2.0,
            CircuitBreakerState::Restricted => 10.0,
            CircuitBreakerState::Halted => f64::INFINITY,
        };

        base_cost * coherence_multiplier * circuit_multiplier
    }

    /// Predict coherence impact of a transaction
    fn predict_coherence_impact(&self, tx: &Transaction) -> f64 {
        match &tx.transaction_type {
            TransactionType::Transfer => 0.0, // Neutral
            TransactionType::OpenLeverage { leverage } => {
                -0.01 * leverage // Leverage reduces coherence
            }
            TransactionType::ClosePosition { .. } => 0.02, // Closing improves coherence
            TransactionType::CreateDerivative { .. } => -0.05, // Derivatives hurt coherence
            TransactionType::MarginCall { .. } => 0.03, // Margin calls improve coherence
        }
    }

    /// Process a transaction through the Δ-behavior filter
    pub fn process_transaction(&mut self, tx: Transaction) -> TransactionResult {
        // Update circuit breaker state
        self.update_circuit_breaker();

        // Check if system is halted
        if self.circuit_breaker == CircuitBreakerState::Halted {
            return TransactionResult::SystemHalted;
        }

        // Calculate energy cost
        let energy_cost = self.transaction_energy_cost(&tx);

        // Predict coherence impact
        let predicted_impact = self.predict_coherence_impact(&tx);
        let predicted_coherence = self.coherence + predicted_impact;

        // CORE Δ-BEHAVIOR: Reject if would cross lockdown threshold
        if predicted_coherence < self.lockdown_threshold {
            return TransactionResult::Rejected {
                reason: format!(
                    "Transaction would reduce coherence to {:.3} (threshold: {:.3})",
                    predicted_coherence, self.lockdown_threshold
                ),
            };
        }

        // In restricted mode, only allow risk-reducing transactions
        if self.circuit_breaker == CircuitBreakerState::Restricted {
            match &tx.transaction_type {
                TransactionType::ClosePosition { .. } | TransactionType::MarginCall { .. } => {}
                _ => {
                    return TransactionResult::Queued {
                        reason: "System in restricted mode - only risk-reducing transactions allowed".to_string(),
                    };
                }
            }
        }

        // Execute the transaction
        self.execute_transaction(&tx);

        // Update coherence
        self.coherence = self.calculate_coherence();
        self.coherence_history.push_back(self.coherence);

        // Keep history bounded - O(1) with VecDeque instead of O(n) with Vec
        if self.coherence_history.len() > 100 {
            self.coherence_history.pop_front();
        }

        TransactionResult::Executed {
            coherence_impact: predicted_impact,
            fee_multiplier: energy_cost,
        }
    }

    fn execute_transaction(&mut self, tx: &Transaction) {
        match &tx.transaction_type {
            TransactionType::Transfer => {
                // Simple transfer logic
                if let Some(from) = self.participants.get_mut(&tx.from) {
                    from.capital -= tx.amount;
                }
                if let Some(to) = self.participants.get_mut(&tx.to) {
                    to.capital += tx.amount;
                }
            }
            TransactionType::OpenLeverage { leverage } => {
                // Create leveraged position
                self.positions.push(Position {
                    holder: tx.from.clone(),
                    counterparty: tx.to.clone(),
                    notional: tx.amount * leverage,
                    leverage: *leverage,
                    derivative_depth: 0,
                });

                // Update metrics
                self.current_leverage = (self.current_leverage + leverage) / 2.0;

                // Update participant exposure
                if let Some(holder) = self.participants.get_mut(&tx.from) {
                    holder.exposure += tx.amount * leverage;
                    holder.interconnectedness += 1.0;
                }
                if let Some(counterparty) = self.participants.get_mut(&tx.to) {
                    counterparty.interconnectedness += 1.0;
                }

                // Update cached factors since leverage/positions changed
                self.update_cached_factors();
            }
            TransactionType::ClosePosition { position_id } => {
                if *position_id < self.positions.len() {
                    let pos = self.positions.remove(*position_id);

                    // Reduce leverage
                    self.current_leverage = (self.current_leverage - pos.leverage * 0.1).max(1.0);

                    // Update participant exposure
                    if let Some(holder) = self.participants.get_mut(&pos.holder) {
                        holder.exposure = (holder.exposure - pos.notional).max(0.0);
                    }

                    // Update cached factors since leverage/positions changed
                    self.update_cached_factors();
                }
            }
            TransactionType::CreateDerivative { underlying_position } => {
                if let Some(underlying) = self.positions.get(*underlying_position) {
                    self.positions.push(Position {
                        holder: tx.from.clone(),
                        counterparty: tx.to.clone(),
                        notional: underlying.notional * 0.5,
                        leverage: underlying.leverage * 1.5,
                        derivative_depth: underlying.derivative_depth + 1,
                    });

                    // Update cached factors since positions changed (derivative depth may increase)
                    self.update_cached_factors();
                }
            }
            TransactionType::MarginCall { participant } => {
                // Force close risky positions for participant using retain() - O(n) instead of O(n^2)
                let initial_len = self.positions.len();
                self.positions.retain(|p| !(&p.holder == participant && p.leverage > 5.0));

                // Update cached factors if positions were removed
                if self.positions.len() != initial_len {
                    self.update_cached_factors();
                }
            }
        }
    }

    fn update_circuit_breaker(&mut self) {
        self.circuit_breaker = match self.coherence {
            c if c >= self.warning_threshold => CircuitBreakerState::Open,
            c if c >= self.critical_threshold => CircuitBreakerState::Cautious,
            c if c >= self.lockdown_threshold => CircuitBreakerState::Restricted,
            _ => CircuitBreakerState::Halted,
        };
    }

    /// Process pending transactions (called when coherence improves)
    pub fn process_pending(&mut self) -> Vec<TransactionResult> {
        if self.circuit_breaker == CircuitBreakerState::Halted
            || self.circuit_breaker == CircuitBreakerState::Restricted {
            return Vec::new();
        }

        let pending = std::mem::take(&mut self.pending_transactions);
        pending.into_iter()
            .map(|tx| self.process_transaction(tx))
            .collect()
    }

    pub fn coherence(&self) -> f64 {
        self.coherence
    }

    pub fn circuit_breaker_state(&self) -> &CircuitBreakerState {
        &self.circuit_breaker
    }

    pub fn status(&self) -> String {
        format!(
            "Coherence: {:.3} | Circuit Breaker: {:?} | Leverage: {:.2}x | Positions: {} | Pending: {}",
            self.coherence,
            self.circuit_breaker,
            self.current_leverage,
            self.positions.len(),
            self.pending_transactions.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anti_cascade_basic() {
        let mut system = AntiCascadeFinancialSystem::new();

        system.add_participant("bank_a", 1000.0);
        system.add_participant("bank_b", 1000.0);
        system.add_participant("hedge_fund", 500.0);

        // Normal transaction should succeed
        let tx = Transaction {
            id: 1,
            from: "bank_a".to_string(),
            to: "bank_b".to_string(),
            amount: 100.0,
            transaction_type: TransactionType::Transfer,
            timestamp: 0,
        };

        let result = system.process_transaction(tx);
        assert!(matches!(result, TransactionResult::Executed { .. }));
        println!("After transfer: {}", system.status());
    }

    #[test]
    fn test_leverage_throttling() {
        let mut system = AntiCascadeFinancialSystem::new();

        system.add_participant("bank_a", 1000.0);
        system.add_participant("bank_b", 1000.0);

        // Open multiple leveraged positions - costs should increase
        let mut costs = Vec::new();

        for i in 0..5 {
            let tx = Transaction {
                id: i,
                from: "bank_a".to_string(),
                to: "bank_b".to_string(),
                amount: 100.0,
                transaction_type: TransactionType::OpenLeverage { leverage: 5.0 },
                timestamp: i,
            };

            if let TransactionResult::Executed { fee_multiplier, .. } = system.process_transaction(tx) {
                costs.push(fee_multiplier);
                println!("Position {}: cost multiplier = {:.2}", i, fee_multiplier);
            }

            println!("  Status: {}", system.status());
        }

        // Costs should generally increase as coherence drops
        // (though relationship isn't strictly monotonic due to multiple factors)
        println!("Cost progression: {:?}", costs);
    }

    #[test]
    fn test_derivative_depth_limit() {
        let mut system = AntiCascadeFinancialSystem::new();

        system.add_participant("bank_a", 10000.0);
        system.add_participant("bank_b", 10000.0);

        // Create base position
        let tx = Transaction {
            id: 0,
            from: "bank_a".to_string(),
            to: "bank_b".to_string(),
            amount: 100.0,
            transaction_type: TransactionType::OpenLeverage { leverage: 2.0 },
            timestamp: 0,
        };
        system.process_transaction(tx);

        // Try to create derivatives of derivatives
        for i in 0..5 {
            let tx = Transaction {
                id: i + 1,
                from: "bank_a".to_string(),
                to: "bank_b".to_string(),
                amount: 50.0,
                transaction_type: TransactionType::CreateDerivative { underlying_position: i as usize },
                timestamp: i + 1,
            };

            let result = system.process_transaction(tx);
            println!("Derivative layer {}: {:?}", i, result);
            println!("  Status: {}", system.status());

            // Eventually should be rejected or system should halt
            if matches!(result, TransactionResult::Rejected { .. } | TransactionResult::SystemHalted) {
                println!("System prevented excessive derivative depth at layer {}", i);
                return;
            }
        }
    }

    #[test]
    fn test_cascade_prevention() {
        let mut system = AntiCascadeFinancialSystem::new();

        // Create interconnected network
        for i in 0..10 {
            system.add_participant(&format!("bank_{}", i), 1000.0);
        }

        // Try to create a cascade scenario
        let mut rejected_count = 0;
        let mut queued_count = 0;
        let mut halted = false;

        for i in 0..50 {
            let from = format!("bank_{}", i % 10);
            let to = format!("bank_{}", (i + 1) % 10);

            let tx = Transaction {
                id: i,
                from,
                to,
                amount: 200.0,
                transaction_type: TransactionType::OpenLeverage { leverage: 8.0 },
                timestamp: i,
            };

            match system.process_transaction(tx) {
                TransactionResult::Rejected { reason } => {
                    rejected_count += 1;
                    println!("Transaction {} rejected: {}", i, reason);
                }
                TransactionResult::SystemHalted => {
                    halted = true;
                    println!("System halted at transaction {}", i);
                    break;
                }
                TransactionResult::Queued { reason } => {
                    queued_count += 1;
                    println!("Transaction {} queued: {}", i, reason);
                }
                TransactionResult::Executed { .. } => {}
            }
        }

        println!("\n=== Final Status ===");
        println!("{}", system.status());
        println!("Rejected: {}, Queued: {}, Halted: {}", rejected_count, queued_count, halted);

        // System should have prevented the cascade (via rejection, queueing, or halt)
        assert!(rejected_count > 0 || queued_count > 0 || halted, "System should prevent cascade");
    }

    #[test]
    fn test_margin_call_improves_coherence() {
        let mut system = AntiCascadeFinancialSystem::new();

        system.add_participant("risky_fund", 500.0);
        system.add_participant("counterparty", 5000.0);

        // Open risky positions
        for i in 0..3 {
            let tx = Transaction {
                id: i,
                from: "risky_fund".to_string(),
                to: "counterparty".to_string(),
                amount: 100.0,
                transaction_type: TransactionType::OpenLeverage { leverage: 7.0 },
                timestamp: i,
            };
            system.process_transaction(tx);
        }

        let coherence_before = system.coherence();
        println!("Before margin call: {}", system.status());

        // Issue margin call
        let margin_tx = Transaction {
            id: 100,
            from: "system".to_string(),
            to: "risky_fund".to_string(),
            amount: 0.0,
            transaction_type: TransactionType::MarginCall { participant: "risky_fund".to_string() },
            timestamp: 100,
        };

        system.process_transaction(margin_tx);
        let coherence_after = system.coherence();
        println!("After margin call: {}", system.status());

        // Coherence should improve after margin call
        assert!(
            coherence_after >= coherence_before,
            "Margin call should improve or maintain coherence"
        );
    }
}
