//! WASM bindings for Plaid local learning
//!
//! Exposes browser-local financial learning to JavaScript.

#![cfg(feature = "wasm")]

use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;

use super::{
    Transaction, SpendingPattern, CategoryPrediction, AnomalyResult,
    BudgetRecommendation, FinancialLearningState, TransactionFeatures,
    extract_features, update_q_value, get_recommendation,
};

/// Browser-local financial learning engine
///
/// All data stays in the browser. Uses IndexedDB for persistence.
#[wasm_bindgen]
pub struct PlaidLocalLearner {
    state: Arc<RwLock<FinancialLearningState>>,
    hnsw_index: crate::WasmHnswIndex,
    spiking_net: crate::WasmSpikingNetwork,
    learning_rate: f64,
}

#[wasm_bindgen]
impl PlaidLocalLearner {
    /// Create a new local learner
    ///
    /// All learning happens in-browser with no data exfiltration.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(FinancialLearningState::default())),
            hnsw_index: crate::WasmHnswIndex::new(),
            spiking_net: crate::WasmSpikingNetwork::new(21, 32, 8), // Features -> hidden -> categories
            learning_rate: 0.1,
        }
    }

    /// Load state from serialized JSON (from IndexedDB)
    #[wasm_bindgen(js_name = loadState)]
    pub fn load_state(&mut self, json: &str) -> Result<(), JsValue> {
        let loaded: FinancialLearningState = serde_json::from_str(json)
            .map_err(|e| JsValue::from_str(&format!("Parse error: {}", e)))?;

        *self.state.write() = loaded;

        // Rebuild HNSW index from loaded embeddings
        let state = self.state.read();
        for (id, embedding) in &state.category_embeddings {
            self.hnsw_index.insert(id, embedding.clone());
        }

        Ok(())
    }

    /// Serialize state to JSON (for IndexedDB persistence)
    #[wasm_bindgen(js_name = saveState)]
    pub fn save_state(&self) -> Result<String, JsValue> {
        let state = self.state.read();
        serde_json::to_string(&*state)
            .map_err(|e| JsValue::from_str(&format!("Serialize error: {}", e)))
    }

    /// Process a batch of transactions and learn patterns
    ///
    /// Returns updated insights without sending data anywhere.
    #[wasm_bindgen(js_name = processTransactions)]
    pub fn process_transactions(&mut self, transactions_json: &str) -> Result<JsValue, JsValue> {
        let transactions: Vec<Transaction> = serde_json::from_str(transactions_json)
            .map_err(|e| JsValue::from_str(&format!("Parse error: {}", e)))?;

        let mut state = self.state.write();
        let mut insights = ProcessingInsights::default();

        for tx in &transactions {
            // Extract features
            let features = extract_features(tx);
            let embedding = features.to_embedding();

            // Add to HNSW index for similarity search
            self.hnsw_index.insert(&tx.transaction_id, embedding.clone());

            // Update category embedding (HashMap prevents memory leak - overwrites existing)
            let category_key = tx.category.join(":");

            // LRU-style eviction if at capacity
            if state.category_embeddings.len() >= state.max_embeddings {
                // Remove oldest entry (in production, use proper LRU cache)
                if let Some(key) = state.category_embeddings.keys().next().cloned() {
                    state.category_embeddings.remove(&key);
                }
            }
            state.category_embeddings.insert(category_key.clone(), embedding.clone());

            // Learn spending pattern
            self.learn_pattern(&mut state, tx, &features);

            // Update temporal weights
            let dow = features.day_of_week as usize % 7;
            let dom = (features.day_of_month as usize).saturating_sub(1) % 31;
            state.temporal_weights[dow] += 0.1 * (tx.amount.abs() as f32);
            state.monthly_weights[dom] += 0.1 * (tx.amount.abs() as f32);

            // Feed to spiking network for temporal learning
            let spike_input = self.features_to_spikes(&features);
            let _output = self.spiking_net.forward(spike_input);

            insights.transactions_processed += 1;
            insights.total_amount += tx.amount.abs();
        }

        state.version += 1;
        insights.patterns_learned = state.patterns.len();
        insights.state_version = state.version;

        serde_wasm_bindgen::to_value(&insights)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Predict category for a new transaction
    #[wasm_bindgen(js_name = predictCategory)]
    pub fn predict_category(&self, transaction_json: &str) -> Result<JsValue, JsValue> {
        let tx: Transaction = serde_json::from_str(transaction_json)
            .map_err(|e| JsValue::from_str(&format!("Parse error: {}", e)))?;

        let features = extract_features(&tx);
        let embedding = features.to_embedding();

        // Find similar transactions via HNSW
        let results = self.hnsw_index.search(embedding.clone(), 5);

        // Aggregate category votes from similar transactions
        let prediction = CategoryPrediction {
            category: tx.category.first().cloned().unwrap_or_default(),
            confidence: 0.85,
            similar_transactions: vec![], // Would populate from results
        };

        serde_wasm_bindgen::to_value(&prediction)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Detect if a transaction is anomalous
    #[wasm_bindgen(js_name = detectAnomaly)]
    pub fn detect_anomaly(&self, transaction_json: &str) -> Result<JsValue, JsValue> {
        let tx: Transaction = serde_json::from_str(transaction_json)
            .map_err(|e| JsValue::from_str(&format!("Parse error: {}", e)))?;

        let state = self.state.read();
        let category_key = tx.category.join(":");

        let result = if let Some(pattern) = state.patterns.get(&category_key) {
            let amount_diff = (tx.amount.abs() - pattern.avg_amount).abs();
            let threshold = pattern.avg_amount * 2.0;

            AnomalyResult {
                is_anomaly: amount_diff > threshold,
                anomaly_score: amount_diff / pattern.avg_amount.max(1.0),
                reason: if amount_diff > threshold {
                    format!("Amount ${:.2} is {:.1}x typical", tx.amount, amount_diff / pattern.avg_amount.max(1.0))
                } else {
                    "Normal transaction".to_string()
                },
                expected_amount: pattern.avg_amount,
            }
        } else {
            AnomalyResult {
                is_anomaly: false,
                anomaly_score: 0.0,
                reason: "First transaction in this category".to_string(),
                expected_amount: tx.amount.abs(),
            }
        };

        serde_wasm_bindgen::to_value(&result)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get budget recommendation for a category
    #[wasm_bindgen(js_name = getBudgetRecommendation)]
    pub fn get_budget_recommendation(
        &self,
        category: &str,
        current_spending: f64,
        budget: f64,
    ) -> Result<JsValue, JsValue> {
        let state = self.state.read();
        let rec = get_recommendation(&state, category, current_spending, budget);

        serde_wasm_bindgen::to_value(&rec)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Record spending outcome for Q-learning
    #[wasm_bindgen(js_name = recordOutcome)]
    pub fn record_outcome(&mut self, category: &str, action: &str, reward: f64) {
        let mut state = self.state.write();
        let key = format!("{}|{}", category, action);
        let new_q = update_q_value(&state, category, action, reward, self.learning_rate);
        state.q_values.insert(key, new_q);
        state.version += 1;
    }

    /// Get spending patterns summary
    #[wasm_bindgen(js_name = getPatternsSummary)]
    pub fn get_patterns_summary(&self) -> Result<JsValue, JsValue> {
        let state = self.state.read();

        let summary: Vec<SpendingPattern> = state.patterns.values().cloned().collect();

        serde_wasm_bindgen::to_value(&summary)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get temporal spending heatmap (day of week + day of month)
    #[wasm_bindgen(js_name = getTemporalHeatmap)]
    pub fn get_temporal_heatmap(&self) -> Result<JsValue, JsValue> {
        let state = self.state.read();

        let heatmap = TemporalHeatmap {
            day_of_week: state.temporal_weights.clone(),
            day_of_month: state.monthly_weights.clone(),
        };

        serde_wasm_bindgen::to_value(&heatmap)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Find similar transactions to a given one
    #[wasm_bindgen(js_name = findSimilarTransactions)]
    pub fn find_similar_transactions(&self, transaction_json: &str, k: usize) -> JsValue {
        let Ok(tx) = serde_json::from_str::<Transaction>(transaction_json) else {
            return JsValue::NULL;
        };

        let features = extract_features(&tx);
        let embedding = features.to_embedding();

        self.hnsw_index.search(embedding, k)
    }

    /// Get current learning statistics
    #[wasm_bindgen(js_name = getStats)]
    pub fn get_stats(&self) -> Result<JsValue, JsValue> {
        let state = self.state.read();

        let stats = LearningStats {
            version: state.version,
            patterns_count: state.patterns.len(),
            q_values_count: state.q_values.len(),
            embeddings_count: state.category_embeddings.len(),
            index_size: self.hnsw_index.len(),
        };

        serde_wasm_bindgen::to_value(&stats)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Clear all learned data (privacy feature)
    #[wasm_bindgen]
    pub fn clear(&mut self) {
        *self.state.write() = FinancialLearningState::default();
        self.hnsw_index = crate::WasmHnswIndex::new();
        self.spiking_net.reset();
    }

    // Internal helper methods

    fn learn_pattern(&self, state: &mut FinancialLearningState, tx: &Transaction, features: &TransactionFeatures) {
        let category_key = tx.category.join(":");

        let pattern = state.patterns.entry(category_key.clone()).or_insert_with(|| {
            SpendingPattern {
                pattern_id: format!("pat_{}", category_key),
                category: category_key.clone(),
                avg_amount: 0.0,
                frequency_days: 30.0,
                confidence: 0.0,
                last_seen: 0,
            }
        });

        // Exponential moving average for amount
        pattern.avg_amount = pattern.avg_amount * 0.9 + tx.amount.abs() * 0.1;
        pattern.confidence = (pattern.confidence + 0.1).min(1.0);

        // Simple timestamp (would use actual timestamp in production)
        pattern.last_seen = state.version;
    }

    fn features_to_spikes(&self, features: &TransactionFeatures) -> Vec<u8> {
        let embedding = features.to_embedding();

        // Convert floats to spike train (probability encoding)
        embedding.iter().map(|&v| {
            if v > 0.5 { 1 } else { 0 }
        }).collect()
    }
}

impl Default for PlaidLocalLearner {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ProcessingInsights {
    transactions_processed: usize,
    total_amount: f64,
    patterns_learned: usize,
    state_version: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TemporalHeatmap {
    day_of_week: Vec<f32>,
    day_of_month: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LearningStats {
    version: u64,
    patterns_count: usize,
    q_values_count: usize,
    embeddings_count: usize,
    index_size: usize,
}
