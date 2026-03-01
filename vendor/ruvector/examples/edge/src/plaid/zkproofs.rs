//! Zero-Knowledge Financial Proofs
//!
//! Prove financial statements without revealing actual numbers.
//! All proofs are generated in the browser - private data never leaves.
//!
//! # ⚠️ SECURITY WARNING ⚠️
//!
//! **THIS IS A DEMONSTRATION IMPLEMENTATION - NOT PRODUCTION READY**
//!
//! The cryptographic primitives in this module are SIMPLIFIED for educational
//! purposes and API demonstration. They do NOT provide real security:
//!
//! - Custom hash function (not SHA-256)
//! - Simplified Pedersen commitments (not elliptic curve based)
//! - Mock bulletproof verification (does not verify mathematical properties)
//!
//! ## For Production Use
//!
//! Replace with battle-tested cryptographic libraries:
//! ```toml
//! bulletproofs = "4.0"           # Real bulletproofs
//! curve25519-dalek = "4.0"       # Elliptic curve operations
//! merlin = "3.0"                 # Fiat-Shamir transcripts
//! sha2 = "0.10"                  # Cryptographic hash
//! ```
//!
//! ## Supported Proofs (API Demo)
//!
//! - **Range Proofs**: Prove a value is within a range
//! - **Comparison Proofs**: Prove value A > value B
//! - **Aggregate Proofs**: Prove sum/average meets criteria
//! - **History Proofs**: Prove statements about transaction history
//!
//! ## Cryptographic Basis (Production)
//!
//! Real implementation would use Bulletproofs for range proofs (no trusted setup).
//! Pedersen commitments on Ristretto255 curve hide values while allowing verification.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Core Types
// ============================================================================

/// A committed value - hides the actual number
///
/// # Security Note
/// In production, this would be a Ristretto255 point: `C = v·G + r·H`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commitment {
    /// The commitment point (in production: compressed Ristretto255)
    pub point: [u8; 32],
    // NOTE: Blinding factor removed from struct to prevent accidental leakage.
    // Prover must track blindings separately in a secure manner.
}

/// A zero-knowledge proof
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZkProof {
    /// Proof type identifier
    pub proof_type: ProofType,
    /// The actual proof bytes
    pub proof_data: Vec<u8>,
    /// Public inputs (what the verifier needs)
    pub public_inputs: PublicInputs,
    /// Timestamp when proof was generated
    pub generated_at: u64,
    /// Expiration (proofs can be time-limited)
    pub expires_at: Option<u64>,
}

/// Types of proofs we can generate
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProofType {
    /// Prove: value ∈ [min, max]
    Range,
    /// Prove: value_a > value_b (or ≥, <, ≤)
    Comparison,
    /// Prove: income ≥ multiplier × expense
    Affordability,
    /// Prove: all values in set ≥ 0 (no overdrafts)
    NonNegative,
    /// Prove: sum of values ≤ threshold
    SumBound,
    /// Prove: average of values meets criteria
    AverageBound,
    /// Prove: membership in a set (e.g., verified accounts)
    SetMembership,
}

/// Public inputs that verifier sees
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicInputs {
    /// Commitments to hidden values
    pub commitments: Vec<Commitment>,
    /// Public threshold/bound values
    pub bounds: Vec<u64>,
    /// Statement being proven (human readable)
    pub statement: String,
    /// Optional: institution that signed the source data
    pub attestation: Option<Attestation>,
}

/// Attestation from a trusted source (e.g., Plaid)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attestation {
    /// Who attested (e.g., "plaid.com")
    pub issuer: String,
    /// Signature over the commitments
    pub signature: Vec<u8>,
    /// When the attestation was made
    pub timestamp: u64,
}

/// Result of proof verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    pub valid: bool,
    pub statement: String,
    pub verified_at: u64,
    pub error: Option<String>,
}

// ============================================================================
// Pedersen Commitments (Simplified)
// ============================================================================

/// Pedersen commitment scheme
/// C = v*G + r*H where v=value, r=blinding, G,H=generator points
pub struct PedersenCommitment;

impl PedersenCommitment {
    /// Create a commitment to a value
    pub fn commit(value: u64, blinding: &[u8; 32]) -> Commitment {
        // Simplified: In production, use curve25519-dalek
        let mut point = [0u8; 32];

        // Hash(value || blinding) as simplified commitment
        let mut hasher = Sha256::new();
        hasher.update(&value.to_le_bytes());
        hasher.update(blinding);
        let hash = hasher.finalize();
        point.copy_from_slice(&hash[..32]);

        Commitment {
            point,
        }
    }

    /// Generate random blinding factor
    pub fn random_blinding() -> [u8; 32] {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let mut blinding = [0u8; 32];
        rng.fill(&mut blinding);
        blinding
    }

    /// Verify a commitment opens to a value (only prover can do this)
    pub fn verify_opening(commitment: &Commitment, value: u64, blinding: &[u8; 32]) -> bool {
        let expected = Self::commit(value, blinding);
        commitment.point == expected.point
    }
}

// Simple SHA256 for commitments
struct Sha256 {
    data: Vec<u8>,
}

impl Sha256 {
    fn new() -> Self {
        Self { data: Vec::new() }
    }

    fn update(&mut self, data: &[u8]) {
        self.data.extend_from_slice(data);
    }

    fn finalize(self) -> [u8; 32] {
        // Simplified hash - in production use sha2 crate
        let mut result = [0u8; 32];
        for (i, chunk) in self.data.chunks(32).enumerate() {
            for (j, &byte) in chunk.iter().enumerate() {
                result[(i + j) % 32] ^= byte.wrapping_mul((i + j + 1) as u8);
            }
        }
        // Mix more
        for i in 0..32 {
            result[i] = result[i]
                .wrapping_add(result[(i + 7) % 32])
                .wrapping_mul(result[(i + 13) % 32] | 1);
        }
        result
    }
}

// ============================================================================
// Range Proofs (Bulletproofs-style)
// ============================================================================

/// Bulletproof-style range proof
/// Proves: value ∈ [0, 2^n) without revealing value
pub struct RangeProof;

impl RangeProof {
    /// Generate a range proof
    /// Proves: committed_value ∈ [min, max]
    pub fn prove(value: u64, min: u64, max: u64, blinding: &[u8; 32]) -> Result<ZkProof, String> {
        // Validate range
        if value < min || value > max {
            return Err("Value not in range".to_string());
        }

        // Create commitment
        let commitment = PedersenCommitment::commit(value, blinding);

        // Generate proof data (simplified Bulletproof)
        // In production: use bulletproofs crate
        let proof_data = Self::generate_bulletproof(value, min, max, blinding);

        Ok(ZkProof {
            proof_type: ProofType::Range,
            proof_data,
            public_inputs: PublicInputs {
                commitments: vec![commitment],
                bounds: vec![min, max],
                statement: format!("Value is between {} and {}", min, max),
                attestation: None,
            },
            generated_at: current_timestamp(),
            expires_at: Some(current_timestamp() + 86400 * 30), // 30 days
        })
    }

    /// Verify a range proof
    pub fn verify(proof: &ZkProof) -> VerificationResult {
        if proof.proof_type != ProofType::Range {
            return VerificationResult {
                valid: false,
                statement: proof.public_inputs.statement.clone(),
                verified_at: current_timestamp(),
                error: Some("Wrong proof type".to_string()),
            };
        }

        // Verify the bulletproof (simplified)
        let valid = Self::verify_bulletproof(
            &proof.proof_data,
            &proof.public_inputs.commitments[0],
            proof.public_inputs.bounds[0],
            proof.public_inputs.bounds[1],
        );

        VerificationResult {
            valid,
            statement: proof.public_inputs.statement.clone(),
            verified_at: current_timestamp(),
            error: if valid { None } else { Some("Proof verification failed".to_string()) },
        }
    }

    // Simplified bulletproof generation
    fn generate_bulletproof(value: u64, min: u64, max: u64, blinding: &[u8; 32]) -> Vec<u8> {
        let mut proof = Vec::new();

        // Encode shifted value (value - min)
        let shifted = value - min;
        let range = max - min;

        // Number of bits needed
        let bits = (64 - range.leading_zeros()) as usize;

        // Generate bit commitments (simplified)
        for i in 0..bits {
            let bit = (shifted >> i) & 1;
            let bit_blinding = Self::derive_bit_blinding(blinding, i);
            let bit_commitment = PedersenCommitment::commit(bit, &bit_blinding);
            proof.extend_from_slice(&bit_commitment.point);
        }

        // Add challenge response (Fiat-Shamir)
        let challenge = Self::fiat_shamir_challenge(&proof, blinding);
        proof.extend_from_slice(&challenge);

        proof
    }

    // Simplified bulletproof verification
    fn verify_bulletproof(
        proof_data: &[u8],
        commitment: &Commitment,
        min: u64,
        max: u64,
    ) -> bool {
        let range = max - min;
        let bits = (64 - range.leading_zeros()) as usize;

        // Check proof has correct structure
        let expected_len = bits * 32 + 32; // bit commitments + challenge
        if proof_data.len() != expected_len {
            return false;
        }

        // Verify structure (simplified - real bulletproofs do much more)
        // In production: verify inner product argument

        // Check challenge is properly formed
        let challenge_start = bits * 32;
        let _challenge = &proof_data[challenge_start..];

        // Simplified: just check it's not all zeros
        proof_data.iter().any(|&b| b != 0)
    }

    fn derive_bit_blinding(base_blinding: &[u8; 32], bit_index: usize) -> [u8; 32] {
        let mut result = *base_blinding;
        result[0] ^= bit_index as u8;
        result[31] ^= (bit_index >> 8) as u8;
        result
    }

    fn fiat_shamir_challenge(transcript: &[u8], blinding: &[u8; 32]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(transcript);
        hasher.update(blinding);
        hasher.finalize()
    }
}

// ============================================================================
// Financial Proof Builder
// ============================================================================

/// Builder for common financial proofs
pub struct FinancialProofBuilder {
    /// Monthly income values
    income: Vec<u64>,
    /// Monthly expenses by category
    expenses: HashMap<String, Vec<u64>>,
    /// Account balances over time
    balances: Vec<i64>,
    /// Blinding factors (kept secret)
    blindings: HashMap<String, [u8; 32]>,
}

impl FinancialProofBuilder {
    pub fn new() -> Self {
        Self {
            income: Vec::new(),
            expenses: HashMap::new(),
            balances: Vec::new(),
            blindings: HashMap::new(),
        }
    }

    /// Add monthly income data
    pub fn with_income(mut self, monthly_income: Vec<u64>) -> Self {
        self.income = monthly_income;
        self
    }

    /// Add expense category data
    pub fn with_expenses(mut self, category: &str, monthly: Vec<u64>) -> Self {
        self.expenses.insert(category.to_string(), monthly);
        self
    }

    /// Add balance history
    pub fn with_balances(mut self, daily_balances: Vec<i64>) -> Self {
        self.balances = daily_balances;
        self
    }

    // ========================================================================
    // Proof Generation
    // ========================================================================

    /// Prove: income ≥ threshold
    pub fn prove_income_above(&self, threshold: u64) -> Result<ZkProof, String> {
        let avg_income = self.income.iter().sum::<u64>() / self.income.len().max(1) as u64;

        let blinding = self.get_or_create_blinding("income");
        RangeProof::prove(avg_income, threshold, u64::MAX / 2, &blinding)
            .map(|mut p| {
                p.public_inputs.statement = format!(
                    "Average monthly income ≥ ${}",
                    threshold
                );
                p
            })
    }

    /// Prove: income ≥ multiplier × rent (affordability)
    pub fn prove_affordability(&self, rent: u64, multiplier: u64) -> Result<ZkProof, String> {
        let avg_income = self.income.iter().sum::<u64>() / self.income.len().max(1) as u64;
        let required = rent * multiplier;

        if avg_income < required {
            return Err("Income does not meet affordability requirement".to_string());
        }

        let blinding = self.get_or_create_blinding("affordability");

        // Prove income ≥ required
        RangeProof::prove(avg_income, required, u64::MAX / 2, &blinding)
            .map(|mut p| {
                p.proof_type = ProofType::Affordability;
                p.public_inputs.statement = format!(
                    "Income ≥ {}× monthly rent of ${}",
                    multiplier, rent
                );
                p.public_inputs.bounds = vec![rent, multiplier];
                p
            })
    }

    /// Prove: no overdrafts (all balances ≥ 0) for N days
    pub fn prove_no_overdrafts(&self, days: usize) -> Result<ZkProof, String> {
        let relevant_balances = if days < self.balances.len() {
            &self.balances[self.balances.len() - days..]
        } else {
            &self.balances[..]
        };

        // Check all balances are non-negative
        let min_balance = *relevant_balances.iter().min().unwrap_or(&0);
        if min_balance < 0 {
            return Err("Overdraft detected in period".to_string());
        }

        let blinding = self.get_or_create_blinding("no_overdraft");

        // Prove minimum balance ≥ 0
        RangeProof::prove(min_balance as u64, 0, u64::MAX / 2, &blinding)
            .map(|mut p| {
                p.proof_type = ProofType::NonNegative;
                p.public_inputs.statement = format!(
                    "No overdrafts in the past {} days",
                    days
                );
                p.public_inputs.bounds = vec![days as u64, 0];
                p
            })
    }

    /// Prove: savings ≥ threshold
    pub fn prove_savings_above(&self, threshold: u64) -> Result<ZkProof, String> {
        let current_balance = *self.balances.last().unwrap_or(&0);

        if current_balance < threshold as i64 {
            return Err("Savings below threshold".to_string());
        }

        let blinding = self.get_or_create_blinding("savings");

        RangeProof::prove(current_balance as u64, threshold, u64::MAX / 2, &blinding)
            .map(|mut p| {
                p.public_inputs.statement = format!(
                    "Current savings ≥ ${}",
                    threshold
                );
                p
            })
    }

    /// Prove: average spending in category ≤ budget
    pub fn prove_budget_compliance(
        &self,
        category: &str,
        budget: u64,
    ) -> Result<ZkProof, String> {
        let expenses = self.expenses.get(category)
            .ok_or_else(|| format!("No data for category: {}", category))?;

        let avg_spending = expenses.iter().sum::<u64>() / expenses.len().max(1) as u64;

        if avg_spending > budget {
            return Err("Average spending exceeds budget".to_string());
        }

        let blinding = self.get_or_create_blinding(&format!("budget_{}", category));

        // Prove spending ≤ budget (equivalent to: spending ∈ [0, budget])
        RangeProof::prove(avg_spending, 0, budget, &blinding)
            .map(|mut p| {
                p.proof_type = ProofType::SumBound;
                p.public_inputs.statement = format!(
                    "Average {} spending ≤ ${}/month",
                    category, budget
                );
                p
            })
    }

    /// Prove: debt-to-income ratio ≤ threshold%
    pub fn prove_debt_ratio(&self, monthly_debt: u64, max_ratio: u64) -> Result<ZkProof, String> {
        let avg_income = self.income.iter().sum::<u64>() / self.income.len().max(1) as u64;

        // ratio = (debt * 100) / income
        let actual_ratio = (monthly_debt * 100) / avg_income.max(1);

        if actual_ratio > max_ratio {
            return Err("Debt ratio exceeds maximum".to_string());
        }

        let blinding = self.get_or_create_blinding("debt_ratio");

        RangeProof::prove(actual_ratio, 0, max_ratio, &blinding)
            .map(|mut p| {
                p.public_inputs.statement = format!(
                    "Debt-to-income ratio ≤ {}%",
                    max_ratio
                );
                p
            })
    }

    // ========================================================================
    // Helpers
    // ========================================================================

    fn get_or_create_blinding(&self, key: &str) -> [u8; 32] {
        // In real impl, would store and reuse blindings
        // For now, generate deterministically from key
        let mut blinding = [0u8; 32];
        for (i, c) in key.bytes().enumerate() {
            blinding[i % 32] ^= c;
        }
        // Add randomness
        let random = PedersenCommitment::random_blinding();
        for i in 0..32 {
            blinding[i] ^= random[i];
        }
        blinding
    }
}

impl Default for FinancialProofBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Composite Proofs (Multiple Statements)
// ============================================================================

/// A bundle of proofs for rental application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RentalApplicationProof {
    /// Prove income meets requirement
    pub income_proof: ZkProof,
    /// Prove no overdrafts
    pub stability_proof: ZkProof,
    /// Prove savings buffer
    pub savings_proof: Option<ZkProof>,
    /// Application metadata
    pub metadata: ApplicationMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicationMetadata {
    pub applicant_id: String,
    pub property_id: Option<String>,
    pub generated_at: u64,
    pub expires_at: u64,
}

impl RentalApplicationProof {
    /// Create a complete rental application proof bundle
    pub fn create(
        builder: &FinancialProofBuilder,
        rent: u64,
        income_multiplier: u64,
        stability_days: usize,
        savings_months: Option<u64>,
    ) -> Result<Self, String> {
        let income_proof = builder.prove_affordability(rent, income_multiplier)?;
        let stability_proof = builder.prove_no_overdrafts(stability_days)?;

        let savings_proof = if let Some(months) = savings_months {
            Some(builder.prove_savings_above(rent * months)?)
        } else {
            None
        };

        Ok(Self {
            income_proof,
            stability_proof,
            savings_proof,
            metadata: ApplicationMetadata {
                applicant_id: generate_anonymous_id(),
                property_id: None,
                generated_at: current_timestamp(),
                expires_at: current_timestamp() + 86400 * 30, // 30 days
            },
        })
    }

    /// Verify all proofs in the bundle
    pub fn verify(&self) -> Vec<VerificationResult> {
        let mut results = vec![
            RangeProof::verify(&self.income_proof),
            RangeProof::verify(&self.stability_proof),
        ];

        if let Some(ref savings_proof) = self.savings_proof {
            results.push(RangeProof::verify(savings_proof));
        }

        results
    }

    /// Check if application is valid (all proofs pass)
    pub fn is_valid(&self) -> bool {
        self.verify().iter().all(|r| r.valid)
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn generate_anonymous_id() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let mut bytes = [0u8; 16];
    rng.fill(&mut bytes);
    hex::encode(bytes)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_range_proof() {
        let value = 5000u64;
        let blinding = PedersenCommitment::random_blinding();

        let proof = RangeProof::prove(value, 3000, 10000, &blinding).unwrap();
        let result = RangeProof::verify(&proof);

        assert!(result.valid);
    }

    #[test]
    fn test_income_proof() {
        let builder = FinancialProofBuilder::new()
            .with_income(vec![6500, 6500, 6800, 6500]); // ~$6500/month

        // Prove income ≥ $5000
        let proof = builder.prove_income_above(5000).unwrap();
        let result = RangeProof::verify(&proof);

        assert!(result.valid);
        assert!(result.statement.contains("5000"));
    }

    #[test]
    fn test_affordability_proof() {
        let builder = FinancialProofBuilder::new()
            .with_income(vec![6500, 6500, 6500, 6500]);

        // Prove can afford $2000 rent (need 3x = $6000)
        let proof = builder.prove_affordability(2000, 3).unwrap();
        let result = RangeProof::verify(&proof);

        assert!(result.valid);
    }

    #[test]
    fn test_no_overdraft_proof() {
        let builder = FinancialProofBuilder::new()
            .with_balances(vec![1000, 800, 1200, 500, 900, 1100, 1500]);

        let proof = builder.prove_no_overdrafts(7).unwrap();
        let result = RangeProof::verify(&proof);

        assert!(result.valid);
    }

    #[test]
    fn test_rental_application() {
        let builder = FinancialProofBuilder::new()
            .with_income(vec![6500, 6500, 6500, 6500])
            .with_balances(vec![5000, 5200, 4800, 5100, 5300, 5000, 5500]);

        let application = RentalApplicationProof::create(
            &builder,
            2000,  // rent
            3,     // income multiplier
            30,    // stability days
            Some(2), // 2 months savings
        ).unwrap();

        assert!(application.is_valid());
    }
}
