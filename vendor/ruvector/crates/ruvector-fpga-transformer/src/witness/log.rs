//! Witness log builder and utilities

use crate::types::{BackendKind, GateDecision, WitnessLog};
use std::time::Instant;

/// Builder for creating witness logs
pub struct WitnessBuilder {
    model_hash: [u8; 32],
    quant_hash: [u8; 32],
    backend: BackendKind,
    start_time: Instant,
    cycles: u32,
    gate_decision: GateDecision,
}

impl WitnessBuilder {
    /// Start building a new witness
    pub fn new(backend: BackendKind) -> Self {
        Self {
            model_hash: [0u8; 32],
            quant_hash: [0u8; 32],
            backend,
            start_time: Instant::now(),
            cycles: 0,
            gate_decision: GateDecision::RanFull,
        }
    }

    /// Set model hash
    pub fn model_hash(mut self, hash: [u8; 32]) -> Self {
        self.model_hash = hash;
        self
    }

    /// Set quantization hash
    pub fn quant_hash(mut self, hash: [u8; 32]) -> Self {
        self.quant_hash = hash;
        self
    }

    /// Set compute cycles
    pub fn cycles(mut self, cycles: u32) -> Self {
        self.cycles = cycles;
        self
    }

    /// Set gate decision
    pub fn gate_decision(mut self, decision: GateDecision) -> Self {
        self.gate_decision = decision;
        self
    }

    /// Build the witness log
    pub fn build(self) -> WitnessLog {
        let latency_ns = self.start_time.elapsed().as_nanos() as u32;

        WitnessLog::new(
            self.model_hash,
            self.quant_hash,
            self.backend,
            self.cycles,
            latency_ns,
            self.gate_decision,
        )
    }
}

impl WitnessLog {
    /// Convert to compact bytes for storage
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(80);

        bytes.extend_from_slice(&self.model_hash);
        bytes.extend_from_slice(&self.quant_hash);
        bytes.push(self.backend as u8);
        bytes.extend_from_slice(&self.cycles.to_le_bytes());
        bytes.extend_from_slice(&self.latency_ns.to_le_bytes());

        // Encode gate decision
        match self.gate_decision {
            GateDecision::RanFull => {
                bytes.push(0);
                bytes.push(0);
            }
            GateDecision::EarlyExit { layer } => {
                bytes.push(1);
                bytes.push(layer);
            }
            GateDecision::Skipped { reason } => {
                bytes.push(2);
                bytes.push(reason as u8);
            }
        }

        bytes
    }

    /// Parse from bytes
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 75 {
            return None;
        }

        let model_hash: [u8; 32] = bytes[0..32].try_into().ok()?;
        let quant_hash: [u8; 32] = bytes[32..64].try_into().ok()?;

        let backend = match bytes[64] {
            0 => BackendKind::FpgaPcie,
            1 => BackendKind::FpgaDaemon,
            2 => BackendKind::WasmSim,
            3 => BackendKind::NativeSim,
            _ => BackendKind::NativeSim,
        };

        let cycles = u32::from_le_bytes(bytes[65..69].try_into().ok()?);
        let latency_ns = u32::from_le_bytes(bytes[69..73].try_into().ok()?);

        let gate_decision = match bytes[73] {
            0 => GateDecision::RanFull,
            1 => GateDecision::EarlyExit { layer: bytes[74] },
            2 => GateDecision::Skipped {
                reason: match bytes[74] {
                    0 => crate::types::SkipReason::LowCoherence,
                    1 => crate::types::SkipReason::PolicyDenied,
                    _ => crate::types::SkipReason::BudgetExceeded,
                },
            },
            _ => GateDecision::RanFull,
        };

        Some(Self {
            model_hash,
            quant_hash,
            backend,
            cycles,
            latency_ns,
            gate_decision,
        })
    }

    /// Get latency in microseconds
    pub fn latency_us(&self) -> f64 {
        self.latency_ns as f64 / 1000.0
    }

    /// Get latency in milliseconds
    pub fn latency_ms(&self) -> f64 {
        self.latency_ns as f64 / 1_000_000.0
    }

    /// Check if this was a successful full inference
    pub fn is_full_inference(&self) -> bool {
        matches!(self.gate_decision, GateDecision::RanFull)
    }

    /// Check if this was an early exit
    pub fn is_early_exit(&self) -> bool {
        matches!(self.gate_decision, GateDecision::EarlyExit { .. })
    }

    /// Check if inference was skipped
    pub fn is_skipped(&self) -> bool {
        matches!(self.gate_decision, GateDecision::Skipped { .. })
    }
}

/// Witness log aggregator for collecting statistics
#[derive(Debug, Default)]
pub struct WitnessAggregator {
    /// Total inferences
    pub count: u64,
    /// Total cycles
    pub total_cycles: u64,
    /// Total latency (ns)
    pub total_latency_ns: u64,
    /// Full inference count
    pub full_count: u64,
    /// Early exit count
    pub early_exit_count: u64,
    /// Skipped count
    pub skipped_count: u64,
    /// Sum of squares for variance calculation
    latency_sq_sum: u128,
}

impl WitnessAggregator {
    /// Create a new aggregator
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a witness to the aggregate
    pub fn add(&mut self, witness: &WitnessLog) {
        self.count += 1;
        self.total_cycles += witness.cycles as u64;
        self.total_latency_ns += witness.latency_ns as u64;
        self.latency_sq_sum += (witness.latency_ns as u128).pow(2);

        match witness.gate_decision {
            GateDecision::RanFull => self.full_count += 1,
            GateDecision::EarlyExit { .. } => self.early_exit_count += 1,
            GateDecision::Skipped { .. } => self.skipped_count += 1,
        }
    }

    /// Get average latency (ns)
    pub fn avg_latency_ns(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.total_latency_ns as f64 / self.count as f64
        }
    }

    /// Get average cycles
    pub fn avg_cycles(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.total_cycles as f64 / self.count as f64
        }
    }

    /// Get latency standard deviation (ns)
    pub fn latency_std_ns(&self) -> f64 {
        if self.count <= 1 {
            return 0.0;
        }

        let mean = self.avg_latency_ns();
        let variance = (self.latency_sq_sum as f64 / self.count as f64) - (mean * mean);
        variance.sqrt()
    }

    /// Get early exit rate
    pub fn early_exit_rate(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.early_exit_count as f64 / self.count as f64
        }
    }

    /// Get skip rate
    pub fn skip_rate(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.skipped_count as f64 / self.count as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_witness_builder() {
        let witness = WitnessBuilder::new(BackendKind::NativeSim)
            .model_hash([1u8; 32])
            .quant_hash([2u8; 32])
            .cycles(1000)
            .gate_decision(GateDecision::RanFull)
            .build();

        assert_eq!(witness.model_hash, [1u8; 32]);
        assert_eq!(witness.backend, BackendKind::NativeSim);
        assert_eq!(witness.cycles, 1000);
    }

    #[test]
    fn test_witness_bytes_roundtrip() {
        let witness = WitnessLog::new(
            [0x42u8; 32],
            [0x24u8; 32],
            BackendKind::FpgaDaemon,
            5000,
            100_000,
            GateDecision::EarlyExit { layer: 4 },
        );

        let bytes = witness.to_bytes();
        let parsed = WitnessLog::from_bytes(&bytes).unwrap();

        assert_eq!(witness.model_hash, parsed.model_hash);
        assert_eq!(witness.quant_hash, parsed.quant_hash);
        assert_eq!(witness.backend, parsed.backend);
        assert_eq!(witness.cycles, parsed.cycles);
        assert_eq!(witness.latency_ns, parsed.latency_ns);
    }

    #[test]
    fn test_witness_aggregator() {
        let mut agg = WitnessAggregator::new();

        for i in 0..10 {
            let mut witness = WitnessLog::empty();
            witness.latency_ns = 1000 * (i + 1);
            witness.cycles = 100 * (i + 1);
            if i < 3 {
                witness.gate_decision = GateDecision::EarlyExit { layer: 2 };
            }
            agg.add(&witness);
        }

        assert_eq!(agg.count, 10);
        assert_eq!(agg.early_exit_count, 3);
        assert!((agg.early_exit_rate() - 0.3).abs() < 0.01);
    }
}
