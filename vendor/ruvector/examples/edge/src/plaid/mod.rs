//! Plaid API Integration with Browser-Local Learning
//!
//! This module provides privacy-preserving financial data analysis that runs entirely
//! in the browser. No financial data, learning patterns, or AI models ever leave the
//! client device.
//!
//! ## Modules
//!
//! - `zkproofs` - Zero-knowledge proofs for financial statements
//! - `wasm` - WASM bindings for browser integration
//! - `zk_wasm` - WASM bindings for ZK proofs

pub mod zkproofs;
pub mod zkproofs_prod;

#[cfg(feature = "wasm")]
pub mod wasm;

#[cfg(feature = "wasm")]
pub mod zk_wasm;

#[cfg(feature = "wasm")]
pub mod zk_wasm_prod;

// Re-export demo ZK types (for backward compatibility)
pub use zkproofs::{
    ZkProof, ProofType, VerificationResult, Commitment,
    FinancialProofBuilder, RentalApplicationProof,
};

// Re-export production ZK types
pub use zkproofs_prod::{
    PedersenCommitment, ZkRangeProof, ProofMetadata,
    VerificationResult as ProdVerificationResult,
    FinancialProver, FinancialVerifier, RentalApplicationBundle,
};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Financial transaction from Plaid
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub transaction_id: String,
    pub account_id: String,
    pub amount: f64,
    pub date: String,
    pub name: String,
    pub merchant_name: Option<String>,
    pub category: Vec<String>,
    pub pending: bool,
    pub payment_channel: String,
}

/// Spending pattern learned from transactions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpendingPattern {
    pub pattern_id: String,
    pub category: String,
    pub avg_amount: f64,
    pub frequency_days: f32,
    pub confidence: f64,
    pub last_seen: u64,
}

/// Category prediction result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryPrediction {
    pub category: String,
    pub confidence: f64,
    pub similar_transactions: Vec<String>,
}

/// Anomaly detection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyResult {
    pub is_anomaly: bool,
    pub anomaly_score: f64,
    pub reason: String,
    pub expected_amount: f64,
}

/// Budget recommendation from learning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetRecommendation {
    pub category: String,
    pub recommended_limit: f64,
    pub current_avg: f64,
    pub trend: String, // "increasing", "stable", "decreasing"
    pub confidence: f64,
}

/// Local learning state for financial patterns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinancialLearningState {
    pub version: u64,
    pub patterns: HashMap<String, SpendingPattern>,
    /// Category embeddings - HashMap prevents unbounded growth (was Vec which leaked memory)
    pub category_embeddings: HashMap<String, Vec<f32>>,
    pub q_values: HashMap<String, f64>, // state|action -> Q-value
    pub temporal_weights: Vec<f32>, // Day-of-week weights (7 days: Sun-Sat)
    pub monthly_weights: Vec<f32>,  // Day-of-month weights (31 days)
    /// Maximum embeddings to store (LRU eviction when exceeded)
    #[serde(default = "default_max_embeddings")]
    pub max_embeddings: usize,
}

fn default_max_embeddings() -> usize {
    10_000 // ~400KB at 10 floats per embedding
}

impl Default for FinancialLearningState {
    fn default() -> Self {
        Self {
            version: 0,
            patterns: HashMap::new(),
            category_embeddings: HashMap::new(),
            q_values: HashMap::new(),
            temporal_weights: vec![1.0; 7],  // 7 days
            monthly_weights: vec![1.0; 31],   // 31 days
            max_embeddings: default_max_embeddings(),
        }
    }
}

/// Transaction feature vector for ML
#[derive(Debug, Clone)]
pub struct TransactionFeatures {
    pub amount_normalized: f32,
    pub day_of_week: f32,
    pub day_of_month: f32,
    pub hour_of_day: f32,
    pub is_weekend: f32,
    pub category_hash: Vec<f32>, // LSH of category text
    pub merchant_hash: Vec<f32>, // LSH of merchant name
}

impl TransactionFeatures {
    /// Convert to embedding vector for HNSW indexing
    pub fn to_embedding(&self) -> Vec<f32> {
        let mut vec = vec![
            self.amount_normalized,
            self.day_of_week / 7.0,
            self.day_of_month / 31.0,
            self.hour_of_day / 24.0,
            self.is_weekend,
        ];
        vec.extend(&self.category_hash);
        vec.extend(&self.merchant_hash);
        vec
    }
}

/// Extract features from a transaction
pub fn extract_features(tx: &Transaction) -> TransactionFeatures {
    // Parse date for temporal features
    let (dow, dom, _hour) = parse_date(&tx.date);

    // Normalize amount (log scale, clipped)
    let amount_normalized = (tx.amount.abs().ln() / 10.0).min(1.0) as f32;

    // LSH hash for category
    let category_text = tx.category.join(" ");
    let category_hash = simple_lsh(&category_text, 8);

    // LSH hash for merchant
    let merchant = tx.merchant_name.as_deref().unwrap_or(&tx.name);
    let merchant_hash = simple_lsh(merchant, 8);

    TransactionFeatures {
        amount_normalized,
        day_of_week: dow as f32,
        day_of_month: dom as f32,
        hour_of_day: 12.0, // Default to noon if no time
        is_weekend: if dow >= 5 { 1.0 } else { 0.0 },
        category_hash,
        merchant_hash,
    }
}

/// Simple LSH (locality-sensitive hashing) for text
fn simple_lsh(text: &str, dims: usize) -> Vec<f32> {
    let mut hash = vec![0.0f32; dims];
    let text_lower = text.to_lowercase();

    for (i, c) in text_lower.chars().enumerate() {
        let idx = (c as usize + i * 31) % dims;
        hash[idx] += 1.0;
    }

    // Normalize
    let norm: f32 = hash.iter().map(|x| x * x).sum::<f32>().sqrt().max(1.0);
    hash.iter_mut().for_each(|x| *x /= norm);

    hash
}

/// Parse date string to (day_of_week, day_of_month, hour)
fn parse_date(date_str: &str) -> (u8, u8, u8) {
    // Simple parser for YYYY-MM-DD format
    let parts: Vec<&str> = date_str.split('-').collect();
    if parts.len() >= 3 {
        let day: u8 = parts[2].parse().unwrap_or(1);
        let month: u8 = parts[1].parse().unwrap_or(1);
        let year: u16 = parts[0].parse().unwrap_or(2024);

        // Simple day-of-week calculation (Zeller's congruence simplified)
        let dow = ((day as u16 + 13 * (month as u16 + 1) / 5 + year + year / 4) % 7) as u8;

        (dow, day, 12) // Default hour
    } else {
        (0, 1, 12)
    }
}

/// Q-learning update for spending decisions
pub fn update_q_value(
    state: &FinancialLearningState,
    category: &str,
    action: &str, // "under_budget", "at_budget", "over_budget"
    reward: f64,
    learning_rate: f64,
) -> f64 {
    let key = format!("{}|{}", category, action);
    let current_q = state.q_values.get(&key).copied().unwrap_or(0.0);

    // Q-learning update: Q(s,a) = Q(s,a) + α * (r - Q(s,a))
    current_q + learning_rate * (reward - current_q)
}

/// Generate spending recommendation based on learned Q-values
pub fn get_recommendation(
    state: &FinancialLearningState,
    category: &str,
    current_spending: f64,
    budget: f64,
) -> BudgetRecommendation {
    let ratio = current_spending / budget.max(1.0);

    let actions = ["under_budget", "at_budget", "over_budget"];
    let mut best_action = "at_budget";
    let mut best_q = f64::NEG_INFINITY;

    for action in &actions {
        let key = format!("{}|{}", category, action);
        if let Some(&q) = state.q_values.get(&key) {
            if q > best_q {
                best_q = q;
                best_action = action;
            }
        }
    }

    let trend = if ratio < 0.8 {
        "decreasing"
    } else if ratio > 1.2 {
        "increasing"
    } else {
        "stable"
    };

    BudgetRecommendation {
        category: category.to_string(),
        recommended_limit: budget * best_q.max(0.5).min(2.0),
        current_avg: current_spending,
        trend: trend.to_string(),
        confidence: (1.0 - 1.0 / (state.version as f64 + 1.0)).max(0.1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_features() {
        let tx = Transaction {
            transaction_id: "tx123".to_string(),
            account_id: "acc456".to_string(),
            amount: 50.0,
            date: "2024-03-15".to_string(),
            name: "Coffee Shop".to_string(),
            merchant_name: Some("Starbucks".to_string()),
            category: vec!["Food".to_string(), "Coffee".to_string()],
            pending: false,
            payment_channel: "in_store".to_string(),
        };

        let features = extract_features(&tx);
        assert!(features.amount_normalized >= 0.0);
        assert!(features.amount_normalized <= 1.0);
        assert_eq!(features.category_hash.len(), 8);
    }

    #[test]
    fn test_q_learning() {
        let state = FinancialLearningState::default();

        let new_q = update_q_value(&state, "Food", "under_budget", 1.0, 0.1);
        assert!(new_q > 0.0);
    }
}
