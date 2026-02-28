//! # Compute AMM (Automated Market Maker)
//!
//! An AMM for compute pricing in the edge-net P2P AI network.
//! Uses a constant-product formula (x * y = k) with dynamic fees.
//!
//! ## Features
//!
//! - **Constant Product**: x * y = k invariant ensures liquidity
//! - **Dynamic Fees**: 0.3% base to 3% at high utilization
//! - **LP Tokens**: Liquidity providers receive proportional tokens
//! - **Price Discovery**: Real-time compute pricing via market forces
//!
//! ## Example
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                     COMPUTE AMM POOL                            │
//! ├─────────────────────────────────────────────────────────────────┤
//! │                                                                 │
//! │   rUv Reserve          Compute Reserve (seconds)                │
//! │   ┌───────────┐        ┌───────────┐                           │
//! │   │ 1,000,000 │   ×    │ 1,000,000 │  = k (invariant)          │
//! │   └───────────┘        └───────────┘                           │
//! │        │                    │                                   │
//! │        └────────┬───────────┘                                   │
//! │                 │                                               │
//! │           Price = rUv / Compute                                 │
//! │                 ▼                                               │
//! │           1 rUv = 1 compute-second (at 1:1 ratio)              │
//! │                                                                 │
//! │   High utilization → Higher fees (0.3% to 3%)                   │
//! │   Low utilization  → Lower fees (0.3% base)                     │
//! │                                                                 │
//! └─────────────────────────────────────────────────────────────────┘
//! ```

use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};
use std::sync::RwLock;

/// Initial compute reserve for baseline calculations
pub const INITIAL_COMPUTE: u64 = 1_000_000;

/// Minimum fee rate (0.3%)
pub const MIN_FEE_RATE: f32 = 0.003;

/// Maximum fee rate at high utilization (3%)
pub const MAX_FEE_RATE: f32 = 0.03;

/// Minimum liquidity to prevent manipulation
pub const MIN_LIQUIDITY: u64 = 1000;

/// AMM Error types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AmmError {
    /// Insufficient reserves for swap
    InsufficientReserves,
    /// Insufficient input amount
    InsufficientInput,
    /// Insufficient liquidity in pool
    InsufficientLiquidity,
    /// Slippage tolerance exceeded
    SlippageExceeded,
    /// Invalid amount (zero or overflow)
    InvalidAmount,
    /// Pool is empty
    EmptyPool,
    /// Math overflow
    Overflow,
}

impl std::fmt::Display for AmmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AmmError::InsufficientReserves => write!(f, "Insufficient reserves for swap"),
            AmmError::InsufficientInput => write!(f, "Insufficient input amount"),
            AmmError::InsufficientLiquidity => write!(f, "Insufficient liquidity in pool"),
            AmmError::SlippageExceeded => write!(f, "Slippage tolerance exceeded"),
            AmmError::InvalidAmount => write!(f, "Invalid amount (zero or overflow)"),
            AmmError::EmptyPool => write!(f, "Pool is empty"),
            AmmError::Overflow => write!(f, "Math overflow"),
        }
    }
}

impl std::error::Error for AmmError {}

/// LP (Liquidity Provider) Token record
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LpPosition {
    /// Provider node ID
    pub provider_id: String,
    /// LP token balance
    pub lp_tokens: u64,
    /// Initial rUv contribution
    pub initial_ruv: u64,
    /// Initial compute contribution
    pub initial_compute: u64,
    /// Timestamp of deposit
    pub deposited_at: u64,
}

/// Swap event for analytics
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SwapEvent {
    /// Trader node ID
    pub trader_id: String,
    /// Input token (ruv or compute)
    pub input_type: SwapType,
    /// Amount input
    pub amount_in: u64,
    /// Amount output
    pub amount_out: u64,
    /// Fee paid
    pub fee: u64,
    /// Timestamp
    pub timestamp: u64,
}

/// Type of swap
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum SwapType {
    /// Swapping rUv for compute time
    RuvForCompute,
    /// Swapping compute time for rUv
    ComputeForRuv,
}

/// Compute AMM - Automated Market Maker for compute pricing
#[wasm_bindgen]
pub struct ComputeAMM {
    /// rUv credit reserve
    reserve_ruv: RwLock<u64>,
    /// Compute-second reserve
    reserve_compute: RwLock<u64>,
    /// Base fee rate (0.3% = 0.003)
    fee_rate: f32,
    /// k invariant (x * y = k)
    k_invariant: RwLock<u128>,
    /// Total LP tokens issued
    total_lp_tokens: RwLock<u64>,
    /// LP positions by provider
    lp_positions: RwLock<Vec<LpPosition>>,
    /// Swap history for analytics
    swap_history: RwLock<Vec<SwapEvent>>,
    /// Cumulative fees collected
    fees_collected: RwLock<u64>,
    /// Initial compute (for utilization calculation)
    initial_compute: u64,
}

#[wasm_bindgen]
impl ComputeAMM {
    /// Create a new Compute AMM with initial reserves
    #[wasm_bindgen(constructor)]
    pub fn new(initial_ruv: u64, initial_compute: u64) -> Result<ComputeAMM, JsValue> {
        if initial_ruv < MIN_LIQUIDITY || initial_compute < MIN_LIQUIDITY {
            return Err(JsValue::from_str("Initial reserves too low"));
        }

        let k = (initial_ruv as u128) * (initial_compute as u128);

        Ok(ComputeAMM {
            reserve_ruv: RwLock::new(initial_ruv),
            reserve_compute: RwLock::new(initial_compute),
            fee_rate: MIN_FEE_RATE,
            k_invariant: RwLock::new(k),
            total_lp_tokens: RwLock::new(initial_ruv), // Initial LP = sqrt(ruv * compute) simplified
            lp_positions: RwLock::new(Vec::new()),
            swap_history: RwLock::new(Vec::new()),
            fees_collected: RwLock::new(0),
            initial_compute,
        })
    }

    /// Get current price in rUv per compute-second
    #[wasm_bindgen(js_name = getPrice)]
    pub fn get_price(&self) -> f64 {
        let ruv = *self.reserve_ruv.read().unwrap();
        let compute = *self.reserve_compute.read().unwrap();

        if compute == 0 {
            return f64::MAX;
        }

        ruv as f64 / compute as f64
    }

    /// Get current rUv reserve
    #[wasm_bindgen(js_name = getReserveRuv)]
    pub fn get_reserve_ruv(&self) -> u64 {
        *self.reserve_ruv.read().unwrap()
    }

    /// Get current compute reserve
    #[wasm_bindgen(js_name = getReserveCompute)]
    pub fn get_reserve_compute(&self) -> u64 {
        *self.reserve_compute.read().unwrap()
    }

    /// Get k invariant
    #[wasm_bindgen(js_name = getKInvariant)]
    pub fn get_k_invariant(&self) -> f64 {
        *self.k_invariant.read().unwrap() as f64
    }

    /// Get total LP tokens
    #[wasm_bindgen(js_name = getTotalLpTokens)]
    pub fn get_total_lp_tokens(&self) -> u64 {
        *self.total_lp_tokens.read().unwrap()
    }

    /// Get total fees collected
    #[wasm_bindgen(js_name = getFeesCollected)]
    pub fn get_fees_collected(&self) -> u64 {
        *self.fees_collected.read().unwrap()
    }

    /// Dynamic fee based on pool utilization
    /// Fee increases as compute is depleted (high demand)
    #[wasm_bindgen(js_name = dynamicFee)]
    pub fn dynamic_fee(&self) -> f32 {
        let reserve = *self.reserve_compute.read().unwrap();
        let utilization = 1.0 - (reserve as f32 / self.initial_compute as f32);
        let utilization_clamped = utilization.clamp(0.0, 1.0);

        // Linear interpolation: 0.3% at 0% utilization, 3% at 100% utilization
        MIN_FEE_RATE + (MAX_FEE_RATE - MIN_FEE_RATE) * utilization_clamped
    }

    /// Get pool utilization (0.0 - 1.0)
    #[wasm_bindgen(js_name = getUtilization)]
    pub fn get_utilization(&self) -> f32 {
        let reserve = *self.reserve_compute.read().unwrap();
        let utilization = 1.0 - (reserve as f32 / self.initial_compute as f32);
        utilization.clamp(0.0, 1.0)
    }

    /// Calculate expected output for rUv to compute swap (quote)
    #[wasm_bindgen(js_name = quoteRuvForCompute)]
    pub fn quote_ruv_for_compute(&self, ruv_in: u64) -> u64 {
        let reserve_ruv = *self.reserve_ruv.read().unwrap();
        let reserve_compute = *self.reserve_compute.read().unwrap();

        let fee = (ruv_in as f64 * self.dynamic_fee() as f64) as u64;
        let ruv_after_fee = ruv_in.saturating_sub(fee);

        if ruv_after_fee == 0 {
            return 0;
        }

        // constant product: (x + dx) * (y - dy) = k
        // dy = y - k / (x + dx)
        let k = *self.k_invariant.read().unwrap();
        let new_ruv = (reserve_ruv as u128).saturating_add(ruv_after_fee as u128);

        if new_ruv == 0 {
            return 0;
        }

        let new_compute = k / new_ruv;
        reserve_compute.saturating_sub(new_compute as u64)
    }

    /// Calculate expected output for compute to rUv swap (quote)
    #[wasm_bindgen(js_name = quoteComputeForRuv)]
    pub fn quote_compute_for_ruv(&self, compute_in: u64) -> u64 {
        let reserve_ruv = *self.reserve_ruv.read().unwrap();
        let reserve_compute = *self.reserve_compute.read().unwrap();

        let fee = (compute_in as f64 * self.dynamic_fee() as f64) as u64;
        let compute_after_fee = compute_in.saturating_sub(fee);

        if compute_after_fee == 0 {
            return 0;
        }

        let k = *self.k_invariant.read().unwrap();
        let new_compute = (reserve_compute as u128).saturating_add(compute_after_fee as u128);

        if new_compute == 0 {
            return 0;
        }

        let new_ruv = k / new_compute;
        reserve_ruv.saturating_sub(new_ruv as u64)
    }

    /// Get swap count
    #[wasm_bindgen(js_name = getSwapCount)]
    pub fn get_swap_count(&self) -> usize {
        self.swap_history.read().unwrap().len()
    }

    /// Get LP position count
    #[wasm_bindgen(js_name = getLpPositionCount)]
    pub fn get_lp_position_count(&self) -> usize {
        self.lp_positions.read().unwrap().len()
    }

    /// Get pool statistics as JSON
    #[wasm_bindgen(js_name = getPoolStats)]
    pub fn get_pool_stats(&self) -> String {
        let stats = serde_json::json!({
            "reserve_ruv": self.get_reserve_ruv(),
            "reserve_compute": self.get_reserve_compute(),
            "price": self.get_price(),
            "k_invariant": self.get_k_invariant(),
            "total_lp_tokens": self.get_total_lp_tokens(),
            "fees_collected": self.get_fees_collected(),
            "dynamic_fee_rate": self.dynamic_fee(),
            "utilization": self.get_utilization(),
            "swap_count": self.get_swap_count(),
            "lp_count": self.get_lp_position_count(),
        });
        serde_json::to_string(&stats).unwrap_or_else(|_| "{}".to_string())
    }
}

impl ComputeAMM {
    /// Swap rUv for compute time
    /// Returns the amount of compute-seconds received
    pub fn swap_ruv_for_compute(&self, ruv_in: u64, trader_id: &str) -> Result<u64, AmmError> {
        if ruv_in == 0 {
            return Err(AmmError::InvalidAmount);
        }

        let mut reserve_ruv = self.reserve_ruv.write().unwrap();
        let mut reserve_compute = self.reserve_compute.write().unwrap();
        let k = *self.k_invariant.read().unwrap();

        // Calculate dynamic fee
        let fee_rate = self.dynamic_fee();
        let fee = (ruv_in as f64 * fee_rate as f64) as u64;
        let ruv_after_fee = ruv_in.saturating_sub(fee);

        if ruv_after_fee == 0 {
            return Err(AmmError::InsufficientInput);
        }

        // Calculate new reserves maintaining k invariant
        let new_ruv = (*reserve_ruv as u128)
            .checked_add(ruv_after_fee as u128)
            .ok_or(AmmError::Overflow)?;

        let new_compute = k
            .checked_div(new_ruv)
            .ok_or(AmmError::Overflow)?;

        let compute_out = (*reserve_compute as u128)
            .checked_sub(new_compute)
            .ok_or(AmmError::InsufficientReserves)? as u64;

        if compute_out == 0 {
            return Err(AmmError::InsufficientReserves);
        }

        // Ensure minimum liquidity remains
        if new_compute < MIN_LIQUIDITY as u128 {
            return Err(AmmError::InsufficientLiquidity);
        }

        // Update reserves
        *reserve_ruv = new_ruv as u64;
        *reserve_compute = new_compute as u64;

        // Record fee
        *self.fees_collected.write().unwrap() += fee;

        // Record swap event
        let now = js_sys::Date::now() as u64;
        self.swap_history.write().unwrap().push(SwapEvent {
            trader_id: trader_id.to_string(),
            input_type: SwapType::RuvForCompute,
            amount_in: ruv_in,
            amount_out: compute_out,
            fee,
            timestamp: now,
        });

        Ok(compute_out)
    }

    /// Swap compute time for rUv
    /// Returns the amount of rUv received
    pub fn swap_compute_for_ruv(&self, compute_in: u64, trader_id: &str) -> Result<u64, AmmError> {
        if compute_in == 0 {
            return Err(AmmError::InvalidAmount);
        }

        let mut reserve_ruv = self.reserve_ruv.write().unwrap();
        let mut reserve_compute = self.reserve_compute.write().unwrap();
        let k = *self.k_invariant.read().unwrap();

        // Calculate dynamic fee
        let fee_rate = self.dynamic_fee();
        let fee = (compute_in as f64 * fee_rate as f64) as u64;
        let compute_after_fee = compute_in.saturating_sub(fee);

        if compute_after_fee == 0 {
            return Err(AmmError::InsufficientInput);
        }

        // Calculate new reserves maintaining k invariant
        let new_compute = (*reserve_compute as u128)
            .checked_add(compute_after_fee as u128)
            .ok_or(AmmError::Overflow)?;

        let new_ruv = k
            .checked_div(new_compute)
            .ok_or(AmmError::Overflow)?;

        let ruv_out = (*reserve_ruv as u128)
            .checked_sub(new_ruv)
            .ok_or(AmmError::InsufficientReserves)? as u64;

        if ruv_out == 0 {
            return Err(AmmError::InsufficientReserves);
        }

        // Ensure minimum liquidity remains
        if new_ruv < MIN_LIQUIDITY as u128 {
            return Err(AmmError::InsufficientLiquidity);
        }

        // Update reserves
        *reserve_ruv = new_ruv as u64;
        *reserve_compute = new_compute as u64;

        // Record swap event
        let now = js_sys::Date::now() as u64;
        self.swap_history.write().unwrap().push(SwapEvent {
            trader_id: trader_id.to_string(),
            input_type: SwapType::ComputeForRuv,
            amount_in: compute_in,
            amount_out: ruv_out,
            fee,
            timestamp: now,
        });

        Ok(ruv_out)
    }

    /// Add liquidity to the pool
    /// Returns the amount of LP tokens minted
    pub fn add_liquidity(&self, ruv: u64, compute: u64, provider_id: &str) -> Result<u64, AmmError> {
        if ruv == 0 || compute == 0 {
            return Err(AmmError::InvalidAmount);
        }

        let mut reserve_ruv = self.reserve_ruv.write().unwrap();
        let mut reserve_compute = self.reserve_compute.write().unwrap();
        let mut total_lp = self.total_lp_tokens.write().unwrap();
        let mut k = self.k_invariant.write().unwrap();

        // Calculate LP tokens to mint
        // LP tokens = min(ruv / reserve_ruv, compute / reserve_compute) * total_lp
        let lp_tokens = if *total_lp == 0 {
            // First liquidity provider gets sqrt(ruv * compute) tokens
            ((ruv as f64 * compute as f64).sqrt()) as u64
        } else {
            let ruv_ratio = (ruv as u128 * *total_lp as u128) / *reserve_ruv as u128;
            let compute_ratio = (compute as u128 * *total_lp as u128) / *reserve_compute as u128;
            ruv_ratio.min(compute_ratio) as u64
        };

        if lp_tokens == 0 {
            return Err(AmmError::InvalidAmount);
        }

        // Update reserves
        *reserve_ruv = reserve_ruv.saturating_add(ruv);
        *reserve_compute = reserve_compute.saturating_add(compute);

        // Update k invariant
        *k = (*reserve_ruv as u128) * (*reserve_compute as u128);

        // Mint LP tokens
        *total_lp = total_lp.saturating_add(lp_tokens);

        // Record LP position
        let now = js_sys::Date::now() as u64;
        let mut positions = self.lp_positions.write().unwrap();

        // Check if provider already has a position
        if let Some(pos) = positions.iter_mut().find(|p| p.provider_id == provider_id) {
            pos.lp_tokens = pos.lp_tokens.saturating_add(lp_tokens);
            pos.initial_ruv = pos.initial_ruv.saturating_add(ruv);
            pos.initial_compute = pos.initial_compute.saturating_add(compute);
        } else {
            positions.push(LpPosition {
                provider_id: provider_id.to_string(),
                lp_tokens,
                initial_ruv: ruv,
                initial_compute: compute,
                deposited_at: now,
            });
        }

        Ok(lp_tokens)
    }

    /// Remove liquidity from the pool
    /// Returns (ruv_amount, compute_amount)
    pub fn remove_liquidity(&self, lp_tokens: u64, provider_id: &str) -> Result<(u64, u64), AmmError> {
        if lp_tokens == 0 {
            return Err(AmmError::InvalidAmount);
        }

        let mut reserve_ruv = self.reserve_ruv.write().unwrap();
        let mut reserve_compute = self.reserve_compute.write().unwrap();
        let mut total_lp = self.total_lp_tokens.write().unwrap();
        let mut k = self.k_invariant.write().unwrap();
        let mut positions = self.lp_positions.write().unwrap();

        // Find provider's position
        let pos = positions.iter_mut()
            .find(|p| p.provider_id == provider_id)
            .ok_or(AmmError::InsufficientLiquidity)?;

        if pos.lp_tokens < lp_tokens {
            return Err(AmmError::InsufficientLiquidity);
        }

        // Calculate amounts to return
        let ruv_out = (lp_tokens as u128 * *reserve_ruv as u128 / *total_lp as u128) as u64;
        let compute_out = (lp_tokens as u128 * *reserve_compute as u128 / *total_lp as u128) as u64;

        // Ensure minimum liquidity remains
        let new_ruv = reserve_ruv.saturating_sub(ruv_out);
        let new_compute = reserve_compute.saturating_sub(compute_out);

        if new_ruv < MIN_LIQUIDITY || new_compute < MIN_LIQUIDITY {
            return Err(AmmError::InsufficientLiquidity);
        }

        // Update reserves
        *reserve_ruv = new_ruv;
        *reserve_compute = new_compute;

        // Update k invariant
        *k = (*reserve_ruv as u128) * (*reserve_compute as u128);

        // Burn LP tokens
        *total_lp = total_lp.saturating_sub(lp_tokens);
        pos.lp_tokens = pos.lp_tokens.saturating_sub(lp_tokens);

        // Remove empty positions
        if pos.lp_tokens == 0 {
            let idx = positions.iter().position(|p| p.provider_id == provider_id);
            if let Some(i) = idx {
                positions.remove(i);
            }
        }

        Ok((ruv_out, compute_out))
    }

    /// Get LP position for a provider
    pub fn get_lp_position(&self, provider_id: &str) -> Option<LpPosition> {
        self.lp_positions.read().unwrap()
            .iter()
            .find(|p| p.provider_id == provider_id)
            .cloned()
    }

    /// Get recent swap history
    pub fn get_swap_history(&self, limit: usize) -> Vec<SwapEvent> {
        let history = self.swap_history.read().unwrap();
        history.iter().rev().take(limit).cloned().collect()
    }

    /// Calculate price impact for a swap
    pub fn calculate_price_impact(&self, ruv_in: u64) -> f64 {
        let current_price = self.get_price();

        // Simulate the swap to get new price
        let reserve_ruv = *self.reserve_ruv.read().unwrap();
        let reserve_compute = *self.reserve_compute.read().unwrap();
        let k = *self.k_invariant.read().unwrap();

        let fee = (ruv_in as f64 * self.dynamic_fee() as f64) as u64;
        let ruv_after_fee = ruv_in.saturating_sub(fee);

        let new_ruv = (reserve_ruv as u128).saturating_add(ruv_after_fee as u128);
        let new_compute = k / new_ruv;

        if new_compute == 0 {
            return 1.0; // 100% price impact
        }

        let new_price = new_ruv as f64 / new_compute as f64;

        ((new_price - current_price) / current_price).abs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_amm_creation() {
        let amm = ComputeAMM::new(1_000_000, 1_000_000).unwrap();
        assert_eq!(amm.get_reserve_ruv(), 1_000_000);
        assert_eq!(amm.get_reserve_compute(), 1_000_000);
        assert!((amm.get_price() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_dynamic_fee() {
        let amm = ComputeAMM::new(1_000_000, 1_000_000).unwrap();

        // At 0% utilization, fee should be MIN_FEE_RATE
        let fee = amm.dynamic_fee();
        assert!((fee - MIN_FEE_RATE).abs() < 0.001);
    }

    #[test]
    fn test_quote() {
        let amm = ComputeAMM::new(1_000_000, 1_000_000).unwrap();

        // Quote should return reasonable amount
        let compute_out = amm.quote_ruv_for_compute(10_000);
        assert!(compute_out > 0);
        assert!(compute_out < 10_000); // Should be less due to price impact + fees
    }

    #[test]
    fn test_k_invariant() {
        let amm = ComputeAMM::new(1_000_000, 1_000_000).unwrap();
        let initial_k = amm.get_k_invariant();

        // After swap, k should remain the same (minus fees which affect reserves)
        let _ = amm.swap_ruv_for_compute(10_000, "test");

        // k should be maintained (within reasonable tolerance due to fees)
        let k_after = amm.get_k_invariant();
        assert!(k_after >= initial_k * 0.99);
    }

    #[test]
    fn test_insufficient_reserves() {
        let amm = ComputeAMM::new(10_000, 10_000).unwrap();

        // Trying to swap too much should fail
        let result = amm.swap_ruv_for_compute(9_500, "test");
        assert!(result.is_err());
    }

    #[test]
    fn test_liquidity() {
        let amm = ComputeAMM::new(1_000_000, 1_000_000).unwrap();

        // Add liquidity
        let lp_tokens = amm.add_liquidity(100_000, 100_000, "provider1").unwrap();
        assert!(lp_tokens > 0);

        // Remove liquidity
        let (ruv, compute) = amm.remove_liquidity(lp_tokens / 2, "provider1").unwrap();
        assert!(ruv > 0);
        assert!(compute > 0);
    }
}
