//! Adversarial attack simulation and defence testing
//!
//! This module provides:
//! - Attack simulation for security hardening
//! - Red team / blue team scenarios
//! - Defence validation and benchmarking
//! - Chaos engineering for resilience testing

use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

/// Types of adversarial attacks to simulate
#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum AttackType {
    /// Distributed denial of service
    DDoS,
    /// Sybil node creation
    SybilAttack,
    /// Double-spend attempt
    DoubleSpend,
    /// Eclipse attack (isolating nodes)
    Eclipse,
    /// Replay attack (old transactions)
    Replay,
    /// Free-riding (consuming without contributing)
    FreeRiding,
    /// Result manipulation
    ResultTampering,
    /// Byzantine node behavior
    Byzantine,
    /// Timing attack
    TimingAttack,
    /// Fingerprint spoofing
    FingerprintSpoof,
}

/// Attack simulation configuration
#[derive(Clone, Serialize, Deserialize)]
pub struct AttackConfig {
    pub attack_type: AttackType,
    pub intensity: f32,      // 0.0 - 1.0
    pub duration_ms: u64,
    pub target_nodes: Vec<String>,
    pub parameters: HashMap<String, String>,
}

/// Defence mechanism results
#[derive(Clone, Serialize, Deserialize)]
pub struct DefenceResult {
    pub attack_type: AttackType,
    pub detected: bool,
    pub detection_time_ms: u64,
    pub mitigated: bool,
    pub mitigation_time_ms: u64,
    pub damage_prevented: f32,  // 0.0 - 1.0
    pub false_positives: u32,
    pub recommendations: Vec<String>,
}

/// Adversarial testing framework
#[wasm_bindgen]
pub struct AdversarialSimulator {
    /// Attack history
    attacks: Vec<AttackRecord>,
    /// Defence performance metrics
    defence_metrics: DefenceMetrics,
    /// Active simulations
    active_simulations: HashMap<String, AttackConfig>,
    /// Random seed for reproducibility
    seed: u64,
    /// Chaos mode enabled
    chaos_mode: bool,
}

#[derive(Clone)]
struct AttackRecord {
    timestamp: u64,
    attack_type: AttackType,
    success: bool,
    detected: bool,
    notes: String,
}

#[derive(Clone, Default, Serialize, Deserialize)]
struct DefenceMetrics {
    total_attacks: u32,
    detected: u32,
    mitigated: u32,
    false_positives: u32,
    avg_detection_time_ms: f32,
    avg_mitigation_time_ms: f32,
}

#[wasm_bindgen]
impl AdversarialSimulator {
    #[wasm_bindgen(constructor)]
    pub fn new() -> AdversarialSimulator {
        AdversarialSimulator {
            attacks: Vec::new(),
            defence_metrics: DefenceMetrics::default(),
            active_simulations: HashMap::new(),
            seed: js_sys::Date::now() as u64,
            chaos_mode: false,
        }
    }

    /// Enable chaos mode for continuous testing
    #[wasm_bindgen(js_name = enableChaosMode)]
    pub fn enable_chaos_mode(&mut self, enabled: bool) {
        self.chaos_mode = enabled;
    }

    /// Simulate DDoS attack
    #[wasm_bindgen(js_name = simulateDDoS)]
    pub fn simulate_ddos(&mut self, requests_per_second: u32, duration_ms: u64) -> String {
        let attack_id = format!("ddos-{}", self.seed);
        self.seed += 1;

        let config = AttackConfig {
            attack_type: AttackType::DDoS,
            intensity: (requests_per_second as f32 / 10000.0).min(1.0),
            duration_ms,
            target_nodes: vec!["*".to_string()],
            parameters: [
                ("rps".to_string(), requests_per_second.to_string()),
            ].into_iter().collect(),
        };

        self.active_simulations.insert(attack_id.clone(), config);

        // Simulate detection
        let detection_time = self.simulate_detection(AttackType::DDoS, requests_per_second as f32 / 10000.0);

        format!(
            r#"{{"attack_id":"{}","type":"ddos","status":"active","rps":{},"detection_time_ms":{}}}"#,
            attack_id, requests_per_second, detection_time
        )
    }

    /// Simulate Sybil attack
    #[wasm_bindgen(js_name = simulateSybil)]
    pub fn simulate_sybil(&mut self, fake_nodes: u32, same_fingerprint: bool) -> String {
        let attack_id = format!("sybil-{}", self.seed);
        self.seed += 1;

        let intensity = if same_fingerprint { 0.3 } else { 0.7 } * (fake_nodes as f32 / 100.0).min(1.0);

        self.record_attack(AttackType::SybilAttack, intensity < 0.5, intensity > 0.3);

        let detected = same_fingerprint || fake_nodes > 10;
        let blocked = detected && fake_nodes <= 50;

        format!(
            r#"{{"attack_id":"{}","type":"sybil","fake_nodes":{},"same_fingerprint":{},"detected":{},"blocked":{}}}"#,
            attack_id, fake_nodes, same_fingerprint, detected, blocked
        )
    }

    /// Simulate double-spend attempt
    #[wasm_bindgen(js_name = simulateDoubleSpend)]
    pub fn simulate_double_spend(&mut self, amount: u64, concurrent_targets: u32) -> String {
        let attack_id = format!("double-spend-{}", self.seed);
        self.seed += 1;

        // Double-spend detection based on DAG validation
        let detection_probability = 0.95 + 0.049 * (concurrent_targets as f32).ln().min(3.0) / 3.0;
        let detected = self.random() < detection_probability;

        self.record_attack(AttackType::DoubleSpend, !detected, detected);

        let blocked = detected;
        let penalty_applied = detected;

        format!(
            r#"{{"attack_id":"{}","type":"double_spend","amount":{},"targets":{},"detected":{},"blocked":{},"penalty_applied":{}}}"#,
            attack_id, amount, concurrent_targets, detected, blocked, penalty_applied
        )
    }

    /// Simulate free-riding attack
    #[wasm_bindgen(js_name = simulateFreeRiding)]
    pub fn simulate_free_riding(&mut self, consumption_rate: f32, contribution_rate: f32) -> String {
        let attack_id = format!("freerider-{}", self.seed);
        self.seed += 1;

        let ratio = consumption_rate / (contribution_rate + 0.001);
        let detected = ratio > 5.0;
        let throttled = ratio > 2.0;

        self.record_attack(AttackType::FreeRiding, !detected, detected);

        format!(
            r#"{{"attack_id":"{}","type":"free_riding","ratio":{:.2},"detected":{},"throttled":{},"balance_impact":"{}"}}"#,
            attack_id, ratio, detected, throttled,
            if throttled { "limited" } else { "normal" }
        )
    }

    /// Simulate result tampering
    #[wasm_bindgen(js_name = simulateResultTampering)]
    pub fn simulate_result_tampering(&mut self, tamper_percentage: f32) -> String {
        let attack_id = format!("tamper-{}", self.seed);
        self.seed += 1;

        // Spot-check detection
        let spot_check_rate = 0.1;
        let detected = self.random() < spot_check_rate || tamper_percentage > 0.5;

        self.record_attack(AttackType::ResultTampering, !detected, detected);

        let reputation_penalty = if detected { 0.3 } else { 0.0 };
        let stake_slashed = detected && tamper_percentage > 0.2;

        format!(
            r#"{{"attack_id":"{}","type":"result_tampering","tamper_pct":{:.2},"detected":{},"reputation_penalty":{:.2},"stake_slashed":{}}}"#,
            attack_id, tamper_percentage, detected, reputation_penalty, stake_slashed
        )
    }

    /// Simulate Byzantine node behavior
    #[wasm_bindgen(js_name = simulateByzantine)]
    pub fn simulate_byzantine(&mut self, byzantine_nodes: u32, total_nodes: u32) -> String {
        let attack_id = format!("byzantine-{}", self.seed);
        self.seed += 1;

        let byzantine_ratio = byzantine_nodes as f32 / total_nodes as f32;
        let threshold = 1.0 / 3.0;

        let network_compromised = byzantine_ratio > threshold;
        let consensus_maintained = !network_compromised;

        self.record_attack(AttackType::Byzantine, network_compromised, true);

        format!(
            r#"{{"attack_id":"{}","type":"byzantine","byzantine_ratio":{:.3},"threshold":{:.3},"consensus_maintained":{},"network_secure":{}}}"#,
            attack_id, byzantine_ratio, threshold, consensus_maintained, !network_compromised
        )
    }

    /// Run comprehensive security audit
    #[wasm_bindgen(js_name = runSecurityAudit)]
    pub fn run_security_audit(&mut self) -> String {
        let mut results = Vec::new();

        // Test each attack type
        results.push(self.simulate_ddos(1000, 1000));
        results.push(self.simulate_sybil(20, true));
        results.push(self.simulate_double_spend(1000, 3));
        results.push(self.simulate_free_riding(10.0, 1.0));
        results.push(self.simulate_result_tampering(0.1));
        results.push(self.simulate_byzantine(10, 100));

        // Calculate overall score
        let detection_rate = self.defence_metrics.detected as f32 /
            self.defence_metrics.total_attacks.max(1) as f32;
        let mitigation_rate = self.defence_metrics.mitigated as f32 /
            self.defence_metrics.total_attacks.max(1) as f32;

        let security_score = (detection_rate * 0.4 + mitigation_rate * 0.6) * 100.0;

        format!(
            r#"{{"audit_complete":true,"total_tests":{},"detection_rate":{:.2},"mitigation_rate":{:.2},"security_score":{:.1},"grade":"{}"}}"#,
            self.defence_metrics.total_attacks,
            detection_rate,
            mitigation_rate,
            security_score,
            self.grade_score(security_score)
        )
    }

    /// Get defence metrics
    #[wasm_bindgen(js_name = getDefenceMetrics)]
    pub fn get_defence_metrics(&self) -> String {
        format!(
            r#"{{"total_attacks":{},"detected":{},"mitigated":{},"false_positives":{},"avg_detection_ms":{:.2},"avg_mitigation_ms":{:.2}}}"#,
            self.defence_metrics.total_attacks,
            self.defence_metrics.detected,
            self.defence_metrics.mitigated,
            self.defence_metrics.false_positives,
            self.defence_metrics.avg_detection_time_ms,
            self.defence_metrics.avg_mitigation_time_ms
        )
    }

    /// Get recommendations based on testing
    #[wasm_bindgen(js_name = getRecommendations)]
    pub fn get_recommendations(&self) -> String {
        let mut recommendations = Vec::new();

        let detection_rate = self.defence_metrics.detected as f32 /
            self.defence_metrics.total_attacks.max(1) as f32;

        if detection_rate < 0.8 {
            recommendations.push("Increase spot-check frequency");
            recommendations.push("Enhance fingerprint analysis");
        }

        if self.defence_metrics.avg_detection_time_ms > 1000.0 {
            recommendations.push("Optimize detection algorithms");
            recommendations.push("Consider edge-based detection");
        }

        if self.defence_metrics.false_positives > 5 {
            recommendations.push("Tune sensitivity thresholds");
            recommendations.push("Add machine learning refinement");
        }

        let json: Vec<String> = recommendations.iter()
            .map(|r| format!(r#""{}""#, r))
            .collect();

        format!("[{}]", json.join(","))
    }

    /// Generate chaos event
    #[wasm_bindgen(js_name = generateChaosEvent)]
    pub fn generate_chaos_event(&mut self) -> Option<String> {
        if !self.chaos_mode {
            return None;
        }

        let event_type = (self.random() * 10.0) as u32;

        let chaos = match event_type {
            0 => ("network_partition", "Simulated network split"),
            1 => ("node_crash", "Random node failure"),
            2 => ("latency_spike", "Increased network latency"),
            3 => ("memory_pressure", "High memory usage"),
            4 => ("cpu_throttle", "CPU throttling active"),
            5 => ("connection_drop", "Dropped connections"),
            _ => return None,
        };

        Some(format!(
            r#"{{"chaos_event":"{}","description":"{}","duration_ms":{}}}"#,
            chaos.0, chaos.1, (self.random() * 5000.0) as u64 + 1000
        ))
    }

    // Helper functions
    fn random(&mut self) -> f32 {
        // Simple LCG for deterministic testing
        self.seed = self.seed.wrapping_mul(1103515245).wrapping_add(12345);
        ((self.seed >> 16) & 0x7fff) as f32 / 32768.0
    }

    fn simulate_detection(&mut self, attack_type: AttackType, intensity: f32) -> u64 {
        let base_time = match attack_type {
            AttackType::DDoS => 50,
            AttackType::SybilAttack => 200,
            AttackType::DoubleSpend => 10,
            AttackType::Eclipse => 500,
            AttackType::Replay => 20,
            AttackType::FreeRiding => 1000,
            AttackType::ResultTampering => 100,
            AttackType::Byzantine => 300,
            AttackType::TimingAttack => 150,
            AttackType::FingerprintSpoof => 250,
        };

        let variance = (self.random() * 0.5 + 0.75) * (1.0 - intensity * 0.3);
        (base_time as f32 * variance) as u64
    }

    fn record_attack(&mut self, attack_type: AttackType, success: bool, detected: bool) {
        self.attacks.push(AttackRecord {
            timestamp: js_sys::Date::now() as u64,
            attack_type,
            success,
            detected,
            notes: String::new(),
        });

        self.defence_metrics.total_attacks += 1;
        if detected {
            self.defence_metrics.detected += 1;
        }
        if !success {
            self.defence_metrics.mitigated += 1;
        }

        // Update averages
        let count = self.defence_metrics.total_attacks as f32;
        self.defence_metrics.avg_detection_time_ms =
            (self.defence_metrics.avg_detection_time_ms * (count - 1.0) + 100.0) / count;
        self.defence_metrics.avg_mitigation_time_ms =
            (self.defence_metrics.avg_mitigation_time_ms * (count - 1.0) + 150.0) / count;
    }

    fn grade_score(&self, score: f32) -> &'static str {
        match score as u32 {
            95..=100 => "A+",
            90..=94 => "A",
            85..=89 => "B+",
            80..=84 => "B",
            75..=79 => "C+",
            70..=74 => "C",
            65..=69 => "D",
            _ => "F",
        }
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    // Tests requiring WASM environment (uses js_sys::Date)
    #[cfg(target_arch = "wasm32")]
    #[test]
    fn test_security_audit() {
        let mut sim = AdversarialSimulator::new();
        let result = sim.run_security_audit();
        assert!(result.contains("security_score"));
        assert!(result.contains("grade"));
    }

    #[cfg(target_arch = "wasm32")]
    #[test]
    fn test_byzantine_threshold() {
        let mut sim = AdversarialSimulator::new();

        // Under 1/3 - should be safe
        let result = sim.simulate_byzantine(30, 100);
        assert!(result.contains("\"consensus_maintained\":true"));

        // Over 1/3 - should be compromised
        let result = sim.simulate_byzantine(40, 100);
        assert!(result.contains("\"consensus_maintained\":false"));
    }
}
