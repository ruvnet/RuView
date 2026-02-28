//! Production WASM Bindings for Zero-Knowledge Financial Proofs
//!
//! Exposes production-grade Bulletproofs to JavaScript with a safe API.
//!
//! ## Security
//!
//! - All cryptographic operations use audited libraries
//! - Constant-time operations prevent timing attacks
//! - No sensitive data exposed to JavaScript

#![cfg(feature = "wasm")]

use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};

use super::zkproofs_prod::{
    FinancialProver, FinancialVerifier, ZkRangeProof,
    RentalApplicationBundle, VerificationResult,
};

/// Production ZK Financial Prover for browser use
///
/// Uses real Bulletproofs for cryptographically secure range proofs.
#[wasm_bindgen]
pub struct WasmFinancialProver {
    inner: FinancialProver,
}

#[wasm_bindgen]
impl WasmFinancialProver {
    /// Create a new prover
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: FinancialProver::new(),
        }
    }

    /// Set monthly income data (in cents)
    ///
    /// Example: $6,500/month = 650000 cents
    #[wasm_bindgen(js_name = setIncome)]
    pub fn set_income(&mut self, income_json: &str) -> Result<(), JsValue> {
        let income: Vec<u64> = serde_json::from_str(income_json)
            .map_err(|e| JsValue::from_str(&format!("Parse error: {}", e)))?;
        self.inner.set_income(income);
        Ok(())
    }

    /// Set daily balance history (in cents)
    ///
    /// Negative values represent overdrafts.
    #[wasm_bindgen(js_name = setBalances)]
    pub fn set_balances(&mut self, balances_json: &str) -> Result<(), JsValue> {
        let balances: Vec<i64> = serde_json::from_str(balances_json)
            .map_err(|e| JsValue::from_str(&format!("Parse error: {}", e)))?;
        self.inner.set_balances(balances);
        Ok(())
    }

    /// Set expense data for a category (in cents)
    #[wasm_bindgen(js_name = setExpenses)]
    pub fn set_expenses(&mut self, category: &str, expenses_json: &str) -> Result<(), JsValue> {
        let expenses: Vec<u64> = serde_json::from_str(expenses_json)
            .map_err(|e| JsValue::from_str(&format!("Parse error: {}", e)))?;
        self.inner.set_expenses(category, expenses);
        Ok(())
    }

    /// Prove: average income >= threshold (in cents)
    ///
    /// Returns a ZK proof that can be verified without revealing actual income.
    #[wasm_bindgen(js_name = proveIncomeAbove)]
    pub fn prove_income_above(&mut self, threshold_cents: u64) -> Result<JsValue, JsValue> {
        let proof = self.inner.prove_income_above(threshold_cents)
            .map_err(|e| JsValue::from_str(&e))?;

        serde_wasm_bindgen::to_value(&ProofResult::from_proof(proof))
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Prove: income >= multiplier × rent
    ///
    /// Common requirement: income must be 3x rent.
    #[wasm_bindgen(js_name = proveAffordability)]
    pub fn prove_affordability(&mut self, rent_cents: u64, multiplier: u64) -> Result<JsValue, JsValue> {
        let proof = self.inner.prove_affordability(rent_cents, multiplier)
            .map_err(|e| JsValue::from_str(&e))?;

        serde_wasm_bindgen::to_value(&ProofResult::from_proof(proof))
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Prove: no overdrafts in the past N days
    #[wasm_bindgen(js_name = proveNoOverdrafts)]
    pub fn prove_no_overdrafts(&mut self, days: usize) -> Result<JsValue, JsValue> {
        let proof = self.inner.prove_no_overdrafts(days)
            .map_err(|e| JsValue::from_str(&e))?;

        serde_wasm_bindgen::to_value(&ProofResult::from_proof(proof))
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Prove: current savings >= threshold (in cents)
    #[wasm_bindgen(js_name = proveSavingsAbove)]
    pub fn prove_savings_above(&mut self, threshold_cents: u64) -> Result<JsValue, JsValue> {
        let proof = self.inner.prove_savings_above(threshold_cents)
            .map_err(|e| JsValue::from_str(&e))?;

        serde_wasm_bindgen::to_value(&ProofResult::from_proof(proof))
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Prove: average spending in category <= budget (in cents)
    #[wasm_bindgen(js_name = proveBudgetCompliance)]
    pub fn prove_budget_compliance(&mut self, category: &str, budget_cents: u64) -> Result<JsValue, JsValue> {
        let proof = self.inner.prove_budget_compliance(category, budget_cents)
            .map_err(|e| JsValue::from_str(&e))?;

        serde_wasm_bindgen::to_value(&ProofResult::from_proof(proof))
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Create a complete rental application bundle
    ///
    /// Combines income, stability, and optional savings proofs.
    #[wasm_bindgen(js_name = createRentalApplication)]
    pub fn create_rental_application(
        &mut self,
        rent_cents: u64,
        income_multiplier: u64,
        stability_days: usize,
        savings_months: Option<u64>,
    ) -> Result<JsValue, JsValue> {
        let bundle = RentalApplicationBundle::create(
            &mut self.inner,
            rent_cents,
            income_multiplier,
            stability_days,
            savings_months,
        ).map_err(|e| JsValue::from_str(&e))?;

        serde_wasm_bindgen::to_value(&BundleResult::from_bundle(bundle))
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

impl Default for WasmFinancialProver {
    fn default() -> Self {
        Self::new()
    }
}

/// Production ZK Verifier for browser use
#[wasm_bindgen]
pub struct WasmFinancialVerifier;

#[wasm_bindgen]
impl WasmFinancialVerifier {
    /// Create a new verifier
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self
    }

    /// Verify a ZK range proof
    ///
    /// Returns verification result without learning the private value.
    #[wasm_bindgen]
    pub fn verify(&self, proof_json: &str) -> Result<JsValue, JsValue> {
        let proof_result: ProofResult = serde_json::from_str(proof_json)
            .map_err(|e| JsValue::from_str(&format!("Parse error: {}", e)))?;

        let proof = proof_result.to_proof()
            .map_err(|e| JsValue::from_str(&e))?;

        let result = FinancialVerifier::verify(&proof)
            .map_err(|e| JsValue::from_str(&e))?;

        serde_wasm_bindgen::to_value(&VerificationOutput::from_result(result))
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Verify a rental application bundle
    #[wasm_bindgen(js_name = verifyBundle)]
    pub fn verify_bundle(&self, bundle_json: &str) -> Result<JsValue, JsValue> {
        let bundle_result: BundleResult = serde_json::from_str(bundle_json)
            .map_err(|e| JsValue::from_str(&format!("Parse error: {}", e)))?;

        let bundle = bundle_result.to_bundle()
            .map_err(|e| JsValue::from_str(&e))?;

        let valid = bundle.verify()
            .map_err(|e| JsValue::from_str(&e))?;

        serde_wasm_bindgen::to_value(&BundleVerification {
            valid,
            application_id: bundle.application_id,
            created_at: bundle.created_at,
        })
        .map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

impl Default for WasmFinancialVerifier {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// JSON-Serializable Types for JS Interop
// ============================================================================

/// Proof result for JS consumption
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofResult {
    /// Base64-encoded proof bytes
    pub proof_base64: String,
    /// Commitment point (hex)
    pub commitment_hex: String,
    /// Lower bound
    pub min: u64,
    /// Upper bound
    pub max: u64,
    /// Statement
    pub statement: String,
    /// Generated timestamp
    pub generated_at: u64,
    /// Expiration timestamp
    pub expires_at: Option<u64>,
    /// Proof hash (hex)
    pub hash_hex: String,
}

impl ProofResult {
    fn from_proof(proof: ZkRangeProof) -> Self {
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        Self {
            proof_base64: STANDARD.encode(&proof.proof_bytes),
            commitment_hex: hex::encode(proof.commitment.point),
            min: proof.min,
            max: proof.max,
            statement: proof.statement,
            generated_at: proof.metadata.generated_at,
            expires_at: proof.metadata.expires_at,
            hash_hex: hex::encode(proof.metadata.hash),
        }
    }

    fn to_proof(&self) -> Result<ZkRangeProof, String> {
        use super::zkproofs_prod::{PedersenCommitment, ProofMetadata};
        use base64::{Engine as _, engine::general_purpose::STANDARD};

        let proof_bytes = STANDARD.decode(&self.proof_base64)
            .map_err(|e| format!("Invalid base64: {}", e))?;

        let commitment_bytes: [u8; 32] = hex::decode(&self.commitment_hex)
            .map_err(|e| format!("Invalid commitment hex: {}", e))?
            .try_into()
            .map_err(|_| "Invalid commitment length")?;

        let hash_bytes: [u8; 32] = hex::decode(&self.hash_hex)
            .map_err(|e| format!("Invalid hash hex: {}", e))?
            .try_into()
            .map_err(|_| "Invalid hash length")?;

        Ok(ZkRangeProof {
            proof_bytes,
            commitment: PedersenCommitment { point: commitment_bytes },
            min: self.min,
            max: self.max,
            statement: self.statement.clone(),
            metadata: ProofMetadata {
                generated_at: self.generated_at,
                expires_at: self.expires_at,
                version: 1,
                hash: hash_bytes,
            },
        })
    }
}

/// Bundle result for JS consumption
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleResult {
    /// Income proof
    pub income_proof: ProofResult,
    /// Stability proof
    pub stability_proof: ProofResult,
    /// Optional savings proof
    pub savings_proof: Option<ProofResult>,
    /// Application ID
    pub application_id: String,
    /// Created timestamp
    pub created_at: u64,
    /// Bundle hash (hex)
    pub bundle_hash_hex: String,
}

impl BundleResult {
    fn from_bundle(bundle: RentalApplicationBundle) -> Self {
        Self {
            income_proof: ProofResult::from_proof(bundle.income_proof),
            stability_proof: ProofResult::from_proof(bundle.stability_proof),
            savings_proof: bundle.savings_proof.map(ProofResult::from_proof),
            application_id: bundle.application_id,
            created_at: bundle.created_at,
            bundle_hash_hex: hex::encode(bundle.bundle_hash),
        }
    }

    fn to_bundle(&self) -> Result<RentalApplicationBundle, String> {
        let bundle_hash: [u8; 32] = hex::decode(&self.bundle_hash_hex)
            .map_err(|e| format!("Invalid bundle hash: {}", e))?
            .try_into()
            .map_err(|_| "Invalid bundle hash length")?;

        Ok(RentalApplicationBundle {
            income_proof: self.income_proof.to_proof()?,
            stability_proof: self.stability_proof.to_proof()?,
            savings_proof: self.savings_proof.as_ref().map(|p| p.to_proof()).transpose()?,
            application_id: self.application_id.clone(),
            created_at: self.created_at,
            bundle_hash,
        })
    }
}

/// Verification output for JS consumption
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationOutput {
    /// Whether the proof is valid
    pub valid: bool,
    /// The statement that was verified
    pub statement: String,
    /// When verified
    pub verified_at: u64,
    /// Error message if invalid
    pub error: Option<String>,
}

impl VerificationOutput {
    fn from_result(result: super::zkproofs_prod::VerificationResult) -> Self {
        Self {
            valid: result.valid,
            statement: result.statement,
            verified_at: result.verified_at,
            error: result.error,
        }
    }
}

/// Bundle verification result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleVerification {
    pub valid: bool,
    pub application_id: String,
    pub created_at: u64,
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Check if production ZK is available
#[wasm_bindgen(js_name = isProductionZkAvailable)]
pub fn is_production_zk_available() -> bool {
    true
}

/// Get ZK library version info
#[wasm_bindgen(js_name = getZkVersionInfo)]
pub fn get_zk_version_info() -> JsValue {
    let info = serde_json::json!({
        "version": "1.0.0",
        "library": "bulletproofs",
        "curve": "ristretto255",
        "transcript": "merlin",
        "security_level": "128-bit",
        "features": [
            "range_proofs",
            "pedersen_commitments",
            "constant_time_operations",
            "fiat_shamir_transform"
        ]
    });

    serde_wasm_bindgen::to_value(&info).unwrap_or(JsValue::NULL)
}
