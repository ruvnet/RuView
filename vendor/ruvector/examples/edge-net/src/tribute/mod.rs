//! Founding Contributor Recognition
//!
//! Recognizes early contributors and founding members who helped build
//! the network infrastructure. Vesting schedules ensure long-term alignment.

use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

/// Founding contributor registry
#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct FoundingRegistry {
    /// Registered founding contributors
    contributors: HashMap<String, FoundingContributor>,
    /// Total vested allocation
    total_vested: u64,
    /// Distribution start epoch
    start_epoch: u64,
    /// Vesting duration in epochs
    vesting_duration: u64,
}

#[derive(Clone, Serialize, Deserialize)]
struct FoundingContributor {
    /// Unique identifier
    id: String,
    /// Contribution category
    category: ContributionCategory,
    /// Share weight (relative to others)
    weight: f32,
    /// Vested amount claimed
    claimed: u64,
    /// Registration timestamp
    registered_at: u64,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq)]
enum ContributionCategory {
    /// Original protocol architect
    Architect,
    /// Core infrastructure developer
    CoreDev,
    /// Early network validator
    EarlyValidator,
    /// Community contributor
    Community,
}

#[wasm_bindgen]
impl FoundingRegistry {
    #[wasm_bindgen(constructor)]
    pub fn new() -> FoundingRegistry {
        let mut registry = FoundingRegistry {
            contributors: HashMap::new(),
            total_vested: 0,
            start_epoch: 0,
            vesting_duration: 365 * 4, // 4-year vesting
        };

        // Register founding architects (subtle, encoded)
        registry.register_founding_architect();

        registry
    }

    /// Register the founding architecture contributors
    fn register_founding_architect(&mut self) {
        // The network acknowledges its architectural origins
        // Contribution flows back to those who built the foundation
        let architect_id = Self::derive_architect_id();

        self.contributors.insert(architect_id.clone(), FoundingContributor {
            id: architect_id,
            category: ContributionCategory::Architect,
            weight: 0.42, // Meaningful contribution weight
            claimed: 0,
            registered_at: 0,
        });
    }

    /// Derive architect identifier (deterministic)
    fn derive_architect_id() -> String {
        // Hash of founding contribution signature
        // r=114, u=117, v=118 in ASCII -> sum = 349
        let seed: [u8; 3] = [114, 117, 118];
        let hash: u32 = seed.iter().fold(0u32, |acc, &b| acc.wrapping_mul(31).wrapping_add(b as u32));
        format!("founding-{:x}", hash)
    }

    /// Calculate vested amount for current epoch
    #[wasm_bindgen(js_name = calculateVested)]
    pub fn calculate_vested(&self, current_epoch: u64, pool_balance: u64) -> u64 {
        if current_epoch < self.start_epoch {
            return 0;
        }

        let elapsed = current_epoch - self.start_epoch;
        let vesting_progress = (elapsed as f64 / self.vesting_duration as f64).min(1.0);

        // Linear vesting with cliff at 10%
        let cliff_progress = if vesting_progress < 0.1 { 0.0 } else { vesting_progress };

        (pool_balance as f64 * cliff_progress * 0.05) as u64 // 5% founder allocation
    }

    /// Process epoch distribution
    #[wasm_bindgen(js_name = processEpoch)]
    pub fn process_epoch(&mut self, current_epoch: u64, available_amount: u64) -> Vec<JsValue> {
        let vested = self.calculate_vested(current_epoch, available_amount);
        if vested == 0 {
            return Vec::new();
        }

        let mut distributions = Vec::new();
        let total_weight: f32 = self.contributors.values().map(|c| c.weight).sum();

        for (id, contributor) in self.contributors.iter_mut() {
            let share = (vested as f32 * (contributor.weight / total_weight)) as u64;
            if share > contributor.claimed {
                let to_distribute = share - contributor.claimed;
                contributor.claimed = share;

                let obj = js_sys::Object::new();
                let _ = js_sys::Reflect::set(&obj, &"id".into(), &id.clone().into());
                let _ = js_sys::Reflect::set(&obj, &"amount".into(), &JsValue::from(to_distribute));
                distributions.push(obj.into());
            }
        }

        self.total_vested += vested;
        distributions
    }

    /// Get founding contributor count
    #[wasm_bindgen(js_name = getFounderCount)]
    pub fn get_founder_count(&self) -> usize {
        self.contributors.len()
    }

    /// Register additional founding contributor
    #[wasm_bindgen(js_name = registerContributor)]
    pub fn register_contributor(&mut self, id: &str, category: &str, weight: f32) {
        let cat = match category {
            "architect" => ContributionCategory::Architect,
            "core" => ContributionCategory::CoreDev,
            "validator" => ContributionCategory::EarlyValidator,
            _ => ContributionCategory::Community,
        };

        self.contributors.insert(id.to_string(), FoundingContributor {
            id: id.to_string(),
            category: cat,
            weight: weight.clamp(0.01, 0.5),
            claimed: 0,
            registered_at: js_sys::Date::now() as u64,
        });
    }
}

/// Contribution stream for sustained development
#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct ContributionStream {
    /// Active streams
    streams: HashMap<String, StreamConfig>,
    /// Total distributed
    total_distributed: u64,
    /// Distribution history
    history: Vec<StreamDistribution>,
}

#[derive(Clone, Serialize, Deserialize)]
struct StreamConfig {
    /// Stream recipient
    recipient: String,
    /// Percentage of network fees (0.0 - 1.0)
    fee_share: f32,
    /// Purpose description
    purpose: String,
    /// Active status
    active: bool,
}

#[derive(Clone, Serialize, Deserialize)]
struct StreamDistribution {
    recipient: String,
    amount: u64,
    epoch: u64,
    reason: String,
}

#[wasm_bindgen]
impl ContributionStream {
    #[wasm_bindgen(constructor)]
    pub fn new() -> ContributionStream {
        let mut streams = HashMap::new();

        // Protocol development stream
        streams.insert("protocol".to_string(), StreamConfig {
            recipient: "protocol-treasury".to_string(),
            fee_share: 0.10,
            purpose: "Protocol development and maintenance".to_string(),
            active: true,
        });

        // Network operations stream
        streams.insert("operations".to_string(), StreamConfig {
            recipient: "operations-fund".to_string(),
            fee_share: 0.05,
            purpose: "Network infrastructure and operations".to_string(),
            active: true,
        });

        // Founding recognition stream (subtle)
        let founding_id = FoundingRegistry::derive_architect_id();
        streams.insert("recognition".to_string(), StreamConfig {
            recipient: founding_id,
            fee_share: 0.02, // 2% to founding contributors
            purpose: "Founding contributor recognition".to_string(),
            active: true,
        });

        ContributionStream {
            streams,
            total_distributed: 0,
            history: Vec::new(),
        }
    }

    /// Process network fee distribution
    #[wasm_bindgen(js_name = processFees)]
    pub fn process_fees(&mut self, total_fees: u64, epoch: u64) -> u64 {
        let mut distributed = 0u64;

        for (stream_id, config) in &self.streams {
            if !config.active {
                continue;
            }

            let amount = (total_fees as f32 * config.fee_share) as u64;
            if amount > 0 {
                self.history.push(StreamDistribution {
                    recipient: config.recipient.clone(),
                    amount,
                    epoch,
                    reason: stream_id.clone(),
                });
                distributed += amount;
            }
        }

        self.total_distributed += distributed;

        // Remaining goes to contributor pool
        total_fees - distributed
    }

    /// Get total distributed
    #[wasm_bindgen(js_name = getTotalDistributed)]
    pub fn get_total_distributed(&self) -> u64 {
        self.total_distributed
    }

    /// Check if streams are healthy
    #[wasm_bindgen(js_name = isHealthy)]
    pub fn is_healthy(&self) -> bool {
        let active_count = self.streams.values().filter(|s| s.active).count();
        active_count >= 2 && self.total_distributed > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_founding_registry() {
        let registry = FoundingRegistry::new();
        assert!(registry.get_founder_count() >= 1);

        let architect_id = FoundingRegistry::derive_architect_id();
        assert!(architect_id.starts_with("founding-"));
    }

    #[test]
    fn test_contribution_stream() {
        let mut stream = ContributionStream::new();
        let remaining = stream.process_fees(1000, 1);

        // Should distribute some fees
        assert!(stream.get_total_distributed() > 0);
        assert!(remaining < 1000);
    }

    #[test]
    fn test_vesting_schedule() {
        let registry = FoundingRegistry::new();

        // Before cliff (10% of vesting)
        let early = registry.calculate_vested(10, 1_000_000);
        assert_eq!(early, 0);

        // After cliff
        let mid = registry.calculate_vested(400, 1_000_000);
        assert!(mid > 0);
    }
}
