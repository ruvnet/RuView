//! WASM bindings for Zero-Knowledge Financial Proofs
//!
//! Generate and verify ZK proofs entirely in the browser.

#![cfg(feature = "wasm")]

use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};

use super::zkproofs::{
    FinancialProofBuilder, RangeProof, RentalApplicationProof,
    ZkProof, VerificationResult, ProofType,
};

/// WASM-compatible ZK Financial Proof Generator
///
/// All proof generation happens in the browser.
/// Private financial data never leaves the client.
#[wasm_bindgen]
pub struct ZkFinancialProver {
    builder: FinancialProofBuilder,
}

#[wasm_bindgen]
impl ZkFinancialProver {
    /// Create a new prover instance
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            builder: FinancialProofBuilder::new(),
        }
    }

    /// Load income data (array of monthly income in cents)
    #[wasm_bindgen(js_name = loadIncome)]
    pub fn load_income(&mut self, monthly_income: Vec<u64>) {
        self.builder = std::mem::take(&mut self.builder)
            .with_income(monthly_income);
    }

    /// Load expense data for a category
    #[wasm_bindgen(js_name = loadExpenses)]
    pub fn load_expenses(&mut self, category: &str, monthly_expenses: Vec<u64>) {
        self.builder = std::mem::take(&mut self.builder)
            .with_expenses(category, monthly_expenses);
    }

    /// Load balance history (array of daily balances in cents, can be negative)
    #[wasm_bindgen(js_name = loadBalances)]
    pub fn load_balances(&mut self, daily_balances: Vec<i64>) {
        self.builder = std::mem::take(&mut self.builder)
            .with_balances(daily_balances);
    }

    // ========================================================================
    // Proof Generation
    // ========================================================================

    /// Prove: average income ≥ threshold
    ///
    /// Returns serialized ZkProof or error string
    #[wasm_bindgen(js_name = proveIncomeAbove)]
    pub fn prove_income_above(&self, threshold_cents: u64) -> Result<JsValue, JsValue> {
        self.builder.prove_income_above(threshold_cents)
            .map(|proof| serde_wasm_bindgen::to_value(&proof).unwrap())
            .map_err(|e| JsValue::from_str(&e))
    }

    /// Prove: income ≥ multiplier × rent
    ///
    /// Common use: prove income ≥ 3× rent for apartment application
    #[wasm_bindgen(js_name = proveAffordability)]
    pub fn prove_affordability(&self, rent_cents: u64, multiplier: u64) -> Result<JsValue, JsValue> {
        self.builder.prove_affordability(rent_cents, multiplier)
            .map(|proof| serde_wasm_bindgen::to_value(&proof).unwrap())
            .map_err(|e| JsValue::from_str(&e))
    }

    /// Prove: no overdrafts in the past N days
    #[wasm_bindgen(js_name = proveNoOverdrafts)]
    pub fn prove_no_overdrafts(&self, days: usize) -> Result<JsValue, JsValue> {
        self.builder.prove_no_overdrafts(days)
            .map(|proof| serde_wasm_bindgen::to_value(&proof).unwrap())
            .map_err(|e| JsValue::from_str(&e))
    }

    /// Prove: current savings ≥ threshold
    #[wasm_bindgen(js_name = proveSavingsAbove)]
    pub fn prove_savings_above(&self, threshold_cents: u64) -> Result<JsValue, JsValue> {
        self.builder.prove_savings_above(threshold_cents)
            .map(|proof| serde_wasm_bindgen::to_value(&proof).unwrap())
            .map_err(|e| JsValue::from_str(&e))
    }

    /// Prove: average spending in category ≤ budget
    #[wasm_bindgen(js_name = proveBudgetCompliance)]
    pub fn prove_budget_compliance(&self, category: &str, budget_cents: u64) -> Result<JsValue, JsValue> {
        self.builder.prove_budget_compliance(category, budget_cents)
            .map(|proof| serde_wasm_bindgen::to_value(&proof).unwrap())
            .map_err(|e| JsValue::from_str(&e))
    }

    /// Prove: debt-to-income ratio ≤ max_ratio%
    #[wasm_bindgen(js_name = proveDebtRatio)]
    pub fn prove_debt_ratio(&self, monthly_debt_cents: u64, max_ratio_percent: u64) -> Result<JsValue, JsValue> {
        self.builder.prove_debt_ratio(monthly_debt_cents, max_ratio_percent)
            .map(|proof| serde_wasm_bindgen::to_value(&proof).unwrap())
            .map_err(|e| JsValue::from_str(&e))
    }

    // ========================================================================
    // Composite Proofs
    // ========================================================================

    /// Generate complete rental application proof bundle
    ///
    /// Includes: income proof, stability proof, optional savings proof
    #[wasm_bindgen(js_name = createRentalApplication)]
    pub fn create_rental_application(
        &self,
        rent_cents: u64,
        income_multiplier: u64,
        stability_days: usize,
        savings_months: Option<u64>,
    ) -> Result<JsValue, JsValue> {
        RentalApplicationProof::create(
            &self.builder,
            rent_cents,
            income_multiplier,
            stability_days,
            savings_months,
        )
            .map(|proof| serde_wasm_bindgen::to_value(&proof).unwrap())
            .map_err(|e| JsValue::from_str(&e))
    }
}

impl Default for ZkFinancialProver {
    fn default() -> Self {
        Self::new()
    }
}

/// WASM-compatible ZK Proof Verifier
///
/// Can verify proofs without knowing the private values
#[wasm_bindgen]
pub struct ZkProofVerifier;

#[wasm_bindgen]
impl ZkProofVerifier {
    /// Verify a single ZK proof
    ///
    /// Returns verification result with validity and statement
    #[wasm_bindgen]
    pub fn verify(proof_json: &str) -> Result<JsValue, JsValue> {
        let proof: ZkProof = serde_json::from_str(proof_json)
            .map_err(|e| JsValue::from_str(&format!("Invalid proof: {}", e)))?;

        let result = RangeProof::verify(&proof);

        serde_wasm_bindgen::to_value(&result)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Verify a rental application proof bundle
    #[wasm_bindgen(js_name = verifyRentalApplication)]
    pub fn verify_rental_application(application_json: &str) -> Result<JsValue, JsValue> {
        let application: RentalApplicationProof = serde_json::from_str(application_json)
            .map_err(|e| JsValue::from_str(&format!("Invalid application: {}", e)))?;

        let results = application.verify();
        let is_valid = application.is_valid();

        let summary = VerificationSummary {
            all_valid: is_valid,
            results,
        };

        serde_wasm_bindgen::to_value(&summary)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get human-readable statement from proof
    #[wasm_bindgen(js_name = getStatement)]
    pub fn get_statement(proof_json: &str) -> Result<String, JsValue> {
        let proof: ZkProof = serde_json::from_str(proof_json)
            .map_err(|e| JsValue::from_str(&format!("Invalid proof: {}", e)))?;

        Ok(proof.public_inputs.statement)
    }

    /// Check if proof is expired
    #[wasm_bindgen(js_name = isExpired)]
    pub fn is_expired(proof_json: &str) -> Result<bool, JsValue> {
        let proof: ZkProof = serde_json::from_str(proof_json)
            .map_err(|e| JsValue::from_str(&format!("Invalid proof: {}", e)))?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Ok(proof.expires_at.map(|exp| now > exp).unwrap_or(false))
    }
}

#[derive(Serialize, Deserialize)]
struct VerificationSummary {
    all_valid: bool,
    results: Vec<VerificationResult>,
}

/// Utility functions for ZK proofs
#[wasm_bindgen]
pub struct ZkUtils;

#[wasm_bindgen]
impl ZkUtils {
    /// Convert dollars to cents (proof system uses cents for precision)
    #[wasm_bindgen(js_name = dollarsToCents)]
    pub fn dollars_to_cents(dollars: f64) -> u64 {
        (dollars * 100.0).round() as u64
    }

    /// Convert cents to dollars
    #[wasm_bindgen(js_name = centsToDollars)]
    pub fn cents_to_dollars(cents: u64) -> f64 {
        cents as f64 / 100.0
    }

    /// Generate a shareable proof URL (base64 encoded)
    #[wasm_bindgen(js_name = proofToUrl)]
    pub fn proof_to_url(proof_json: &str, base_url: &str) -> String {
        let encoded = base64_encode(proof_json.as_bytes());
        format!("{}?proof={}", base_url, encoded)
    }

    /// Extract proof from URL parameter
    #[wasm_bindgen(js_name = proofFromUrl)]
    pub fn proof_from_url(encoded: &str) -> Result<String, JsValue> {
        let decoded = base64_decode(encoded)
            .map_err(|e| JsValue::from_str(&format!("Invalid encoding: {}", e)))?;

        String::from_utf8(decoded)
            .map_err(|e| JsValue::from_str(&format!("Invalid UTF-8: {}", e)))
    }
}

// Simple base64 encoding (no external deps)
fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut result = String::new();

    for chunk in data.chunks(3) {
        let mut n = (chunk[0] as u32) << 16;
        if chunk.len() > 1 {
            n |= (chunk[1] as u32) << 8;
        }
        if chunk.len() > 2 {
            n |= chunk[2] as u32;
        }

        result.push(ALPHABET[(n >> 18) as usize & 0x3F] as char);
        result.push(ALPHABET[(n >> 12) as usize & 0x3F] as char);

        if chunk.len() > 1 {
            result.push(ALPHABET[(n >> 6) as usize & 0x3F] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(ALPHABET[n as usize & 0x3F] as char);
        } else {
            result.push('=');
        }
    }

    result
}

fn base64_decode(data: &str) -> Result<Vec<u8>, &'static str> {
    const DECODE: [i8; 128] = [
        -1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,
        -1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,
        -1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,62,-1,-1,-1,63,
        52,53,54,55,56,57,58,59,60,61,-1,-1,-1,-1,-1,-1,
        -1, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9,10,11,12,13,14,
        15,16,17,18,19,20,21,22,23,24,25,-1,-1,-1,-1,-1,
        -1,26,27,28,29,30,31,32,33,34,35,36,37,38,39,40,
        41,42,43,44,45,46,47,48,49,50,51,-1,-1,-1,-1,-1,
    ];

    let mut result = Vec::new();
    let bytes: Vec<u8> = data.bytes().filter(|&b| b != b'=').collect();

    for chunk in bytes.chunks(4) {
        if chunk.len() < 2 {
            break;
        }

        let mut n = 0u32;
        for (i, &b) in chunk.iter().enumerate() {
            if b >= 128 || DECODE[b as usize] < 0 {
                return Err("Invalid base64 character");
            }
            n |= (DECODE[b as usize] as u32) << (18 - i * 6);
        }

        result.push((n >> 16) as u8);
        if chunk.len() > 2 {
            result.push((n >> 8) as u8);
        }
        if chunk.len() > 3 {
            result.push(n as u8);
        }
    }

    Ok(result)
}
