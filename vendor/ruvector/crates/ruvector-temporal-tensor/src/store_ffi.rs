//! WASM/C FFI for the block-based temporal tensor store (ADR-022).
//!
//! Exports `extern "C"` functions prefixed with `tts_` for:
//! - Store lifecycle (`tts_init`)
//! - Block ingest and read (`tts_put`, `tts_get`)
//! - Access tracking (`tts_touch`)
//! - Maintenance (`tts_tick`, `tts_evict`)
//! - Statistics (`tts_stats`, `tts_block_count`, `tts_tier_count`)
//!
//! Coexists with `ffi.rs` which exports `ttc_*` functions for the
//! frame-based compressor.

use std::collections::HashMap;

use crate::quantizer;
use crate::segment;

// ── Error codes ──────────────────────────────────────────────────────

#[allow(dead_code)]
const ERR_NOT_INITIALIZED: i32 = -1;
const ERR_NULL_POINTER: i32 = -2;
const ERR_INVALID_CONFIG: i32 = -3;
const ERR_BLOCK_NOT_FOUND: i32 = -4;
const ERR_BUFFER_TOO_SMALL: i32 = -5;
const ERR_EMPTY_DATA: i32 = -6;

// ── Types ────────────────────────────────────────────────────────────
// These mirror the types defined in store.rs and tiering.rs which are
// being written in parallel.  Once those modules land, these can be
// replaced with `use crate::store::*` / `use crate::tiering::*`.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct BlockKey {
    tensor_id: u128,
    block_index: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
enum Tier {
    Hot = 0,
    Warm = 1,
    Cool = 2,
    Cold = 3,
}

impl Tier {
    fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Tier::Hot),
            1 => Some(Tier::Warm),
            2 => Some(Tier::Cool),
            3 => Some(Tier::Cold),
            _ => None,
        }
    }

    /// Quantization bit-width for this tier.
    fn bits(self) -> u8 {
        match self {
            Tier::Hot => 8,
            Tier::Warm => 7,
            Tier::Cool => 5,
            Tier::Cold => 3,
        }
    }
}

#[derive(Clone, Debug)]
struct BlockMeta {
    tier: Tier,
    access_count: u32,
    last_access_ts: u64,
    ema_score: f32,
    /// Original f32 count; used when re-tiering to size the decode buffer.
    #[allow(dead_code)]
    element_count: usize,
}

/// Binary config layout (little-endian, 45 bytes):
/// ```text
/// [block_bytes:u32][alpha:f32][tau:f32][w_ema:f32][w_pop:f32][w_rec:f32]
/// [t1:f32][t2:f32][t3:f32][hysteresis:f32][min_residency:u32][max_delta_chain:u8]
/// ```
#[derive(Clone, Debug)]
struct TierConfig {
    block_bytes: u32,
    alpha: f32,
    tau: f32,
    w_ema: f32,
    w_pop: f32,
    w_rec: f32,
    t1: f32,
    t2: f32,
    t3: f32,
    hysteresis: f32,
    min_residency: u32,
    max_delta_chain: u8,
}

const CONFIG_BINARY_LEN: usize = 45;

impl Default for TierConfig {
    fn default() -> Self {
        Self {
            block_bytes: 4096,
            alpha: 0.3,
            tau: 100.0,
            w_ema: 0.5,
            w_pop: 0.3,
            w_rec: 0.2,
            t1: 0.8,
            t2: 0.5,
            t3: 0.2,
            hysteresis: 0.05,
            min_residency: 10,
            max_delta_chain: 4,
        }
    }
}

impl TierConfig {
    fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < CONFIG_BINARY_LEN {
            return None;
        }
        let mut off = 0usize;
        let block_bytes = read_u32_le(bytes, &mut off);
        let alpha = read_f32_le(bytes, &mut off);
        let tau = read_f32_le(bytes, &mut off);
        let w_ema = read_f32_le(bytes, &mut off);
        let w_pop = read_f32_le(bytes, &mut off);
        let w_rec = read_f32_le(bytes, &mut off);
        let t1 = read_f32_le(bytes, &mut off);
        let t2 = read_f32_le(bytes, &mut off);
        let t3 = read_f32_le(bytes, &mut off);
        let hysteresis = read_f32_le(bytes, &mut off);
        let min_residency = read_u32_le(bytes, &mut off);
        let max_delta_chain = bytes[off];

        if ![alpha, tau, w_ema, w_pop, w_rec, t1, t2, t3, hysteresis]
            .iter()
            .all(|v| v.is_finite())
        {
            return None;
        }

        Some(Self {
            block_bytes,
            alpha,
            tau,
            w_ema,
            w_pop,
            w_rec,
            t1,
            t2,
            t3,
            hysteresis,
            min_residency,
            max_delta_chain,
        })
    }
}

// ── Store ────────────────────────────────────────────────────────────

struct TieredStore {
    blocks: HashMap<BlockKey, (BlockMeta, Vec<u8>)>,
}

impl TieredStore {
    fn new() -> Self {
        Self {
            blocks: HashMap::new(),
        }
    }

    fn block_count(&self) -> usize {
        self.blocks.len()
    }

    fn tier_count(&self, tier: Tier) -> usize {
        self.blocks.values().filter(|(m, _)| m.tier == tier).count()
    }

    fn total_bytes(&self) -> usize {
        self.blocks.values().map(|(_, d)| d.len()).sum()
    }
}

// ── Global state ─────────────────────────────────────────────────────

struct StoreState {
    store: TieredStore,
    config: TierConfig,
    tick_count: u64,
}

static mut STORE_STATE: Option<StoreState> = None;

// ── Helpers ──────────────────────────────────────────────────────────

/// Combine hi/lo u64 into u128 tensor_id.
#[inline]
fn make_tensor_id(hi: u64, lo: u64) -> u128 {
    ((hi as u128) << 64) | (lo as u128)
}

/// Access the global store state, initializing with defaults if needed.
fn with_state<F, R>(f: F) -> R
where
    F: FnOnce(&mut StoreState) -> R,
{
    unsafe {
        if STORE_STATE.is_none() {
            STORE_STATE = Some(StoreState {
                store: TieredStore::new(),
                config: TierConfig::default(),
                tick_count: 0,
            });
        }
        f(STORE_STATE.as_mut().unwrap())
    }
}

const DEFAULT_GROUP_LEN: usize = 64;

/// Composite access score used for tier selection.
fn compute_score(config: &TierConfig, meta: &BlockMeta, tick: u64) -> f32 {
    let recency = if tick > meta.last_access_ts {
        (-((tick - meta.last_access_ts) as f32) / config.tau).exp()
    } else {
        1.0
    };
    let popularity = (meta.access_count as f32).ln_1p();
    config.w_ema * meta.ema_score + config.w_pop * popularity + config.w_rec * recency
}

/// Map a score to a tier using the config thresholds.
fn choose_tier(config: &TierConfig, score: f32) -> Tier {
    if score >= config.t1 {
        Tier::Hot
    } else if score >= config.t2 {
        Tier::Warm
    } else if score >= config.t3 {
        Tier::Cool
    } else {
        Tier::Cold
    }
}

/// Quantize f32 data and encode into a compressed segment.
fn encode_block(data: &[f32], tier: Tier) -> Vec<u8> {
    let bits = tier.bits();
    let group_len = DEFAULT_GROUP_LEN;
    let scales = quantizer::compute_scales(data, group_len, bits);
    let mut packed = Vec::new();
    quantizer::quantize_and_pack(data, &scales, group_len, bits, &mut packed);
    let mut seg = Vec::new();
    segment::encode(
        bits,
        group_len as u32,
        data.len() as u32,
        1,
        &scales,
        &packed,
        &mut seg,
    );
    seg
}

/// Decode a compressed segment back to f32.
fn decode_block(seg: &[u8]) -> Vec<f32> {
    let mut out = Vec::new();
    segment::decode(seg, &mut out);
    out
}

#[inline]
fn read_u32_le(bytes: &[u8], off: &mut usize) -> u32 {
    let o = *off;
    let arr = [bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]];
    *off = o + 4;
    u32::from_le_bytes(arr)
}

#[inline]
fn read_f32_le(bytes: &[u8], off: &mut usize) -> f32 {
    f32::from_bits(read_u32_le(bytes, off))
}

#[inline]
fn write_u32_le(buf: &mut [u8], off: &mut usize, v: u32) {
    buf[*off..*off + 4].copy_from_slice(&v.to_le_bytes());
    *off += 4;
}

#[inline]
fn write_u64_le(buf: &mut [u8], off: &mut usize, v: u64) {
    buf[*off..*off + 8].copy_from_slice(&v.to_le_bytes());
    *off += 8;
}

/// Stats binary layout (36 bytes, little-endian):
/// ```text
/// [block_count:u32][hot:u32][warm:u32][cool:u32][cold:u32]
/// [total_bytes:u64][tick_count:u64]
/// ```
const STATS_SIZE: usize = 5 * 4 + 2 * 8;

// ── FFI exports ──────────────────────────────────────────────────────

/// Initialize the temporal tensor store with a serialized config.
/// If `policy_ptr` is null or `policy_len` is 0, uses `TierConfig::default()`.
/// Returns 0 on success, negative on error.
#[no_mangle]
pub extern "C" fn tts_init(policy_ptr: *const u8, policy_len: usize) -> i32 {
    let config = if policy_ptr.is_null() || policy_len == 0 {
        TierConfig::default()
    } else {
        let bytes = unsafe { std::slice::from_raw_parts(policy_ptr, policy_len) };
        match TierConfig::from_bytes(bytes) {
            Some(c) => c,
            None => return ERR_INVALID_CONFIG,
        }
    };

    unsafe {
        STORE_STATE = Some(StoreState {
            store: TieredStore::new(),
            config,
            tick_count: 0,
        });
    }
    0
}

/// Store a tensor block.  Quantizes according to the block's current tier
/// (or Hot for new blocks).  `tensor_id` is split into hi/lo because WASM
/// does not support u128.
/// Returns 0 on success, negative on error.
#[no_mangle]
pub extern "C" fn tts_put(
    tensor_id_hi: u64,
    tensor_id_lo: u64,
    block_index: u32,
    data_ptr: *const f32,
    data_len: usize,
) -> i32 {
    if data_ptr.is_null() {
        return ERR_NULL_POINTER;
    }
    if data_len == 0 {
        return ERR_EMPTY_DATA;
    }

    let data = unsafe { std::slice::from_raw_parts(data_ptr, data_len) };
    let key = BlockKey {
        tensor_id: make_tensor_id(tensor_id_hi, tensor_id_lo),
        block_index,
    };

    with_state(|state| {
        let tier = state
            .store
            .blocks
            .get(&key)
            .map(|(m, _)| m.tier)
            .unwrap_or(Tier::Hot);

        let seg = encode_block(data, tier);
        let meta = BlockMeta {
            tier,
            access_count: 1,
            last_access_ts: state.tick_count,
            ema_score: 1.0,
            element_count: data_len,
        };
        state.store.blocks.insert(key, (meta, seg));
        0
    })
}

/// Read a tensor block, dequantized to f32.
/// Returns the number of f32 elements written, or negative on error.
#[no_mangle]
pub extern "C" fn tts_get(
    tensor_id_hi: u64,
    tensor_id_lo: u64,
    block_index: u32,
    out_ptr: *mut f32,
    out_len: usize,
) -> i32 {
    if out_ptr.is_null() {
        return ERR_NULL_POINTER;
    }

    let key = BlockKey {
        tensor_id: make_tensor_id(tensor_id_hi, tensor_id_lo),
        block_index,
    };

    with_state(|state| match state.store.blocks.get(&key) {
        None => ERR_BLOCK_NOT_FOUND,
        Some((_meta, seg)) => {
            let decoded = decode_block(seg);
            if decoded.len() > out_len {
                return ERR_BUFFER_TOO_SMALL;
            }
            let out = unsafe { std::slice::from_raw_parts_mut(out_ptr, out_len) };
            out[..decoded.len()].copy_from_slice(&decoded);
            decoded.len() as i32
        }
    })
}

/// Run a maintenance tick with byte and operation budgets.
/// Re-scores every block and migrates those whose tier has changed,
/// subject to hysteresis.
/// Returns number of migration operations performed, or negative on error.
#[no_mangle]
pub extern "C" fn tts_tick(budget_bytes: u32, budget_ops: u32) -> i32 {
    with_state(|state| {
        state.tick_count += 1;
        let tick = state.tick_count;

        // Snapshot keys and scores so we can mutate blocks afterwards.
        let entries: Vec<(BlockKey, f32)> = state
            .store
            .blocks
            .iter()
            .map(|(k, (m, _))| (*k, compute_score(&state.config, m, tick)))
            .collect();

        let mut ops = 0u32;
        let mut bytes_used = 0u32;

        for (key, score) in entries {
            if ops >= budget_ops || bytes_used >= budget_bytes {
                break;
            }

            if let Some((meta, seg)) = state.store.blocks.get_mut(&key) {
                let new_tier = choose_tier(&state.config, score);

                let current_threshold = match meta.tier {
                    Tier::Hot => state.config.t1,
                    Tier::Warm => state.config.t2,
                    Tier::Cool => state.config.t3,
                    Tier::Cold => 0.0,
                };
                let needs_change = new_tier != meta.tier
                    && (score - current_threshold).abs() > state.config.hysteresis;

                if needs_change {
                    let decoded = decode_block(seg);
                    if !decoded.is_empty() {
                        let new_seg = encode_block(&decoded, new_tier);
                        bytes_used = bytes_used.saturating_add(new_seg.len() as u32);
                        *seg = new_seg;
                        meta.tier = new_tier;
                        ops += 1;
                    }
                }

                // Update EMA for every block regardless of migration.
                meta.ema_score =
                    state.config.alpha * score + (1.0 - state.config.alpha) * meta.ema_score;
            }
        }

        ops as i32
    })
}

/// Write a statistics snapshot to `out_ptr`.
/// Returns number of bytes written, or negative on error.
#[no_mangle]
pub extern "C" fn tts_stats(out_ptr: *mut u8, out_len: usize) -> i32 {
    if out_ptr.is_null() {
        return ERR_NULL_POINTER;
    }
    if out_len < STATS_SIZE {
        return ERR_BUFFER_TOO_SMALL;
    }

    with_state(|state| {
        let out = unsafe { std::slice::from_raw_parts_mut(out_ptr, out_len) };
        let mut off = 0usize;

        write_u32_le(out, &mut off, state.store.block_count() as u32);
        write_u32_le(out, &mut off, state.store.tier_count(Tier::Hot) as u32);
        write_u32_le(out, &mut off, state.store.tier_count(Tier::Warm) as u32);
        write_u32_le(out, &mut off, state.store.tier_count(Tier::Cool) as u32);
        write_u32_le(out, &mut off, state.store.tier_count(Tier::Cold) as u32);
        write_u64_le(out, &mut off, state.store.total_bytes() as u64);
        write_u64_le(out, &mut off, state.tick_count);

        STATS_SIZE as i32
    })
}

/// Record an access event for a block (increments count, updates timestamp).
/// Returns 0 on success, negative on error.
#[no_mangle]
pub extern "C" fn tts_touch(tensor_id_hi: u64, tensor_id_lo: u64, block_index: u32) -> i32 {
    let key = BlockKey {
        tensor_id: make_tensor_id(tensor_id_hi, tensor_id_lo),
        block_index,
    };

    with_state(|state| match state.store.blocks.get_mut(&key) {
        None => ERR_BLOCK_NOT_FOUND,
        Some((meta, _)) => {
            meta.access_count = meta.access_count.saturating_add(1);
            meta.last_access_ts = state.tick_count;
            0
        }
    })
}

/// Evict a block, removing it from the store entirely.
/// Returns 0 on success, negative on error.
#[no_mangle]
pub extern "C" fn tts_evict(tensor_id_hi: u64, tensor_id_lo: u64, block_index: u32) -> i32 {
    let key = BlockKey {
        tensor_id: make_tensor_id(tensor_id_hi, tensor_id_lo),
        block_index,
    };

    with_state(|state| match state.store.blocks.remove(&key) {
        None => ERR_BLOCK_NOT_FOUND,
        Some(_) => 0,
    })
}

/// Get total number of blocks in the store.
#[no_mangle]
pub extern "C" fn tts_block_count() -> i32 {
    with_state(|state| state.store.block_count() as i32)
}

/// Get number of blocks in a specific tier (0=Hot, 1=Warm, 2=Cool, 3=Cold).
#[no_mangle]
pub extern "C" fn tts_tier_count(tier: u8) -> i32 {
    match Tier::from_u8(tier) {
        Some(t) => with_state(|state| state.store.tier_count(t) as i32),
        None => ERR_INVALID_CONFIG,
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Reset global state before each test.
    fn reset() {
        unsafe {
            STORE_STATE = None;
        }
    }

    /// Build a binary config buffer from the default TierConfig.
    fn default_config_bytes() -> Vec<u8> {
        let c = TierConfig::default();
        let mut buf = Vec::with_capacity(CONFIG_BINARY_LEN);
        buf.extend_from_slice(&c.block_bytes.to_le_bytes());
        buf.extend_from_slice(&c.alpha.to_bits().to_le_bytes());
        buf.extend_from_slice(&c.tau.to_bits().to_le_bytes());
        buf.extend_from_slice(&c.w_ema.to_bits().to_le_bytes());
        buf.extend_from_slice(&c.w_pop.to_bits().to_le_bytes());
        buf.extend_from_slice(&c.w_rec.to_bits().to_le_bytes());
        buf.extend_from_slice(&c.t1.to_bits().to_le_bytes());
        buf.extend_from_slice(&c.t2.to_bits().to_le_bytes());
        buf.extend_from_slice(&c.t3.to_bits().to_le_bytes());
        buf.extend_from_slice(&c.hysteresis.to_bits().to_le_bytes());
        buf.extend_from_slice(&c.min_residency.to_le_bytes());
        buf.push(c.max_delta_chain);
        buf
    }

    #[test]
    fn test_init_default() {
        reset();
        let rc = tts_init(std::ptr::null(), 0);
        assert_eq!(rc, 0);
        assert_eq!(tts_block_count(), 0);
    }

    #[test]
    fn test_init_with_config() {
        reset();
        let cfg = default_config_bytes();
        let rc = tts_init(cfg.as_ptr(), cfg.len());
        assert_eq!(rc, 0);
        assert_eq!(tts_block_count(), 0);
    }

    #[test]
    fn test_init_invalid_config_too_short() {
        reset();
        let buf = [0u8; 10];
        let rc = tts_init(buf.as_ptr(), buf.len());
        assert_eq!(rc, ERR_INVALID_CONFIG);
    }

    #[test]
    fn test_put_get_roundtrip() {
        reset();
        tts_init(std::ptr::null(), 0);

        let data: Vec<f32> = (0..64).map(|i| (i as f32 - 32.0) * 0.1).collect();
        let rc = tts_put(0, 1, 0, data.as_ptr(), data.len());
        assert_eq!(rc, 0);

        let mut out = vec![0.0f32; 64];
        let n = tts_get(0, 1, 0, out.as_mut_ptr(), out.len());
        assert_eq!(n, 64);

        // 8-bit quantization: expect low error.
        let max_abs = data.iter().map(|v| v.abs()).fold(0.0f32, f32::max);
        for (i, (&orig, &dec)) in data.iter().zip(out.iter()).enumerate() {
            let err = (orig - dec).abs();
            assert!(
                err < max_abs * 0.05,
                "i={i} orig={orig} dec={dec} err={err}"
            );
        }
    }

    #[test]
    fn test_put_null_pointer() {
        reset();
        tts_init(std::ptr::null(), 0);
        let rc = tts_put(0, 1, 0, std::ptr::null(), 64);
        assert_eq!(rc, ERR_NULL_POINTER);
    }

    #[test]
    fn test_put_empty_data() {
        reset();
        tts_init(std::ptr::null(), 0);
        let data = [1.0f32; 1];
        let rc = tts_put(0, 1, 0, data.as_ptr(), 0);
        assert_eq!(rc, ERR_EMPTY_DATA);
    }

    #[test]
    fn test_get_not_found() {
        reset();
        tts_init(std::ptr::null(), 0);
        let mut out = vec![0.0f32; 64];
        let rc = tts_get(0, 99, 0, out.as_mut_ptr(), out.len());
        assert_eq!(rc, ERR_BLOCK_NOT_FOUND);
    }

    #[test]
    fn test_get_null_pointer() {
        reset();
        tts_init(std::ptr::null(), 0);
        let rc = tts_get(0, 1, 0, std::ptr::null_mut(), 64);
        assert_eq!(rc, ERR_NULL_POINTER);
    }

    #[test]
    fn test_get_buffer_too_small() {
        reset();
        tts_init(std::ptr::null(), 0);

        let data = vec![1.0f32; 64];
        tts_put(0, 1, 0, data.as_ptr(), data.len());

        let mut out = vec![0.0f32; 2]; // too small
        let rc = tts_get(0, 1, 0, out.as_mut_ptr(), out.len());
        assert_eq!(rc, ERR_BUFFER_TOO_SMALL);
    }

    #[test]
    fn test_block_count_after_puts() {
        reset();
        tts_init(std::ptr::null(), 0);

        let data = vec![1.0f32; 64];
        tts_put(0, 1, 0, data.as_ptr(), data.len());
        tts_put(0, 1, 1, data.as_ptr(), data.len());
        tts_put(0, 2, 0, data.as_ptr(), data.len());

        assert_eq!(tts_block_count(), 3);
    }

    #[test]
    fn test_tier_count_initial() {
        reset();
        tts_init(std::ptr::null(), 0);

        let data = vec![1.0f32; 64];
        tts_put(0, 1, 0, data.as_ptr(), data.len());
        tts_put(0, 1, 1, data.as_ptr(), data.len());

        // New blocks default to Hot.
        assert_eq!(tts_tier_count(0), 2); // Hot
        assert_eq!(tts_tier_count(1), 0); // Warm
        assert_eq!(tts_tier_count(2), 0); // Cool
        assert_eq!(tts_tier_count(3), 0); // Cold
    }

    #[test]
    fn test_tier_count_invalid_tier() {
        reset();
        tts_init(std::ptr::null(), 0);
        assert_eq!(tts_tier_count(99), ERR_INVALID_CONFIG);
    }

    #[test]
    fn test_touch() {
        reset();
        tts_init(std::ptr::null(), 0);

        let data = vec![1.0f32; 64];
        tts_put(0, 1, 0, data.as_ptr(), data.len());

        let rc = tts_touch(0, 1, 0);
        assert_eq!(rc, 0);

        // Touch a non-existent block.
        let rc = tts_touch(0, 99, 0);
        assert_eq!(rc, ERR_BLOCK_NOT_FOUND);
    }

    #[test]
    fn test_evict() {
        reset();
        tts_init(std::ptr::null(), 0);

        let data = vec![1.0f32; 64];
        tts_put(0, 1, 0, data.as_ptr(), data.len());
        assert_eq!(tts_block_count(), 1);

        let rc = tts_evict(0, 1, 0);
        assert_eq!(rc, 0);
        assert_eq!(tts_block_count(), 0);

        // Evict again should fail.
        let rc = tts_evict(0, 1, 0);
        assert_eq!(rc, ERR_BLOCK_NOT_FOUND);
    }

    #[test]
    fn test_tick_does_not_crash() {
        reset();
        tts_init(std::ptr::null(), 0);

        let data = vec![1.0f32; 64];
        tts_put(0, 1, 0, data.as_ptr(), data.len());
        tts_put(0, 1, 1, data.as_ptr(), data.len());

        // Run several ticks with generous budgets.
        for _ in 0..10 {
            let ops = tts_tick(1_000_000, 1000);
            assert!(ops >= 0);
        }

        // Blocks should still be readable.
        let mut out = vec![0.0f32; 64];
        let n = tts_get(0, 1, 0, out.as_mut_ptr(), out.len());
        assert!(n > 0);
    }

    #[test]
    fn test_tick_with_zero_budget() {
        reset();
        tts_init(std::ptr::null(), 0);

        let data = vec![1.0f32; 64];
        tts_put(0, 1, 0, data.as_ptr(), data.len());

        let ops = tts_tick(0, 0);
        assert_eq!(ops, 0);
    }

    #[test]
    fn test_stats_returns_valid_data() {
        reset();
        tts_init(std::ptr::null(), 0);

        let data = vec![1.0f32; 64];
        tts_put(0, 1, 0, data.as_ptr(), data.len());
        tts_put(0, 1, 1, data.as_ptr(), data.len());

        let mut buf = vec![0u8; STATS_SIZE];
        let written = tts_stats(buf.as_mut_ptr(), buf.len());
        assert_eq!(written, STATS_SIZE as i32);

        // Parse the stats back.
        let mut off = 0usize;
        let block_count = read_u32_le(&buf, &mut off);
        let hot = read_u32_le(&buf, &mut off);
        let warm = read_u32_le(&buf, &mut off);
        let cool = read_u32_le(&buf, &mut off);
        let cold = read_u32_le(&buf, &mut off);

        assert_eq!(block_count, 2);
        assert_eq!(hot, 2);
        assert_eq!(warm, 0);
        assert_eq!(cool, 0);
        assert_eq!(cold, 0);
    }

    #[test]
    fn test_stats_null_pointer() {
        reset();
        tts_init(std::ptr::null(), 0);
        let rc = tts_stats(std::ptr::null_mut(), 64);
        assert_eq!(rc, ERR_NULL_POINTER);
    }

    #[test]
    fn test_stats_buffer_too_small() {
        reset();
        tts_init(std::ptr::null(), 0);
        let mut buf = vec![0u8; 4]; // too small
        let rc = tts_stats(buf.as_mut_ptr(), buf.len());
        assert_eq!(rc, ERR_BUFFER_TOO_SMALL);
    }

    #[test]
    fn test_make_tensor_id() {
        assert_eq!(make_tensor_id(0, 0), 0u128);
        assert_eq!(make_tensor_id(0, 1), 1u128);
        assert_eq!(make_tensor_id(1, 0), 1u128 << 64);
        assert_eq!(make_tensor_id(u64::MAX, u64::MAX), u128::MAX,);
    }

    #[test]
    fn test_multiple_tensor_ids() {
        reset();
        tts_init(std::ptr::null(), 0);

        let data = vec![1.0f32; 64];
        tts_put(0, 1, 0, data.as_ptr(), data.len());
        tts_put(0, 2, 0, data.as_ptr(), data.len());
        tts_put(1, 0, 0, data.as_ptr(), data.len());

        assert_eq!(tts_block_count(), 3);

        // Each should be independently readable.
        let mut out = vec![0.0f32; 64];
        assert!(tts_get(0, 1, 0, out.as_mut_ptr(), out.len()) > 0);
        assert!(tts_get(0, 2, 0, out.as_mut_ptr(), out.len()) > 0);
        assert!(tts_get(1, 0, 0, out.as_mut_ptr(), out.len()) > 0);
    }

    #[test]
    fn test_overwrite_block() {
        reset();
        tts_init(std::ptr::null(), 0);

        let data1 = vec![1.0f32; 64];
        tts_put(0, 1, 0, data1.as_ptr(), data1.len());

        let data2 = vec![2.0f32; 64];
        tts_put(0, 1, 0, data2.as_ptr(), data2.len());

        assert_eq!(tts_block_count(), 1);

        // Should read back the second write.
        let mut out = vec![0.0f32; 64];
        let n = tts_get(0, 1, 0, out.as_mut_ptr(), out.len());
        assert_eq!(n, 64);
        for &v in &out {
            assert!((v - 2.0).abs() < 0.1);
        }
    }
}
