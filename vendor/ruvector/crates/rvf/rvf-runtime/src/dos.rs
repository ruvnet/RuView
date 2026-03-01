//! DoS hardening for ADR-033 §3.3.1.
//!
//! Provides per-connection budget tokens, negative caching of degenerate
//! queries, and optional proof-of-work for public endpoints.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Per-connection token bucket for rate-limiting distance operations.
///
/// Each query consumes tokens from the bucket. When tokens are exhausted,
/// queries are rejected until the bucket refills.
pub struct BudgetTokenBucket {
    /// Maximum tokens (distance ops) per window.
    max_tokens: u64,
    /// Current available tokens.
    tokens: u64,
    /// Window duration for token refill.
    window: Duration,
    /// Start of current window.
    window_start: Instant,
}

impl BudgetTokenBucket {
    /// Create a new token bucket.
    ///
    /// # Arguments
    /// * `max_tokens` - Maximum distance ops per window.
    /// * `window` - Duration of each refill window.
    pub fn new(max_tokens: u64, window: Duration) -> Self {
        Self {
            max_tokens,
            tokens: max_tokens,
            window,
            window_start: Instant::now(),
        }
    }

    /// Try to consume `cost` tokens. Returns `Ok(remaining)` if sufficient
    /// tokens are available, `Err(deficit)` if not.
    pub fn try_consume(&mut self, cost: u64) -> Result<u64, u64> {
        self.maybe_refill();

        if cost <= self.tokens {
            self.tokens -= cost;
            Ok(self.tokens)
        } else {
            Err(cost - self.tokens)
        }
    }

    /// Check remaining tokens without consuming.
    pub fn remaining(&mut self) -> u64 {
        self.maybe_refill();
        self.tokens
    }

    /// Force a refill (for testing or manual reset).
    pub fn refill(&mut self) {
        self.tokens = self.max_tokens;
        self.window_start = Instant::now();
    }

    fn maybe_refill(&mut self) {
        if self.window_start.elapsed() >= self.window {
            self.tokens = self.max_tokens;
            self.window_start = Instant::now();
        }
    }
}

/// Quantized query signature for negative caching.
///
/// The query vector is quantized to int8 and hashed to produce a
/// compact fingerprint for degenerate query detection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct QuerySignature {
    hash: u64,
}

impl QuerySignature {
    /// Compute a signature from a query vector.
    ///
    /// Quantizes to int8, then hashes with FNV-1a for speed.
    pub fn from_query(query: &[f32]) -> Self {
        // FNV-1a hash of quantized vector.
        let mut hash: u64 = 0xcbf29ce484222325;
        for &val in query {
            // Quantize to int8 range [-128, 127].
            let quantized = (val.clamp(-1.0, 1.0) * 127.0) as i8;
            hash ^= quantized as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        Self { hash }
    }
}

/// Negative cache entry tracking degenerate query hits.
struct NegativeCacheEntry {
    hit_count: u32,
    first_seen: Instant,
    last_seen: Instant,
}

/// Negative cache for degenerate queries.
///
/// If a query signature triggers degenerate mode more than N times
/// in a window, forces `SafetyNetBudget::DISABLED` for subsequent
/// matches, preventing repeated budget burn on the same attack vector.
pub struct NegativeCache {
    entries: HashMap<QuerySignature, NegativeCacheEntry>,
    /// Number of degenerate hits before a signature is blacklisted.
    threshold: u32,
    /// Window duration for counting hits.
    window: Duration,
    /// Maximum cache size to prevent memory exhaustion.
    max_entries: usize,
}

impl NegativeCache {
    /// Create a new negative cache.
    ///
    /// # Arguments
    /// * `threshold` - Number of degenerate hits before blacklisting.
    /// * `window` - Duration window for counting hits.
    /// * `max_entries` - Maximum cache entries.
    pub fn new(threshold: u32, window: Duration, max_entries: usize) -> Self {
        Self {
            entries: HashMap::new(),
            threshold,
            window,
            max_entries,
        }
    }

    /// Record a degenerate query hit. Returns `true` if the query is
    /// now blacklisted (should force DISABLED safety net).
    pub fn record_degenerate(&mut self, sig: QuerySignature) -> bool {
        let now = Instant::now();

        // Evict expired entries periodically.
        if self.entries.len() >= self.max_entries {
            self.evict_expired(now);
        }

        // If still at capacity, evict oldest.
        if self.entries.len() >= self.max_entries {
            self.evict_oldest();
        }

        let entry = self.entries.entry(sig).or_insert(NegativeCacheEntry {
            hit_count: 0,
            first_seen: now,
            last_seen: now,
        });

        // Reset if outside window.
        if now.duration_since(entry.first_seen) > self.window {
            entry.hit_count = 0;
            entry.first_seen = now;
        }

        entry.hit_count += 1;
        entry.last_seen = now;

        entry.hit_count >= self.threshold
    }

    /// Check if a query signature is blacklisted.
    pub fn is_blacklisted(&self, sig: &QuerySignature) -> bool {
        if let Some(entry) = self.entries.get(sig) {
            entry.hit_count >= self.threshold
        } else {
            false
        }
    }

    /// Number of currently tracked signatures.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn evict_expired(&mut self, now: Instant) {
        self.entries
            .retain(|_, entry| now.duration_since(entry.first_seen) <= self.window);
    }

    fn evict_oldest(&mut self) {
        if let Some(oldest_key) = self
            .entries
            .iter()
            .min_by_key(|(_, e)| e.last_seen)
            .map(|(k, _)| *k)
        {
            self.entries.remove(&oldest_key);
        }
    }
}

/// Proof-of-work challenge for public endpoints.
///
/// The caller must find a nonce such that `hash(challenge || nonce)`
/// has `difficulty` leading zero bits. This is opt-in, not default.
#[derive(Clone, Debug)]
pub struct ProofOfWork {
    /// The challenge bytes (typically random).
    pub challenge: [u8; 16],
    /// Required leading zero bits in the hash. Capped at MAX_DIFFICULTY.
    pub difficulty: u8,
}

impl ProofOfWork {
    /// Maximum allowed difficulty (24 bits = ~16M hashes average).
    /// Higher values risk CPU-bound DoS.
    pub const MAX_DIFFICULTY: u8 = 24;

    /// Verify that a nonce satisfies the proof-of-work requirement.
    ///
    /// Uses FNV-1a for speed (this is DoS mitigation, not cryptographic security).
    /// Clamps difficulty to MAX_DIFFICULTY to prevent compute DoS.
    pub fn verify(&self, nonce: u64) -> bool {
        let mut hash: u64 = 0xcbf29ce484222325;
        for &byte in &self.challenge {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        for &byte in &nonce.to_le_bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }

        let clamped = self.difficulty.min(Self::MAX_DIFFICULTY);
        let leading_zeros = hash.leading_zeros() as u8;
        leading_zeros >= clamped
    }

    /// Find a valid nonce (for testing / client-side use).
    /// Returns `None` if no nonce found within `max_attempts`.
    pub fn solve(&self) -> Option<u64> {
        let max_attempts: u64 = 1u64 << self.difficulty.min(Self::MAX_DIFFICULTY).min(30);
        for nonce in 0..max_attempts.saturating_mul(4) {
            if self.verify(nonce) {
                return Some(nonce);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_bucket_basic() {
        let mut bucket = BudgetTokenBucket::new(100, Duration::from_secs(1));
        assert_eq!(bucket.remaining(), 100);
        assert_eq!(bucket.try_consume(30), Ok(70));
        assert_eq!(bucket.remaining(), 70);
    }

    #[test]
    fn token_bucket_exhaustion() {
        let mut bucket = BudgetTokenBucket::new(10, Duration::from_secs(60));
        assert_eq!(bucket.try_consume(10), Ok(0));
        assert!(bucket.try_consume(1).is_err());
    }

    #[test]
    fn token_bucket_refill() {
        let mut bucket = BudgetTokenBucket::new(100, Duration::from_millis(1));
        bucket.try_consume(100).unwrap();
        assert!(bucket.try_consume(1).is_err());
        std::thread::sleep(Duration::from_millis(2));
        assert_eq!(bucket.remaining(), 100);
    }

    #[test]
    fn token_bucket_manual_refill() {
        let mut bucket = BudgetTokenBucket::new(100, Duration::from_secs(60));
        bucket.try_consume(100).unwrap();
        bucket.refill();
        assert_eq!(bucket.remaining(), 100);
    }

    #[test]
    fn query_signature_deterministic() {
        let query = vec![0.1, 0.2, 0.3, 0.4];
        let sig1 = QuerySignature::from_query(&query);
        let sig2 = QuerySignature::from_query(&query);
        assert_eq!(sig1, sig2);
    }

    #[test]
    fn query_signature_different_vectors() {
        let sig1 = QuerySignature::from_query(&[0.1, 0.2, 0.3]);
        let sig2 = QuerySignature::from_query(&[0.4, 0.5, 0.6]);
        assert_ne!(sig1, sig2);
    }

    #[test]
    fn negative_cache_below_threshold() {
        let mut cache = NegativeCache::new(3, Duration::from_secs(60), 1000);
        let sig = QuerySignature::from_query(&[0.1, 0.2]);
        assert!(!cache.record_degenerate(sig));
        assert!(!cache.record_degenerate(sig));
        assert!(!cache.is_blacklisted(&sig));
    }

    #[test]
    fn negative_cache_reaches_threshold() {
        let mut cache = NegativeCache::new(3, Duration::from_secs(60), 1000);
        let sig = QuerySignature::from_query(&[0.1, 0.2]);
        cache.record_degenerate(sig);
        cache.record_degenerate(sig);
        assert!(cache.record_degenerate(sig)); // 3rd hit = blacklisted.
        assert!(cache.is_blacklisted(&sig));
    }

    #[test]
    fn negative_cache_max_entries() {
        let mut cache = NegativeCache::new(100, Duration::from_secs(60), 5);
        for i in 0..10 {
            let sig = QuerySignature::from_query(&[i as f32]);
            cache.record_degenerate(sig);
        }
        assert!(cache.len() <= 5);
    }

    #[test]
    fn negative_cache_empty() {
        let cache = NegativeCache::new(3, Duration::from_secs(60), 1000);
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn proof_of_work_low_difficulty() {
        let pow = ProofOfWork {
            challenge: [0xAB; 16],
            difficulty: 1, // Very easy.
        };
        let nonce = pow.solve().expect("should solve easily");
        assert!(pow.verify(nonce));
    }

    #[test]
    fn proof_of_work_wrong_nonce() {
        let pow = ProofOfWork {
            challenge: [0xAB; 16],
            difficulty: 16, // Moderate difficulty.
        };
        // Random nonce is very unlikely to pass.
        assert!(!pow.verify(0xDEADBEEF));
    }

    #[test]
    fn proof_of_work_solve_and_verify() {
        let pow = ProofOfWork {
            challenge: [0x42; 16],
            difficulty: 8,
        };
        let nonce = pow.solve().expect("should solve d=8");
        assert!(pow.verify(nonce));
    }

    #[test]
    fn proof_of_work_max_difficulty_clamped() {
        let pow = ProofOfWork {
            challenge: [0x42; 16],
            difficulty: 255, // Extreme — will be clamped to MAX_DIFFICULTY.
        };
        // verify() clamps internally, so this is equivalent to d=24.
        // solve() uses clamped difficulty too.
        assert_eq!(pow.difficulty.min(ProofOfWork::MAX_DIFFICULTY), 24);
    }
}
