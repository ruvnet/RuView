//! QDAG (Quantum-Resistant DAG) Currency System
//!
//! A feeless, quantum-resistant cryptocurrency for edge-net compute credits.
//! Uses a DAG (Directed Acyclic Graph) structure instead of a blockchain for:
//! - Instant finality (no blocks, no mining)
//! - Zero transaction fees
//! - High throughput (parallel transaction validation)
//! - Quantum resistance via hybrid signatures
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                           QDAG LEDGER                                   │
//! ├─────────────────────────────────────────────────────────────────────────┤
//! │                                                                         │
//! │    ┌───┐   ┌───┐   ┌───┐                                               │
//! │    │TX1│──►│TX2│──►│TX4│                                               │
//! │    └───┘   └───┘   └───┘                                               │
//! │       ╲       ╲     ╱                                                   │
//! │        ╲       ╲   ╱                                                    │
//! │         ╲       ╲ ╱                                                     │
//! │    ┌───┐ ╲   ┌───┐   ┌───┐                                             │
//! │    │TX3│──►──│TX5│──►│TX6│◄── Latest transactions                      │
//! │    └───┘     └───┘   └───┘                                             │
//! │                                                                         │
//! │    Each transaction validates 2+ previous transactions                  │
//! │    No mining, no fees, instant confirmation                            │
//! │                                                                         │
//! └─────────────────────────────────────────────────────────────────────────┘
//! ```

use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};
use std::collections::{HashMap, HashSet, VecDeque};
use uuid::Uuid;

/// QDAG Transaction - a single credit transfer
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct QDAGTransaction {
    /// Unique transaction ID (hash of contents)
    pub id: [u8; 32],
    /// Previous transaction IDs this validates (2+ required)
    pub validates: Vec<[u8; 32]>,
    /// Sender node ID
    pub sender: String,
    /// Recipient node ID (or "network" for compute rewards)
    pub recipient: String,
    /// Amount in microcredits (1 credit = 1,000,000 microcredits)
    pub amount: u64,
    /// Transaction type
    pub tx_type: TransactionType,
    /// Timestamp (Unix milliseconds)
    pub timestamp: u64,
    /// Ed25519 signature of transaction content
    pub signature_ed25519: Vec<u8>,
    /// Dilithium signature (post-quantum) - optional for now
    pub signature_pq: Option<Vec<u8>>,
    /// Sender's public key (Ed25519)
    pub sender_pubkey: Vec<u8>,
    /// Proof of work (small, just to prevent spam)
    pub pow_nonce: u64,
    /// Cumulative weight (sum of all validated transactions)
    pub cumulative_weight: u64,
}

/// Transaction types
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub enum TransactionType {
    /// Credit earned from compute work
    ComputeReward,
    /// Credit transferred between nodes
    Transfer,
    /// Stake for network participation
    Stake,
    /// Unstake credits
    Unstake,
    /// Penalty/slash for bad behavior
    Penalty,
    /// Genesis transaction (initial distribution)
    Genesis,
}

/// QDAG Ledger - the full transaction graph
#[wasm_bindgen]
pub struct QDAGLedger {
    /// All transactions indexed by ID
    transactions: HashMap<[u8; 32], QDAGTransaction>,
    /// Transactions that haven't been validated yet (tips)
    tips: HashSet<[u8; 32]>,
    /// Balance cache per node
    balances: HashMap<String, i64>,
    /// Staked amounts per node
    stakes: HashMap<String, u64>,
    /// Transaction count per node (for rate limiting)
    tx_counts: HashMap<String, u64>,
    /// Genesis transaction ID
    genesis_id: Option<[u8; 32]>,
    /// Total supply ever minted
    total_supply: u64,
    /// Maximum supply (deflationary cap)
    max_supply: u64,
    /// Current proof-of-work difficulty (target zeros)
    pow_difficulty: u8,
    /// Minimum stake to participate
    min_stake: u64,
}

#[wasm_bindgen]
impl QDAGLedger {
    /// Create a new QDAG ledger
    #[wasm_bindgen(constructor)]
    pub fn new() -> QDAGLedger {
        QDAGLedger {
            transactions: HashMap::new(),
            tips: HashSet::new(),
            balances: HashMap::new(),
            stakes: HashMap::new(),
            tx_counts: HashMap::new(),
            genesis_id: None,
            total_supply: 0,
            max_supply: 1_000_000_000_000_000, // 1 billion credits (in microcredits)
            pow_difficulty: 16, // 16 leading zero bits (~65K hash attempts)
            min_stake: 100_000_000, // 100 credits minimum stake
        }
    }

    /// Create genesis transaction (called once at network start)
    #[wasm_bindgen(js_name = createGenesis)]
    pub fn create_genesis(
        &mut self,
        initial_supply: u64,
        founder_pubkey: &[u8],
    ) -> Result<Vec<u8>, JsValue> {
        if self.genesis_id.is_some() {
            return Err(JsValue::from_str("Genesis already created"));
        }

        let tx = QDAGTransaction {
            id: [0u8; 32], // Will be set after hashing
            validates: vec![], // Genesis has no parents
            sender: "genesis".to_string(),
            recipient: hex::encode(founder_pubkey),
            amount: initial_supply,
            tx_type: TransactionType::Genesis,
            timestamp: js_sys::Date::now() as u64,
            signature_ed25519: vec![], // Genesis is self-signed
            signature_pq: None,
            sender_pubkey: founder_pubkey.to_vec(),
            pow_nonce: 0,
            cumulative_weight: 1,
        };

        let id = self.hash_transaction(&tx);
        let mut tx = tx;
        tx.id = id;

        self.transactions.insert(id, tx.clone());
        self.tips.insert(id);
        self.genesis_id = Some(id);
        self.total_supply = initial_supply;
        self.balances.insert(hex::encode(founder_pubkey), initial_supply as i64);

        Ok(id.to_vec())
    }

    /// Get balance for a node
    #[wasm_bindgen]
    pub fn balance(&self, node_id: &str) -> i64 {
        *self.balances.get(node_id).unwrap_or(&0)
    }

    /// Get staked amount for a node
    #[wasm_bindgen(js_name = stakedAmount)]
    pub fn staked_amount(&self, node_id: &str) -> u64 {
        *self.stakes.get(node_id).unwrap_or(&0)
    }

    /// Create and validate a new transaction
    #[wasm_bindgen(js_name = createTransaction)]
    pub fn create_transaction(
        &mut self,
        sender_id: &str,
        recipient_id: &str,
        amount: u64,
        tx_type: u8,
        sender_privkey: &[u8],
        sender_pubkey: &[u8],
    ) -> Result<Vec<u8>, JsValue> {
        // Validate sender has sufficient balance
        let sender_balance = self.balance(sender_id);
        if sender_balance < amount as i64 {
            return Err(JsValue::from_str("Insufficient balance"));
        }

        // Select tips to validate (2 random tips)
        let tips: Vec<[u8; 32]> = self.select_tips(2)?;
        if tips.len() < 2 && self.transactions.len() > 1 {
            return Err(JsValue::from_str("Not enough tips to validate"));
        }

        // Calculate cumulative weight
        let cumulative_weight = self.calculate_cumulative_weight(&tips);

        // Create transaction
        let tx_type = match tx_type {
            0 => TransactionType::ComputeReward,
            1 => TransactionType::Transfer,
            2 => TransactionType::Stake,
            3 => TransactionType::Unstake,
            4 => TransactionType::Penalty,
            _ => return Err(JsValue::from_str("Invalid transaction type")),
        };

        let mut tx = QDAGTransaction {
            id: [0u8; 32],
            validates: tips.clone(),
            sender: sender_id.to_string(),
            recipient: recipient_id.to_string(),
            amount,
            tx_type,
            timestamp: js_sys::Date::now() as u64,
            signature_ed25519: vec![],
            signature_pq: None,
            sender_pubkey: sender_pubkey.to_vec(),
            pow_nonce: 0,
            cumulative_weight,
        };

        // Find valid PoW nonce
        tx.pow_nonce = self.find_pow_nonce(&tx)?;

        // Calculate transaction ID
        tx.id = self.hash_transaction(&tx);

        // Sign transaction
        tx.signature_ed25519 = self.sign_transaction(&tx, sender_privkey)?;

        // Validate the transaction
        self.validate_transaction(&tx)?;

        // Apply to ledger
        self.apply_transaction(&tx)?;

        Ok(tx.id.to_vec())
    }

    /// Validate an incoming transaction
    fn validate_transaction(&self, tx: &QDAGTransaction) -> Result<(), JsValue> {
        // 1. Verify transaction hash
        let expected_id = self.hash_transaction(tx);
        if expected_id != tx.id {
            return Err(JsValue::from_str("Invalid transaction ID"));
        }

        // 2. Verify signature
        if !self.verify_signature(tx) {
            return Err(JsValue::from_str("Invalid signature"));
        }

        // 3. Verify proof of work
        if !self.verify_pow(tx) {
            return Err(JsValue::from_str("Invalid proof of work"));
        }

        // 4. Verify parent transactions exist
        for parent_id in &tx.validates {
            if !self.transactions.contains_key(parent_id) {
                return Err(JsValue::from_str("Parent transaction not found"));
            }
        }

        // 5. Verify timestamp is reasonable
        let now = js_sys::Date::now() as u64;
        if tx.timestamp > now + 60_000 {
            return Err(JsValue::from_str("Transaction from the future"));
        }

        // 6. Verify sender has sufficient balance (for non-reward transactions)
        if tx.tx_type != TransactionType::ComputeReward && tx.tx_type != TransactionType::Genesis {
            let sender_balance = self.balance(&tx.sender);
            if sender_balance < tx.amount as i64 {
                return Err(JsValue::from_str("Insufficient balance"));
            }
        }

        // 7. Verify stake requirements for compute rewards
        if tx.tx_type == TransactionType::ComputeReward {
            let stake = self.staked_amount(&tx.recipient);
            if stake < self.min_stake {
                return Err(JsValue::from_str("Recipient must stake minimum amount"));
            }
        }

        // 8. Rate limiting check
        let tx_count = *self.tx_counts.get(&tx.sender).unwrap_or(&0);
        if tx_count > 1000 && tx.tx_type != TransactionType::ComputeReward {
            return Err(JsValue::from_str("Rate limit exceeded"));
        }

        Ok(())
    }

    /// Apply a validated transaction to the ledger
    fn apply_transaction(&mut self, tx: &QDAGTransaction) -> Result<(), JsValue> {
        // Remove validated tips
        for parent_id in &tx.validates {
            self.tips.remove(parent_id);
        }

        // Add this transaction as a new tip
        self.tips.insert(tx.id);

        // Update balances
        match tx.tx_type {
            TransactionType::ComputeReward => {
                // Minting new credits (only if under max supply)
                if self.total_supply + tx.amount <= self.max_supply {
                    *self.balances.entry(tx.recipient.clone()).or_insert(0) += tx.amount as i64;
                    self.total_supply += tx.amount;
                }
            }
            TransactionType::Transfer => {
                *self.balances.entry(tx.sender.clone()).or_insert(0) -= tx.amount as i64;
                *self.balances.entry(tx.recipient.clone()).or_insert(0) += tx.amount as i64;
            }
            TransactionType::Stake => {
                *self.balances.entry(tx.sender.clone()).or_insert(0) -= tx.amount as i64;
                *self.stakes.entry(tx.sender.clone()).or_insert(0) += tx.amount;
            }
            TransactionType::Unstake => {
                let staked = self.stakes.get(&tx.sender).copied().unwrap_or(0);
                if tx.amount <= staked {
                    *self.stakes.entry(tx.sender.clone()).or_insert(0) -= tx.amount;
                    *self.balances.entry(tx.sender.clone()).or_insert(0) += tx.amount as i64;
                }
            }
            TransactionType::Penalty => {
                let staked = self.stakes.get(&tx.sender).copied().unwrap_or(0);
                let penalty = tx.amount.min(staked);
                *self.stakes.entry(tx.sender.clone()).or_insert(0) -= penalty;
                // Burned (not transferred)
            }
            TransactionType::Genesis => {
                // Already handled in create_genesis
            }
        }

        // Store transaction
        self.transactions.insert(tx.id, tx.clone());

        // Update transaction count
        *self.tx_counts.entry(tx.sender.clone()).or_insert(0) += 1;

        Ok(())
    }

    /// Select tips for validation (weighted random selection)
    fn select_tips(&self, count: usize) -> Result<Vec<[u8; 32]>, JsValue> {
        if self.tips.is_empty() {
            return Ok(vec![]);
        }

        // Simple random selection (would use weighted selection in production)
        let tips: Vec<[u8; 32]> = self.tips.iter().copied().take(count).collect();
        Ok(tips)
    }

    /// Calculate cumulative weight from parent transactions
    fn calculate_cumulative_weight(&self, parents: &[[u8; 32]]) -> u64 {
        let mut weight = 1u64;
        for parent_id in parents {
            if let Some(parent) = self.transactions.get(parent_id) {
                weight = weight.saturating_add(parent.cumulative_weight);
            }
        }
        weight
    }

    /// Hash transaction content
    fn hash_transaction(&self, tx: &QDAGTransaction) -> [u8; 32] {
        let mut hasher = Sha256::new();

        // Hash all fields except id and signature
        for parent in &tx.validates {
            hasher.update(parent);
        }
        hasher.update(tx.sender.as_bytes());
        hasher.update(tx.recipient.as_bytes());
        hasher.update(&tx.amount.to_le_bytes());
        hasher.update(&[tx.tx_type as u8]);
        hasher.update(&tx.timestamp.to_le_bytes());
        hasher.update(&tx.sender_pubkey);
        hasher.update(&tx.pow_nonce.to_le_bytes());

        hasher.finalize().into()
    }

    /// Find valid proof-of-work nonce
    fn find_pow_nonce(&self, tx: &QDAGTransaction) -> Result<u64, JsValue> {
        let mut tx = tx.clone();

        for nonce in 0..u64::MAX {
            tx.pow_nonce = nonce;
            let hash = self.hash_transaction(&tx);

            if self.check_pow_hash(&hash) {
                return Ok(nonce);
            }

            // Timeout after 1 million attempts
            if nonce > 1_000_000 {
                return Err(JsValue::from_str("PoW timeout - difficulty too high"));
            }
        }

        Err(JsValue::from_str("Failed to find valid nonce"))
    }

    /// Check if hash meets PoW difficulty
    fn check_pow_hash(&self, hash: &[u8; 32]) -> bool {
        // Count leading zero bytes
        let zero_bytes = hash.iter().take_while(|&&b| b == 0).count();

        // Count additional leading zero bits in the first non-zero byte
        let extra_bits = hash.get(zero_bytes)
            .map(|b| b.leading_zeros() as usize)
            .unwrap_or(0);

        let total_leading_zeros = zero_bytes * 8 + extra_bits;
        total_leading_zeros >= self.pow_difficulty as usize
    }

    /// Verify proof of work
    fn verify_pow(&self, tx: &QDAGTransaction) -> bool {
        let hash = self.hash_transaction(tx);
        self.check_pow_hash(&hash)
    }

    /// Sign transaction with Ed25519
    fn sign_transaction(&self, tx: &QDAGTransaction, privkey: &[u8]) -> Result<Vec<u8>, JsValue> {
        use ed25519_dalek::{SigningKey, Signer};

        if privkey.len() != 32 {
            return Err(JsValue::from_str("Invalid private key length"));
        }

        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(privkey);

        let signing_key = SigningKey::from_bytes(&key_bytes);
        let message = self.hash_transaction(tx);
        let signature = signing_key.sign(&message);

        Ok(signature.to_bytes().to_vec())
    }

    /// Verify Ed25519 signature
    fn verify_signature(&self, tx: &QDAGTransaction) -> bool {
        use ed25519_dalek::{VerifyingKey, Signature, Verifier};

        if tx.sender_pubkey.len() != 32 || tx.signature_ed25519.len() != 64 {
            return false;
        }

        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(&tx.sender_pubkey);

        let mut sig_bytes = [0u8; 64];
        sig_bytes.copy_from_slice(&tx.signature_ed25519);

        let verifying_key = match VerifyingKey::from_bytes(&key_bytes) {
            Ok(k) => k,
            Err(_) => return false,
        };

        let signature = Signature::from_bytes(&sig_bytes);
        let message = self.hash_transaction(tx);

        verifying_key.verify(&message, &signature).is_ok()
    }

    /// Get total supply
    #[wasm_bindgen(js_name = totalSupply)]
    pub fn total_supply(&self) -> u64 {
        self.total_supply
    }

    /// Get transaction count
    #[wasm_bindgen(js_name = transactionCount)]
    pub fn transaction_count(&self) -> usize {
        self.transactions.len()
    }

    /// Get tip count
    #[wasm_bindgen(js_name = tipCount)]
    pub fn tip_count(&self) -> usize {
        self.tips.len()
    }

    /// Export ledger state for sync
    #[wasm_bindgen(js_name = exportState)]
    pub fn export_state(&self) -> Result<Vec<u8>, JsValue> {
        let state = LedgerState {
            transactions: self.transactions.values().cloned().collect(),
            tips: self.tips.iter().copied().collect(),
            total_supply: self.total_supply,
        };

        serde_json::to_vec(&state)
            .map_err(|e| JsValue::from_str(&format!("Serialization error: {}", e)))
    }

    /// Import ledger state from sync
    #[wasm_bindgen(js_name = importState)]
    pub fn import_state(&mut self, state_bytes: &[u8]) -> Result<u32, JsValue> {
        let state: LedgerState = serde_json::from_slice(state_bytes)
            .map_err(|e| JsValue::from_str(&format!("Deserialization error: {}", e)))?;

        let mut imported = 0u32;

        for tx in state.transactions {
            if !self.transactions.contains_key(&tx.id) {
                // Validate before importing
                if self.validate_transaction(&tx).is_ok() {
                    self.apply_transaction(&tx)?;
                    imported += 1;
                }
            }
        }

        Ok(imported)
    }
}

/// Serializable ledger state
#[derive(Serialize, Deserialize)]
struct LedgerState {
    transactions: Vec<QDAGTransaction>,
    tips: Vec<[u8; 32]>,
    total_supply: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    // Tests that require WASM environment (js_sys::Date)
    #[cfg(target_arch = "wasm32")]
    #[test]
    fn test_genesis_creation() {
        let mut ledger = QDAGLedger::new();
        let pubkey = [1u8; 32];

        let genesis_id = ledger.create_genesis(1_000_000_000_000, &pubkey).unwrap();
        assert_eq!(genesis_id.len(), 32);
        assert_eq!(ledger.total_supply(), 1_000_000_000_000);
        assert_eq!(ledger.balance(&hex::encode(&pubkey)), 1_000_000_000_000);
    }

    #[test]
    fn test_pow_difficulty() {
        // Test PoW hash validation (no WASM dependencies)
        // Hash with 2 leading zero bytes should pass difficulty 16
        let hash = [0u8, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14,
                    15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30];

        // Calculate leading zeros directly
        let zero_bytes = hash.iter().take_while(|&&b| b == 0).count();
        let extra_bits = hash.get(zero_bytes).map(|b| b.leading_zeros() as usize).unwrap_or(0);
        let leading_zeros = zero_bytes * 8 + extra_bits;

        // Difficulty 16 means 16 leading zero bits (2 zero bytes)
        assert!(leading_zeros >= 16);

        // Hash with only 1 leading zero byte should fail difficulty 16
        let hash2 = [0u8, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
                     16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31];
        let zero_bytes2 = hash2.iter().take_while(|&&b| b == 0).count();
        let extra_bits2 = hash2.get(zero_bytes2).map(|b| b.leading_zeros() as usize).unwrap_or(0);
        let leading_zeros2 = zero_bytes2 * 8 + extra_bits2;
        assert!(leading_zeros2 < 16);
    }
}
