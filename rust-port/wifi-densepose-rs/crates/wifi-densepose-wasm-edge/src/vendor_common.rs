//! Shared types and utilities for vendor-integrated WASM modules (ADR-041).
//!
//! All structures are `no_std`, `const`-constructible, and heap-free.
//! Designed for reuse across the 24 vendor-integrated modules
//! (signal intelligence, adaptive learning, spatial reasoning,
//! temporal analysis, AI security, quantum-inspired, autonomous).

use libm::{fabsf, sqrtf};

// ---- VendorModuleState trait -------------------------------------------------

/// Lifecycle trait for vendor-integrated modules.
///
/// Every vendor module implements this trait so that the combined pipeline
/// can uniformly initialise, process frames, and run periodic timers.
pub trait VendorModuleState {
    /// Called once when the WASM module is loaded.
    fn init(&mut self);

    /// Called per CSI frame (~20 Hz).
    /// `n_subcarriers` is the number of valid subcarriers in this frame.
    fn process(&mut self, n_subcarriers: usize);

    /// Called at a configurable interval (default 1 s).
    fn timer(&mut self);
}

// ---- CircularBuffer ----------------------------------------------------------

/// Fixed-size circular buffer for phase history and other rolling data.
///
/// `N` is the maximum capacity. All storage is on the stack (or WASM linear
/// memory). Const-constructible with `CircularBuffer::new()`.
pub struct CircularBuffer<const N: usize> {
    buf: [f32; N],
    head: usize,
    len: usize,
}

impl<const N: usize> CircularBuffer<N> {
    /// Create an empty circular buffer.
    pub const fn new() -> Self {
        Self {
            buf: [0.0; N],
            head: 0,
            len: 0,
        }
    }

    /// Push a value. Overwrites the oldest entry when full.
    pub fn push(&mut self, value: f32) {
        self.buf[self.head] = value;
        self.head = (self.head + 1) % N;
        if self.len < N {
            self.len += 1;
        }
    }

    /// Number of values currently stored.
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Whether the buffer is at capacity.
    pub const fn is_full(&self) -> bool {
        self.len == N
    }

    /// Read the i-th oldest element (0 = oldest, len-1 = newest).
    /// Returns 0.0 if `i >= len`.
    pub fn get(&self, i: usize) -> f32 {
        if i >= self.len {
            return 0.0;
        }
        // oldest is at (head + N - len) % N
        let idx = (self.head + N - self.len + i) % N;
        self.buf[idx]
    }

    /// Read the most recent value. Returns 0.0 if empty.
    pub fn latest(&self) -> f32 {
        if self.len == 0 {
            return 0.0;
        }
        let idx = (self.head + N - 1) % N;
        self.buf[idx]
    }

    /// Copy up to `out.len()` of the most recent values into `out` (oldest first).
    /// Returns the number of values copied.
    pub fn copy_recent(&self, out: &mut [f32]) -> usize {
        let count = if out.len() < self.len { out.len() } else { self.len };
        let start = self.len - count;
        for i in 0..count {
            out[i] = self.get(start + i);
        }
        count
    }

    /// Clear all data.
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }

    /// Capacity of the buffer.
    pub const fn capacity(&self) -> usize {
        N
    }
}

// ---- EMA (Exponential Moving Average) ----------------------------------------

/// Exponential Moving Average with configurable smoothing factor.
///
/// `value = alpha * sample + (1 - alpha) * value`
///
/// Const-constructible. Set `alpha` in `[0.0, 1.0]`.
pub struct Ema {
    /// Current smoothed value.
    pub value: f32,
    /// Smoothing factor (0 = no update, 1 = no smoothing).
    alpha: f32,
    /// Whether the first sample has been received.
    initialized: bool,
}

impl Ema {
    /// Create a new EMA with the given smoothing factor.
    pub const fn new(alpha: f32) -> Self {
        Self {
            value: 0.0,
            alpha,
            initialized: false,
        }
    }

    /// Create a new EMA with an initial seed value.
    pub const fn with_initial(alpha: f32, initial: f32) -> Self {
        Self {
            value: initial,
            alpha,
            initialized: true,
        }
    }

    /// Feed a new sample and return the updated smoothed value.
    pub fn update(&mut self, sample: f32) -> f32 {
        if !self.initialized {
            self.value = sample;
            self.initialized = true;
        } else {
            self.value = self.alpha * sample + (1.0 - self.alpha) * self.value;
        }
        self.value
    }

    /// Reset to uninitialised state.
    pub fn reset(&mut self) {
        self.value = 0.0;
        self.initialized = false;
    }

    /// Whether any sample has been fed.
    pub const fn is_initialized(&self) -> bool {
        self.initialized
    }
}

// ---- WelfordStats (online mean / variance / std) -----------------------------

/// Welford online statistics: computes running mean, variance, and standard
/// deviation in a single pass with O(1) memory.
pub struct WelfordStats {
    count: u32,
    mean: f32,
    m2: f32,
}

impl WelfordStats {
    pub const fn new() -> Self {
        Self {
            count: 0,
            mean: 0.0,
            m2: 0.0,
        }
    }

    /// Feed a new sample.
    pub fn update(&mut self, x: f32) {
        self.count += 1;
        let delta = x - self.mean;
        self.mean += delta / (self.count as f32);
        let delta2 = x - self.mean;
        self.m2 += delta * delta2;
    }

    /// Current mean.
    pub const fn mean(&self) -> f32 {
        self.mean
    }

    /// Population variance (biased).
    pub fn variance(&self) -> f32 {
        if self.count < 2 {
            return 0.0;
        }
        self.m2 / (self.count as f32)
    }

    /// Sample variance (unbiased). Returns 0.0 if fewer than 2 samples.
    pub fn sample_variance(&self) -> f32 {
        if self.count < 2 {
            return 0.0;
        }
        self.m2 / ((self.count - 1) as f32)
    }

    /// Population standard deviation.
    pub fn std_dev(&self) -> f32 {
        sqrtf(self.variance())
    }

    /// Number of samples ingested.
    pub const fn count(&self) -> u32 {
        self.count
    }

    /// Reset all statistics.
    pub fn reset(&mut self) {
        self.count = 0;
        self.mean = 0.0;
        self.m2 = 0.0;
    }
}

// ---- Fixed-size vector math helpers ------------------------------------------

/// Dot product of two slices (up to `min(a.len(), b.len())` elements).
pub fn dot_product(a: &[f32], b: &[f32]) -> f32 {
    let n = if a.len() < b.len() { a.len() } else { b.len() };
    let mut sum = 0.0f32;
    for i in 0..n {
        sum += a[i] * b[i];
    }
    sum
}

/// L2 (Euclidean) norm of a slice.
pub fn l2_norm(a: &[f32]) -> f32 {
    let mut sum = 0.0f32;
    for i in 0..a.len() {
        sum += a[i] * a[i];
    }
    sqrtf(sum)
}

/// Cosine similarity in `[-1, 1]`. Returns 0.0 if either vector has zero norm.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot = dot_product(a, b);
    let na = l2_norm(a);
    let nb = l2_norm(b);
    let denom = na * nb;
    if denom < 1e-12 {
        return 0.0;
    }
    dot / denom
}

/// Squared Euclidean distance between two slices.
pub fn l2_distance_sq(a: &[f32], b: &[f32]) -> f32 {
    let n = if a.len() < b.len() { a.len() } else { b.len() };
    let mut sum = 0.0f32;
    for i in 0..n {
        let d = a[i] - b[i];
        sum += d * d;
    }
    sum
}

/// Euclidean distance between two slices.
pub fn l2_distance(a: &[f32], b: &[f32]) -> f32 {
    sqrtf(l2_distance_sq(a, b))
}

// ---- DTW (Dynamic Time Warping) for small sequences --------------------------

/// Maximum sequence length for DTW. Keeps stack usage under 16 KiB
/// (64 * 64 * 4 bytes = 16,384 bytes).
pub const DTW_MAX_LEN: usize = 64;

/// Compute Dynamic Time Warping distance between two sequences.
///
/// Both `a` and `b` must have length <= `DTW_MAX_LEN`.
/// Uses a full cost matrix on the stack. Returns `f32::MAX` on empty input.
/// Result is normalised by path length `(a.len() + b.len())`.
pub fn dtw_distance(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len();
    let m = b.len();

    if n == 0 || m == 0 || n > DTW_MAX_LEN || m > DTW_MAX_LEN {
        return f32::MAX;
    }

    let mut cost = [[f32::MAX; DTW_MAX_LEN]; DTW_MAX_LEN];
    cost[0][0] = fabsf(a[0] - b[0]);

    for i in 0..n {
        for j in 0..m {
            let c = fabsf(a[i] - b[j]);
            if i == 0 && j == 0 {
                cost[0][0] = c;
            } else {
                let mut prev = f32::MAX;
                if i > 0 && cost[i - 1][j] < prev {
                    prev = cost[i - 1][j];
                }
                if j > 0 && cost[i][j - 1] < prev {
                    prev = cost[i][j - 1];
                }
                if i > 0 && j > 0 && cost[i - 1][j - 1] < prev {
                    prev = cost[i - 1][j - 1];
                }
                cost[i][j] = c + prev;
            }
        }
    }

    cost[n - 1][m - 1] / ((n + m) as f32)
}

/// Constrained DTW with Sakoe-Chiba band.
///
/// `band` limits the warping path to `|i - j| <= band`, reducing
/// computation from O(nm) to O(n * band).
pub fn dtw_distance_banded(a: &[f32], b: &[f32], band: usize) -> f32 {
    let n = a.len();
    let m = b.len();

    if n == 0 || m == 0 || n > DTW_MAX_LEN || m > DTW_MAX_LEN {
        return f32::MAX;
    }

    let mut cost = [[f32::MAX; DTW_MAX_LEN]; DTW_MAX_LEN];
    cost[0][0] = fabsf(a[0] - b[0]);

    for i in 0..n {
        for j in 0..m {
            let diff = if i > j { i - j } else { j - i };
            if diff > band {
                continue;
            }
            let c = fabsf(a[i] - b[j]);
            if i == 0 && j == 0 {
                cost[0][0] = c;
            } else {
                let mut prev = f32::MAX;
                if i > 0 && cost[i - 1][j] < prev {
                    prev = cost[i - 1][j];
                }
                if j > 0 && cost[i][j - 1] < prev {
                    prev = cost[i][j - 1];
                }
                if i > 0 && j > 0 && cost[i - 1][j - 1] < prev {
                    prev = cost[i - 1][j - 1];
                }
                cost[i][j] = c + prev;
            }
        }
    }

    cost[n - 1][m - 1] / ((n + m) as f32)
}

// ---- FixedPriorityQueue (max-heap, fixed capacity) ---------------------------

/// Fixed-size max-priority queue for top-K selection.
///
/// Capacity is `CAP` (const generic, max 16).
/// Stores `(f32, u16)` pairs: `(score, id)`.
/// Keeps the `CAP` entries with the *highest* scores.
///
/// When the queue is full and a new entry has a score lower than the
/// current minimum, it is silently discarded.
pub struct FixedPriorityQueue<const CAP: usize> {
    scores: [f32; CAP],
    ids: [u16; CAP],
    len: usize,
}

impl<const CAP: usize> FixedPriorityQueue<CAP> {
    pub const fn new() -> Self {
        Self {
            scores: [0.0; CAP],
            ids: [0; CAP],
            len: 0,
        }
    }

    /// Insert a `(score, id)` pair. If full, replaces the minimum entry
    /// only if `score` exceeds it.
    pub fn insert(&mut self, score: f32, id: u16) {
        if self.len < CAP {
            self.scores[self.len] = score;
            self.ids[self.len] = id;
            self.len += 1;
        } else {
            // Find the minimum score in the queue.
            let mut min_idx = 0;
            let mut min_val = self.scores[0];
            for i in 1..self.len {
                if self.scores[i] < min_val {
                    min_val = self.scores[i];
                    min_idx = i;
                }
            }
            if score > min_val {
                self.scores[min_idx] = score;
                self.ids[min_idx] = id;
            }
        }
    }

    /// Number of entries.
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Whether the queue is empty.
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get the entry with the highest score. Returns `(score, id)` or `None`.
    pub fn peek_max(&self) -> Option<(f32, u16)> {
        if self.len == 0 {
            return None;
        }
        let mut max_idx = 0;
        let mut max_val = self.scores[0];
        for i in 1..self.len {
            if self.scores[i] > max_val {
                max_val = self.scores[i];
                max_idx = i;
            }
        }
        Some((self.scores[max_idx], self.ids[max_idx]))
    }

    /// Get the entry with the lowest score. Returns `(score, id)` or `None`.
    pub fn peek_min(&self) -> Option<(f32, u16)> {
        if self.len == 0 {
            return None;
        }
        let mut min_idx = 0;
        let mut min_val = self.scores[0];
        for i in 1..self.len {
            if self.scores[i] < min_val {
                min_val = self.scores[i];
                min_idx = i;
            }
        }
        Some((self.scores[min_idx], self.ids[min_idx]))
    }

    /// Get score and id at position `i` (unordered). Returns `(0.0, 0)` if OOB.
    pub fn get(&self, i: usize) -> (f32, u16) {
        if i >= self.len {
            return (0.0, 0);
        }
        (self.scores[i], self.ids[i])
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.len = 0;
    }

    /// Copy all IDs into `out` (unordered). Returns count copied.
    pub fn ids(&self, out: &mut [u16]) -> usize {
        let n = if out.len() < self.len { out.len() } else { self.len };
        for i in 0..n {
            out[i] = self.ids[i];
        }
        n
    }
}

// ---- Tests -------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn circular_buffer_basic() {
        let mut buf = CircularBuffer::<4>::new();
        assert!(buf.is_empty());
        assert_eq!(buf.len(), 0);

        buf.push(1.0);
        buf.push(2.0);
        buf.push(3.0);
        assert_eq!(buf.len(), 3);
        assert_eq!(buf.get(0), 1.0);
        assert_eq!(buf.get(2), 3.0);
        assert!((buf.latest() - 3.0).abs() < 1e-6);

        // Fill and overflow.
        buf.push(4.0);
        buf.push(5.0); // overwrites 1.0
        assert_eq!(buf.len(), 4);
        assert_eq!(buf.get(0), 2.0); // oldest is now 2.0
        assert_eq!(buf.get(3), 5.0); // newest is 5.0
    }

    #[test]
    fn circular_buffer_copy_recent() {
        let mut buf = CircularBuffer::<8>::new();
        for i in 0..6 {
            buf.push(i as f32);
        }
        let mut out = [0.0f32; 4];
        let n = buf.copy_recent(&mut out);
        assert_eq!(n, 4);
        // Oldest 4 of the 6 values: 2, 3, 4, 5
        assert_eq!(out, [2.0, 3.0, 4.0, 5.0]);
    }

    #[test]
    fn ema_basic() {
        let mut ema = Ema::new(0.5);
        assert!(!ema.is_initialized());
        let v = ema.update(10.0);
        assert!((v - 10.0).abs() < 1e-6);
        let v = ema.update(20.0);
        assert!((v - 15.0).abs() < 1e-6); // 0.5*20 + 0.5*10 = 15
    }

    #[test]
    fn welford_basic() {
        let mut w = WelfordStats::new();
        w.update(2.0);
        w.update(4.0);
        w.update(4.0);
        w.update(4.0);
        w.update(5.0);
        w.update(5.0);
        w.update(7.0);
        w.update(9.0);
        assert!((w.mean() - 5.0).abs() < 1e-4);
        // Population variance = 4.0
        assert!((w.variance() - 4.0).abs() < 0.1);
    }

    #[test]
    fn dot_product_test() {
        let a = [1.0, 2.0, 3.0];
        let b = [4.0, 5.0, 6.0];
        assert!((dot_product(&a, &b) - 32.0).abs() < 1e-6);
    }

    #[test]
    fn l2_norm_test() {
        let a = [3.0, 4.0];
        assert!((l2_norm(&a) - 5.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_identical() {
        let a = [1.0, 2.0, 3.0];
        assert!((cosine_similarity(&a, &a) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn cosine_similarity_orthogonal() {
        let a = [1.0, 0.0];
        let b = [0.0, 1.0];
        assert!(cosine_similarity(&a, &b).abs() < 1e-5);
    }

    #[test]
    fn l2_distance_test() {
        let a = [0.0, 0.0];
        let b = [3.0, 4.0];
        assert!((l2_distance(&a, &b) - 5.0).abs() < 1e-6);
    }

    #[test]
    fn dtw_identical_sequences() {
        let a = [1.0, 2.0, 3.0, 4.0];
        let d = dtw_distance(&a, &a);
        assert!(d < 1e-6);
    }

    #[test]
    fn dtw_shifted_sequences() {
        let a = [0.0, 1.0, 2.0, 1.0, 0.0];
        let b = [0.0, 0.0, 1.0, 2.0, 1.0];
        let d = dtw_distance(&a, &b);
        // Should be small since b is just a shifted version of a.
        assert!(d < 1.0);
    }

    #[test]
    fn dtw_banded_matches_full_on_aligned() {
        let a = [1.0, 2.0, 3.0, 2.0, 1.0];
        let full = dtw_distance(&a, &a);
        let banded = dtw_distance_banded(&a, &a, 2);
        assert!((full - banded).abs() < 1e-6);
    }

    #[test]
    fn priority_queue_basic() {
        let mut pq = FixedPriorityQueue::<4>::new();
        pq.insert(3.0, 10);
        pq.insert(1.0, 20);
        pq.insert(5.0, 30);
        pq.insert(2.0, 40);
        assert_eq!(pq.len(), 4);

        let (max_score, max_id) = pq.peek_max().unwrap();
        assert!((max_score - 5.0).abs() < 1e-6);
        assert_eq!(max_id, 30);

        // Insert something larger than the min (1.0) => replaces it.
        pq.insert(4.0, 50);
        let (min_score, _) = pq.peek_min().unwrap();
        assert!((min_score - 2.0).abs() < 1e-6); // 1.0 was replaced

        // Insert something smaller than the min => discarded.
        pq.insert(0.5, 60);
        assert_eq!(pq.len(), 4);
        let (min_score, _) = pq.peek_min().unwrap();
        assert!((min_score - 2.0).abs() < 1e-6); // unchanged
    }
}
