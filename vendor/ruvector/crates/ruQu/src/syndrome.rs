//! Syndrome Processing Module
//!
//! High-throughput data pipeline for quantum error syndrome ingestion, buffering,
//! and transformation. This module implements the Supporting Domain for the
//! Coherence Gate core domain.
//!
//! ## Components
//!
//! - [`DetectorBitmap`]: Packed bit representation for up to 1024 detectors
//! - [`SyndromeRound`]: Complete syndrome measurement cycle
//! - [`SyndromeBuffer`]: Ring buffer for syndrome history
//! - [`SyndromeDelta`]: Change between consecutive rounds
//!
//! ## Performance
//!
//! All types are designed for microsecond-scale operations:
//! - SIMD-friendly memory layouts (aligned, packed)
//! - Zero-copy where possible
//! - Preallocated buffers to avoid allocation on hot paths

use serde::{Deserialize, Serialize};

// ============================================================================
// DetectorBitmap - Packed bit representation for detectors
// ============================================================================

/// Number of u64 words in the bitmap (1024 detectors / 64 bits per word)
const BITMAP_WORDS: usize = 16;

/// Packed bit representation of detector values.
///
/// Efficiently stores up to 1024 detector values (one bit each) in a fixed-size
/// array of 16 u64 words. Operations are optimized for SIMD execution.
///
/// # Layout
///
/// ```text
/// bits[0]:  detectors 0-63
/// bits[1]:  detectors 64-127
/// ...
/// bits[15]: detectors 960-1023
/// ```
///
/// # Example
///
/// ```rust
/// use ruqu::syndrome::DetectorBitmap;
///
/// let mut bitmap = DetectorBitmap::new(128);
///
/// // Set some detectors as fired
/// bitmap.set(0, true);
/// bitmap.set(64, true);
/// bitmap.set(127, true);
///
/// assert_eq!(bitmap.fired_count(), 3);
/// assert!(bitmap.get(0));
/// assert!(!bitmap.get(1));
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(C, align(64))] // Cache-line aligned for SIMD
pub struct DetectorBitmap {
    /// Packed detector bits (16 * 64 = 1024 detectors max)
    bits: [u64; BITMAP_WORDS],
    /// Number of detectors in use (may be less than 1024)
    count: usize,
}

impl Default for DetectorBitmap {
    fn default() -> Self {
        Self::new(0)
    }
}

impl std::fmt::Debug for DetectorBitmap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DetectorBitmap")
            .field("count", &self.count)
            .field("fired", &self.fired_count())
            .finish()
    }
}

impl DetectorBitmap {
    /// Creates a new bitmap with the specified number of detectors.
    ///
    /// All detectors are initially set to 0 (not fired).
    ///
    /// # Arguments
    ///
    /// * `count` - Number of detectors (0 to 1024)
    ///
    /// # Panics
    ///
    /// Panics if `count` exceeds 1024.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ruqu::syndrome::DetectorBitmap;
    ///
    /// let bitmap = DetectorBitmap::new(256);
    /// assert_eq!(bitmap.detector_count(), 256);
    /// assert_eq!(bitmap.fired_count(), 0);
    /// ```
    #[inline]
    #[must_use]
    pub fn new(count: usize) -> Self {
        assert!(count <= BITMAP_WORDS * 64, "count exceeds maximum of 1024");
        Self {
            bits: [0u64; BITMAP_WORDS],
            count,
        }
    }

    /// Creates a bitmap from raw bits.
    ///
    /// # Arguments
    ///
    /// * `bits` - Array of 16 u64 words containing packed detector values
    /// * `count` - Number of detectors in use (must be <= 1024)
    ///
    /// # Panics
    ///
    /// Panics if `count` exceeds 1024.
    ///
    /// # Note
    ///
    /// For consistent behavior, bits beyond `count` should be zero.
    #[inline]
    #[must_use]
    pub const fn from_raw(bits: [u64; BITMAP_WORDS], count: usize) -> Self {
        // SECURITY: Validate count to prevent out-of-bounds access in other methods
        assert!(count <= BITMAP_WORDS * 64, "count exceeds maximum of 1024");
        Self { bits, count }
    }

    /// Returns the raw bits array.
    #[inline]
    #[must_use]
    pub const fn raw_bits(&self) -> &[u64; BITMAP_WORDS] {
        &self.bits
    }

    /// Returns the number of detectors configured.
    #[inline]
    #[must_use]
    pub const fn detector_count(&self) -> usize {
        self.count
    }

    /// Sets the value of a detector.
    ///
    /// # Arguments
    ///
    /// * `idx` - Detector index (0 to count-1)
    /// * `value` - true if detector fired, false otherwise
    ///
    /// # Panics
    ///
    /// Panics if `idx >= count` (in both debug and release builds).
    ///
    /// # Example
    ///
    /// ```rust
    /// use ruqu::syndrome::DetectorBitmap;
    ///
    /// let mut bitmap = DetectorBitmap::new(64);
    /// bitmap.set(5, true);
    /// assert!(bitmap.get(5));
    ///
    /// bitmap.set(5, false);
    /// assert!(!bitmap.get(5));
    /// ```
    #[inline]
    pub fn set(&mut self, idx: usize, value: bool) {
        // SECURITY: Use assert! not debug_assert! to ensure bounds check in release builds
        assert!(
            idx < self.count,
            "detector index {} out of bounds (count: {})",
            idx,
            self.count
        );
        let word = idx / 64;
        let bit = idx % 64;
        if value {
            self.bits[word] |= 1u64 << bit;
        } else {
            self.bits[word] &= !(1u64 << bit);
        }
    }

    /// Gets the value of a detector.
    ///
    /// # Arguments
    ///
    /// * `idx` - Detector index (0 to count-1)
    ///
    /// # Returns
    ///
    /// `true` if the detector fired, `false` otherwise.
    ///
    /// # Panics
    ///
    /// Panics if `idx >= count` (in both debug and release builds).
    ///
    /// # Example
    ///
    /// ```rust
    /// use ruqu::syndrome::DetectorBitmap;
    ///
    /// let mut bitmap = DetectorBitmap::new(64);
    /// bitmap.set(10, true);
    ///
    /// assert!(bitmap.get(10));
    /// assert!(!bitmap.get(0));
    /// ```
    #[inline]
    #[must_use]
    pub fn get(&self, idx: usize) -> bool {
        // SECURITY: Use assert! not debug_assert! to ensure bounds check in release builds
        assert!(
            idx < self.count,
            "detector index {} out of bounds (count: {})",
            idx,
            self.count
        );
        let word = idx / 64;
        let bit = idx % 64;
        (self.bits[word] >> bit) & 1 == 1
    }

    /// Returns the number of fired detectors (popcount).
    ///
    /// Uses hardware popcount instructions when available.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ruqu::syndrome::DetectorBitmap;
    ///
    /// let mut bitmap = DetectorBitmap::new(64);
    /// bitmap.set(0, true);
    /// bitmap.set(10, true);
    /// bitmap.set(63, true);
    ///
    /// assert_eq!(bitmap.fired_count(), 3);
    /// ```
    #[inline]
    #[must_use]
    pub fn fired_count(&self) -> usize {
        self.popcount()
    }

    /// Returns the total popcount (number of set bits).
    ///
    /// This is the same as `fired_count()` but with a more algorithmic name.
    ///
    /// # Performance
    ///
    /// - Uses hardware `popcnt` instruction on x86_64
    /// - With `simd` feature, uses AVX2 parallel popcount for additional speedup
    /// - Falls back to portable implementation on other architectures
    #[inline]
    #[must_use]
    pub fn popcount(&self) -> usize {
        // Calculate how many full words to count based on detector count
        let full_words = self.count / 64;
        let remaining_bits = self.count % 64;

        #[cfg(all(feature = "simd", target_arch = "x86_64"))]
        {
            // AVX2 SIMD popcount using lookup table method
            if is_x86_feature_detected!("avx2") && full_words >= 4 {
                unsafe {
                    return self.popcount_avx2(full_words, remaining_bits);
                }
            }
        }

        // Scalar path with hardware popcnt
        let mut total = 0usize;

        // Count full words
        for word in &self.bits[..full_words] {
            total += word.count_ones() as usize;
        }

        // Count partial word if any
        if remaining_bits > 0 && full_words < BITMAP_WORDS {
            let mask = (1u64 << remaining_bits) - 1;
            total += (self.bits[full_words] & mask).count_ones() as usize;
        }

        total
    }

    /// AVX2 SIMD popcount implementation
    ///
    /// Uses the lookup table method: count bits in each nibble using vpshufb
    #[cfg(all(feature = "simd", target_arch = "x86_64"))]
    #[inline]
    #[target_feature(enable = "avx2")]
    unsafe fn popcount_avx2(&self, full_words: usize, remaining_bits: usize) -> usize {
        use std::arch::x86_64::*;

        // Lookup table for 4-bit popcount
        let lookup = _mm256_setr_epi8(
            0, 1, 1, 2, 1, 2, 2, 3, 1, 2, 2, 3, 2, 3, 3, 4, 0, 1, 1, 2, 1, 2, 2, 3, 1, 2, 2, 3, 2,
            3, 3, 4,
        );
        let low_mask = _mm256_set1_epi8(0x0f);

        let mut total_vec = _mm256_setzero_si256();
        let mut i = 0;

        // Process 4 u64s (256 bits) at a time
        while i + 4 <= full_words {
            let data = _mm256_loadu_si256(self.bits.as_ptr().add(i) as *const __m256i);

            // Split into low and high nibbles
            let lo = _mm256_and_si256(data, low_mask);
            let hi = _mm256_and_si256(_mm256_srli_epi16(data, 4), low_mask);

            // Lookup popcount for each nibble
            let popcnt_lo = _mm256_shuffle_epi8(lookup, lo);
            let popcnt_hi = _mm256_shuffle_epi8(lookup, hi);

            // Sum nibble popcounts (sad accumulates byte sums into u64)
            let popcnt = _mm256_add_epi8(popcnt_lo, popcnt_hi);
            total_vec =
                _mm256_add_epi64(total_vec, _mm256_sad_epu8(popcnt, _mm256_setzero_si256()));

            i += 4;
        }

        // Horizontal sum of the 4 u64 accumulators
        let mut total = 0usize;
        let mut buf = [0u64; 4];
        _mm256_storeu_si256(buf.as_mut_ptr() as *mut __m256i, total_vec);
        total += buf[0] as usize + buf[1] as usize + buf[2] as usize + buf[3] as usize;

        // Handle remaining full words with scalar popcnt
        while i < full_words {
            total += self.bits[i].count_ones() as usize;
            i += 1;
        }

        // Count partial word if any
        if remaining_bits > 0 && full_words < BITMAP_WORDS {
            let mask = (1u64 << remaining_bits) - 1;
            total += (self.bits[full_words] & mask).count_ones() as usize;
        }

        total
    }

    /// Returns an iterator over fired detector indices.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ruqu::syndrome::DetectorBitmap;
    ///
    /// let mut bitmap = DetectorBitmap::new(64);
    /// bitmap.set(5, true);
    /// bitmap.set(10, true);
    /// bitmap.set(20, true);
    ///
    /// let fired: Vec<usize> = bitmap.iter_fired().collect();
    /// assert_eq!(fired, vec![5, 10, 20]);
    /// ```
    #[inline]
    pub fn iter_fired(&self) -> FiredIterator<'_> {
        FiredIterator {
            bitmap: self,
            word_idx: 0,
            current_word: self.bits[0],
            base_idx: 0,
        }
    }

    /// Computes the XOR of two bitmaps.
    ///
    /// The result shows which detectors changed state between the two bitmaps.
    /// The count is set to the maximum of the two input counts.
    ///
    /// # Performance
    ///
    /// When the `simd` feature is enabled on x86_64, uses AVX2 instructions
    /// for 4x speedup on the XOR operation.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ruqu::syndrome::DetectorBitmap;
    ///
    /// let mut a = DetectorBitmap::new(64);
    /// a.set(0, true);
    /// a.set(5, true);
    ///
    /// let mut b = DetectorBitmap::new(64);
    /// b.set(0, true);
    /// b.set(10, true);
    ///
    /// let delta = a.xor(&b);
    /// assert!(delta.get(5));   // Changed: was true, now false
    /// assert!(delta.get(10));  // Changed: was false, now true
    /// assert!(!delta.get(0));  // Unchanged: both true
    /// ```
    #[inline]
    #[must_use]
    pub fn xor(&self, other: &DetectorBitmap) -> DetectorBitmap {
        let mut result = DetectorBitmap::new(self.count.max(other.count));

        #[cfg(all(feature = "simd", target_arch = "x86_64"))]
        {
            // AVX2 SIMD: process 256 bits (4 u64s) at a time
            if is_x86_feature_detected!("avx2") {
                unsafe {
                    use std::arch::x86_64::*;
                    // Process first 8 words (512 bits) with two AVX2 operations
                    let a0 = _mm256_loadu_si256(self.bits.as_ptr() as *const __m256i);
                    let b0 = _mm256_loadu_si256(other.bits.as_ptr() as *const __m256i);
                    let r0 = _mm256_xor_si256(a0, b0);
                    _mm256_storeu_si256(result.bits.as_mut_ptr() as *mut __m256i, r0);

                    let a1 = _mm256_loadu_si256(self.bits.as_ptr().add(4) as *const __m256i);
                    let b1 = _mm256_loadu_si256(other.bits.as_ptr().add(4) as *const __m256i);
                    let r1 = _mm256_xor_si256(a1, b1);
                    _mm256_storeu_si256(result.bits.as_mut_ptr().add(4) as *mut __m256i, r1);

                    // Process remaining 8 words
                    let a2 = _mm256_loadu_si256(self.bits.as_ptr().add(8) as *const __m256i);
                    let b2 = _mm256_loadu_si256(other.bits.as_ptr().add(8) as *const __m256i);
                    let r2 = _mm256_xor_si256(a2, b2);
                    _mm256_storeu_si256(result.bits.as_mut_ptr().add(8) as *mut __m256i, r2);

                    let a3 = _mm256_loadu_si256(self.bits.as_ptr().add(12) as *const __m256i);
                    let b3 = _mm256_loadu_si256(other.bits.as_ptr().add(12) as *const __m256i);
                    let r3 = _mm256_xor_si256(a3, b3);
                    _mm256_storeu_si256(result.bits.as_mut_ptr().add(12) as *mut __m256i, r3);

                    return result;
                }
            }
        }

        // Scalar fallback: SIMD-friendly unrolled XOR
        for i in 0..BITMAP_WORDS {
            result.bits[i] = self.bits[i] ^ other.bits[i];
        }

        result
    }

    /// Computes the AND of two bitmaps.
    ///
    /// Returns detectors that are fired in both bitmaps.
    ///
    /// # Performance
    ///
    /// With `simd` feature on x86_64, uses AVX2 for vectorized AND.
    #[inline]
    #[must_use]
    pub fn and(&self, other: &DetectorBitmap) -> DetectorBitmap {
        let mut result = DetectorBitmap::new(self.count.min(other.count));

        #[cfg(all(feature = "simd", target_arch = "x86_64"))]
        {
            if is_x86_feature_detected!("avx2") {
                unsafe {
                    use std::arch::x86_64::*;
                    for i in (0..BITMAP_WORDS).step_by(4) {
                        let a = _mm256_loadu_si256(self.bits.as_ptr().add(i) as *const __m256i);
                        let b = _mm256_loadu_si256(other.bits.as_ptr().add(i) as *const __m256i);
                        let r = _mm256_and_si256(a, b);
                        _mm256_storeu_si256(result.bits.as_mut_ptr().add(i) as *mut __m256i, r);
                    }
                    return result;
                }
            }
        }

        for i in 0..BITMAP_WORDS {
            result.bits[i] = self.bits[i] & other.bits[i];
        }

        result
    }

    /// Computes the OR of two bitmaps.
    ///
    /// Returns detectors that are fired in either bitmap.
    ///
    /// # Performance
    ///
    /// With `simd` feature on x86_64, uses AVX2 for vectorized OR.
    #[inline]
    #[must_use]
    pub fn or(&self, other: &DetectorBitmap) -> DetectorBitmap {
        let mut result = DetectorBitmap::new(self.count.max(other.count));

        #[cfg(all(feature = "simd", target_arch = "x86_64"))]
        {
            if is_x86_feature_detected!("avx2") {
                unsafe {
                    use std::arch::x86_64::*;
                    for i in (0..BITMAP_WORDS).step_by(4) {
                        let a = _mm256_loadu_si256(self.bits.as_ptr().add(i) as *const __m256i);
                        let b = _mm256_loadu_si256(other.bits.as_ptr().add(i) as *const __m256i);
                        let r = _mm256_or_si256(a, b);
                        _mm256_storeu_si256(result.bits.as_mut_ptr().add(i) as *mut __m256i, r);
                    }
                    return result;
                }
            }
        }

        for i in 0..BITMAP_WORDS {
            result.bits[i] = self.bits[i] | other.bits[i];
        }

        result
    }

    /// Computes the NOT of this bitmap (inverts all bits).
    ///
    /// # Performance
    ///
    /// With `simd` feature on x86_64, uses AVX2 for vectorized NOT.
    #[inline]
    #[must_use]
    pub fn not(&self) -> DetectorBitmap {
        let mut result = DetectorBitmap::new(self.count);

        #[cfg(all(feature = "simd", target_arch = "x86_64"))]
        {
            if is_x86_feature_detected!("avx2") {
                unsafe {
                    use std::arch::x86_64::*;
                    let ones = _mm256_set1_epi64x(-1i64);
                    for i in (0..BITMAP_WORDS).step_by(4) {
                        let a = _mm256_loadu_si256(self.bits.as_ptr().add(i) as *const __m256i);
                        let r = _mm256_xor_si256(a, ones);
                        _mm256_storeu_si256(result.bits.as_mut_ptr().add(i) as *mut __m256i, r);
                    }
                    // Mask off bits beyond count
                    let full_words = self.count / 64;
                    let remaining_bits = self.count % 64;
                    if remaining_bits > 0 && full_words < BITMAP_WORDS {
                        let mask = (1u64 << remaining_bits) - 1;
                        result.bits[full_words] &= mask;
                    }
                    // Zero out words beyond count
                    for i in (full_words + 1)..BITMAP_WORDS {
                        result.bits[i] = 0;
                    }
                    return result;
                }
            }
        }

        let full_words = self.count / 64;
        let remaining_bits = self.count % 64;

        for i in 0..full_words {
            result.bits[i] = !self.bits[i];
        }

        if remaining_bits > 0 && full_words < BITMAP_WORDS {
            let mask = (1u64 << remaining_bits) - 1;
            result.bits[full_words] = (!self.bits[full_words]) & mask;
        }

        result
    }

    /// Returns true if no detectors are fired.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bits.iter().all(|&w| w == 0)
    }

    /// Clears all detector values to zero.
    #[inline]
    pub fn clear(&mut self) {
        self.bits = [0u64; BITMAP_WORDS];
    }
}

/// Iterator over fired detector indices.
pub struct FiredIterator<'a> {
    bitmap: &'a DetectorBitmap,
    word_idx: usize,
    current_word: u64,
    base_idx: usize,
}

impl<'a> Iterator for FiredIterator<'a> {
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.current_word != 0 {
                // Find lowest set bit
                let trailing = self.current_word.trailing_zeros() as usize;
                let idx = self.base_idx + trailing;

                // Check if within detector count
                if idx >= self.bitmap.count {
                    return None;
                }

                // Clear the bit we just found
                self.current_word &= self.current_word - 1;

                return Some(idx);
            }

            // Move to next word
            self.word_idx += 1;
            if self.word_idx >= BITMAP_WORDS {
                return None;
            }

            self.base_idx = self.word_idx * 64;

            // Check if we've passed the detector count
            if self.base_idx >= self.bitmap.count {
                return None;
            }

            self.current_word = self.bitmap.bits[self.word_idx];
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        // Upper bound is remaining popcount
        let remaining_popcount = self.bitmap.popcount();
        (0, Some(remaining_popcount))
    }
}

// ============================================================================
// SyndromeRound - Complete syndrome measurement
// ============================================================================

/// A complete syndrome measurement cycle.
///
/// Represents all syndrome data collected in one measurement round (typically 1μs).
/// This is the aggregate root for syndrome data, containing the detector bitmap
/// and associated metadata.
///
/// # Memory Layout
///
/// Total size: 152 bytes (with 64-byte aligned DetectorBitmap)
///
/// # Example
///
/// ```rust
/// use ruqu::syndrome::{DetectorBitmap, SyndromeRound};
///
/// let mut detectors = DetectorBitmap::new(64);
/// detectors.set(5, true);
/// detectors.set(10, true);
///
/// let round = SyndromeRound {
///     round_id: 12345,
///     cycle: 1000,
///     timestamp: 1705500000000,
///     detectors,
///     source_tile: 0,
/// };
///
/// assert_eq!(round.fired_count(), 2);
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyndromeRound {
    /// Unique identifier for this round (monotonically increasing per tile)
    pub round_id: u64,
    /// Quantum cycle number (global clock)
    pub cycle: u64,
    /// Hardware timestamp in nanoseconds
    pub timestamp: u64,
    /// Detector measurement outcomes
    pub detectors: DetectorBitmap,
    /// Source tile identifier (0-255)
    pub source_tile: u8,
}

impl SyndromeRound {
    /// Creates a new syndrome round with the given parameters.
    #[inline]
    #[must_use]
    pub fn new(
        round_id: u64,
        cycle: u64,
        timestamp: u64,
        detectors: DetectorBitmap,
        source_tile: u8,
    ) -> Self {
        Self {
            round_id,
            cycle,
            timestamp,
            detectors,
            source_tile,
        }
    }

    /// Returns the number of fired detectors in this round.
    #[inline]
    #[must_use]
    pub fn fired_count(&self) -> usize {
        self.detectors.fired_count()
    }

    /// Returns an iterator over fired detector indices.
    #[inline]
    pub fn iter_fired(&self) -> FiredIterator<'_> {
        self.detectors.iter_fired()
    }

    /// Computes the delta to another round.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ruqu::syndrome::{DetectorBitmap, SyndromeRound};
    ///
    /// let mut d1 = DetectorBitmap::new(64);
    /// d1.set(0, true);
    /// d1.set(5, true);
    ///
    /// let round1 = SyndromeRound::new(1, 100, 1000, d1, 0);
    ///
    /// let mut d2 = DetectorBitmap::new(64);
    /// d2.set(5, true);
    /// d2.set(10, true);
    ///
    /// let round2 = SyndromeRound::new(2, 101, 1001, d2, 0);
    ///
    /// let delta = round1.delta_to(&round2);
    /// assert_eq!(delta.flip_count(), 2); // 0 cleared, 10 fired
    /// ```
    #[inline]
    #[must_use]
    pub fn delta_to(&self, other: &SyndromeRound) -> SyndromeDelta {
        SyndromeDelta::compute(self, other)
    }
}

// ============================================================================
// SyndromeBuffer - Ring buffer for syndrome history
// ============================================================================

/// Statistics about buffer state.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BufferStatistics {
    /// Total rounds pushed to buffer
    pub total_rounds: u64,
    /// Number of rounds currently in buffer
    pub current_size: usize,
    /// Buffer capacity
    pub capacity: usize,
    /// Number of rounds evicted (overwritten)
    pub evicted_rounds: u64,
    /// Average firing rate across recent rounds
    pub avg_firing_rate: f64,
    /// Maximum firing count seen
    pub max_firing_count: usize,
    /// Oldest round ID in buffer
    pub oldest_round_id: Option<u64>,
    /// Newest round ID in buffer
    pub newest_round_id: Option<u64>,
}

/// Ring buffer holding recent syndrome history.
///
/// Provides efficient O(1) push and windowed access to recent syndrome rounds.
/// When the buffer is full, oldest entries are overwritten.
///
/// # Capacity
///
/// The buffer has a fixed capacity set at creation. Typical values:
/// - 1024 rounds for 1ms history at 1MHz syndrome rate
/// - 4096 rounds for longer-term analysis
///
/// # Thread Safety
///
/// This buffer is not thread-safe. Use external synchronization or
/// one buffer per tile (recommended).
///
/// # Example
///
/// ```rust
/// use ruqu::syndrome::{DetectorBitmap, SyndromeRound, SyndromeBuffer};
///
/// let mut buffer = SyndromeBuffer::new(1024);
///
/// // Push rounds
/// for i in 0..100 {
///     let mut detectors = DetectorBitmap::new(64);
///     if i % 10 == 0 {
///         detectors.set(i % 64, true);
///     }
///     let round = SyndromeRound::new(i as u64, i as u64, i as u64 * 1000, detectors, 0);
///     buffer.push(round);
/// }
///
/// // Get recent window
/// let window = buffer.window(10);
/// assert_eq!(window.len(), 10);
///
/// // Access by round ID
/// if let Some(round) = buffer.get(95) {
///     assert_eq!(round.round_id, 95);
/// }
/// ```
#[derive(Clone, Debug)]
pub struct SyndromeBuffer {
    /// Buffer capacity (fixed at creation)
    capacity: usize,
    /// Preallocated round storage
    rounds: Vec<Option<SyndromeRound>>,
    /// Current write index (wraps at capacity)
    write_index: usize,
    /// Number of valid entries
    valid_count: usize,
    /// Watermark: oldest round ID guaranteed to be in buffer
    watermark: u64,
    /// Total rounds pushed (for statistics)
    total_pushed: u64,
    /// Running sum of firing counts (for average)
    firing_sum: u64,
    /// Maximum firing count seen
    max_firing: usize,
}

impl SyndromeBuffer {
    /// Creates a new buffer with the specified capacity.
    ///
    /// # Arguments
    ///
    /// * `capacity` - Maximum number of rounds to store
    ///
    /// # Panics
    ///
    /// Panics if capacity is 0.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "buffer capacity must be positive");
        Self {
            capacity,
            rounds: vec![None; capacity],
            write_index: 0,
            valid_count: 0,
            watermark: 0,
            total_pushed: 0,
            firing_sum: 0,
            max_firing: 0,
        }
    }

    /// Returns the buffer capacity.
    #[inline]
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns the number of valid entries in the buffer.
    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        self.valid_count
    }

    /// Returns true if the buffer is empty.
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.valid_count == 0
    }

    /// Returns true if the buffer is full.
    #[inline]
    #[must_use]
    pub const fn is_full(&self) -> bool {
        self.valid_count >= self.capacity
    }

    /// Returns the current watermark (oldest retained round ID).
    #[inline]
    #[must_use]
    pub const fn watermark(&self) -> u64 {
        self.watermark
    }

    /// Pushes a new round into the buffer.
    ///
    /// If the buffer is full, the oldest entry is evicted.
    ///
    /// # Arguments
    ///
    /// * `round` - The syndrome round to add
    ///
    /// # Example
    ///
    /// ```rust
    /// use ruqu::syndrome::{DetectorBitmap, SyndromeRound, SyndromeBuffer};
    ///
    /// let mut buffer = SyndromeBuffer::new(100);
    /// let round = SyndromeRound::new(1, 1, 1000, DetectorBitmap::new(64), 0);
    /// buffer.push(round);
    ///
    /// assert_eq!(buffer.len(), 1);
    /// ```
    #[inline]
    pub fn push(&mut self, round: SyndromeRound) {
        // Update statistics
        let fired = round.fired_count();
        self.firing_sum += fired as u64;
        self.max_firing = self.max_firing.max(fired);
        self.total_pushed += 1;

        // Update watermark if we're overwriting
        if self.valid_count >= self.capacity {
            if let Some(ref old) = self.rounds[self.write_index] {
                // Advance watermark past the evicted round
                self.watermark = old.round_id + 1;
            }
        }

        // Store the round
        self.rounds[self.write_index] = Some(round);

        // Advance write pointer
        self.write_index = (self.write_index + 1) % self.capacity;

        // Update valid count
        if self.valid_count < self.capacity {
            self.valid_count += 1;
        }
    }

    /// Returns a window of the most recent rounds.
    ///
    /// # Arguments
    ///
    /// * `size` - Number of rounds to retrieve (clamped to available)
    ///
    /// # Returns
    ///
    /// A vector of the most recent `size` rounds, oldest first.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ruqu::syndrome::{DetectorBitmap, SyndromeRound, SyndromeBuffer};
    ///
    /// let mut buffer = SyndromeBuffer::new(100);
    /// for i in 0..50 {
    ///     let round = SyndromeRound::new(i, i, i * 1000, DetectorBitmap::new(64), 0);
    ///     buffer.push(round);
    /// }
    ///
    /// let window = buffer.window(10);
    /// assert_eq!(window.len(), 10);
    /// assert_eq!(window[0].round_id, 40); // Oldest in window
    /// assert_eq!(window[9].round_id, 49); // Newest in window
    /// ```
    #[must_use]
    pub fn window(&self, size: usize) -> Vec<&SyndromeRound> {
        let actual_size = size.min(self.valid_count);
        if actual_size == 0 {
            return Vec::new();
        }

        let mut result = Vec::with_capacity(actual_size);

        // Calculate start index (oldest in window)
        let start = if self.write_index >= actual_size {
            self.write_index - actual_size
        } else {
            self.capacity - (actual_size - self.write_index)
        };

        for i in 0..actual_size {
            let idx = (start + i) % self.capacity;
            if let Some(ref round) = self.rounds[idx] {
                result.push(round);
            }
        }

        result
    }

    /// Retrieves a round by its round ID.
    ///
    /// # Arguments
    ///
    /// * `round_id` - The round ID to look up
    ///
    /// # Returns
    ///
    /// `Some(&SyndromeRound)` if found, `None` if not in buffer.
    ///
    /// # Performance
    ///
    /// O(1) if the buffer maintains sequential round IDs, otherwise O(n).
    #[must_use]
    pub fn get(&self, round_id: u64) -> Option<&SyndromeRound> {
        if self.valid_count == 0 || round_id < self.watermark {
            return None;
        }

        // Try direct index first (assumes sequential round IDs)
        if let Some(ref newest) =
            self.rounds[(self.write_index + self.capacity - 1) % self.capacity]
        {
            if round_id <= newest.round_id {
                let offset = (newest.round_id - round_id) as usize;
                if offset < self.valid_count {
                    let idx = if self.write_index > offset {
                        self.write_index - 1 - offset
                    } else {
                        self.capacity - 1 - (offset - self.write_index)
                    };

                    if let Some(ref round) = self.rounds[idx] {
                        if round.round_id == round_id {
                            return Some(round);
                        }
                    }
                }
            }
        }

        // Fall back to linear search
        for i in 0..self.valid_count {
            let idx = if self.write_index > i {
                self.write_index - 1 - i
            } else {
                self.capacity - 1 - (i - self.write_index)
            };

            if let Some(ref round) = self.rounds[idx] {
                if round.round_id == round_id {
                    return Some(round);
                }
            }
        }

        None
    }

    /// Returns buffer statistics.
    #[must_use]
    pub fn statistics(&self) -> BufferStatistics {
        let (oldest_id, newest_id) = if self.valid_count > 0 {
            let oldest_idx = if self.valid_count < self.capacity {
                0
            } else {
                self.write_index
            };
            let newest_idx = (self.write_index + self.capacity - 1) % self.capacity;

            let oldest = self.rounds[oldest_idx].as_ref().map(|r| r.round_id);
            let newest = self.rounds[newest_idx].as_ref().map(|r| r.round_id);
            (oldest, newest)
        } else {
            (None, None)
        };

        let avg_firing = if self.total_pushed > 0 {
            self.firing_sum as f64 / self.total_pushed as f64
        } else {
            0.0
        };

        let evicted = if self.total_pushed > self.capacity as u64 {
            self.total_pushed - self.capacity as u64
        } else {
            0
        };

        BufferStatistics {
            total_rounds: self.total_pushed,
            current_size: self.valid_count,
            capacity: self.capacity,
            evicted_rounds: evicted,
            avg_firing_rate: avg_firing,
            max_firing_count: self.max_firing,
            oldest_round_id: oldest_id,
            newest_round_id: newest_id,
        }
    }

    /// Clears the buffer, removing all entries.
    pub fn clear(&mut self) {
        for round in &mut self.rounds {
            *round = None;
        }
        self.write_index = 0;
        self.valid_count = 0;
        self.total_pushed = 0;
        self.firing_sum = 0;
        self.max_firing = 0;
        self.watermark = 0;
    }

    /// Returns an iterator over all valid rounds, oldest first.
    pub fn iter(&self) -> impl Iterator<Item = &SyndromeRound> {
        let start = if self.valid_count < self.capacity {
            0
        } else {
            self.write_index
        };

        (0..self.valid_count)
            .map(move |i| (start + i) % self.capacity)
            .filter_map(move |idx| self.rounds[idx].as_ref())
    }
}

// ============================================================================
// SyndromeDelta - Change between rounds
// ============================================================================

/// Represents the change in syndrome state between two rounds.
///
/// Used to track which detectors flipped between consecutive measurements,
/// enabling efficient change detection and activity monitoring.
///
/// # Example
///
/// ```rust
/// use ruqu::syndrome::{DetectorBitmap, SyndromeRound, SyndromeDelta};
///
/// let mut d1 = DetectorBitmap::new(64);
/// d1.set(0, true);
/// d1.set(5, true);
///
/// let mut d2 = DetectorBitmap::new(64);
/// d2.set(5, true);
/// d2.set(10, true);
///
/// let round1 = SyndromeRound::new(1, 100, 1000, d1, 0);
/// let round2 = SyndromeRound::new(2, 101, 2000, d2, 0);
///
/// let delta = SyndromeDelta::compute(&round1, &round2);
///
/// assert_eq!(delta.from_round, 1);
/// assert_eq!(delta.to_round, 2);
/// assert_eq!(delta.flip_count(), 2);  // Detectors 0 and 10 flipped
/// assert!(!delta.is_quiet());
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyndromeDelta {
    /// Source round ID
    pub from_round: u64,
    /// Target round ID
    pub to_round: u64,
    /// Bitmap of flipped detectors (XOR of the two rounds)
    pub flipped: DetectorBitmap,
}

impl SyndromeDelta {
    /// Computes the delta between two syndrome rounds.
    ///
    /// # Arguments
    ///
    /// * `from` - The earlier round
    /// * `to` - The later round
    #[inline]
    #[must_use]
    pub fn compute(from: &SyndromeRound, to: &SyndromeRound) -> Self {
        Self {
            from_round: from.round_id,
            to_round: to.round_id,
            flipped: from.detectors.xor(&to.detectors),
        }
    }

    /// Creates a delta from raw components.
    #[inline]
    #[must_use]
    pub const fn new(from_round: u64, to_round: u64, flipped: DetectorBitmap) -> Self {
        Self {
            from_round,
            to_round,
            flipped,
        }
    }

    /// Returns true if no detectors changed state.
    ///
    /// A "quiet" delta indicates the syndrome is stable.
    #[inline]
    #[must_use]
    pub fn is_quiet(&self) -> bool {
        self.flipped.is_empty()
    }

    /// Returns the number of detectors that flipped.
    #[inline]
    #[must_use]
    pub fn flip_count(&self) -> usize {
        self.flipped.popcount()
    }

    /// Returns the activity level as a ratio of flipped detectors.
    ///
    /// Activity level = flipped_count / total_detectors
    ///
    /// # Returns
    ///
    /// Value between 0.0 (no activity) and 1.0 (all detectors flipped).
    #[inline]
    #[must_use]
    pub fn activity_level(&self) -> f64 {
        let count = self.flipped.detector_count();
        if count == 0 {
            return 0.0;
        }
        self.flipped.popcount() as f64 / count as f64
    }

    /// Returns an iterator over flipped detector indices.
    #[inline]
    pub fn iter_flipped(&self) -> FiredIterator<'_> {
        self.flipped.iter_fired()
    }

    /// Returns the temporal span of this delta.
    ///
    /// # Returns
    ///
    /// Number of rounds between from and to (to_round - from_round).
    #[inline]
    #[must_use]
    pub const fn span(&self) -> u64 {
        self.to_round.saturating_sub(self.from_round)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ---------- DetectorBitmap tests ----------

    #[test]
    fn test_bitmap_new() {
        let bitmap = DetectorBitmap::new(64);
        assert_eq!(bitmap.detector_count(), 64);
        assert_eq!(bitmap.fired_count(), 0);
        assert!(bitmap.is_empty());
    }

    #[test]
    fn test_bitmap_set_get() {
        let mut bitmap = DetectorBitmap::new(128);

        bitmap.set(0, true);
        bitmap.set(63, true);
        bitmap.set(64, true);
        bitmap.set(127, true);

        assert!(bitmap.get(0));
        assert!(bitmap.get(63));
        assert!(bitmap.get(64));
        assert!(bitmap.get(127));
        assert!(!bitmap.get(1));
        assert!(!bitmap.get(100));
    }

    #[test]
    fn test_bitmap_fired_count() {
        let mut bitmap = DetectorBitmap::new(256);

        bitmap.set(0, true);
        bitmap.set(10, true);
        bitmap.set(100, true);
        bitmap.set(200, true);

        assert_eq!(bitmap.fired_count(), 4);
        assert!(!bitmap.is_empty());
    }

    #[test]
    fn test_bitmap_iter_fired() {
        let mut bitmap = DetectorBitmap::new(128);

        bitmap.set(5, true);
        bitmap.set(64, true);
        bitmap.set(100, true);

        let fired: Vec<usize> = bitmap.iter_fired().collect();
        assert_eq!(fired, vec![5, 64, 100]);
    }

    #[test]
    fn test_bitmap_xor() {
        let mut a = DetectorBitmap::new(64);
        a.set(0, true);
        a.set(5, true);
        a.set(10, true);

        let mut b = DetectorBitmap::new(64);
        b.set(5, true);
        b.set(10, true);
        b.set(20, true);

        let result = a.xor(&b);

        // 0: a=1, b=0 -> 1
        // 5: a=1, b=1 -> 0
        // 10: a=1, b=1 -> 0
        // 20: a=0, b=1 -> 1
        assert!(result.get(0));
        assert!(!result.get(5));
        assert!(!result.get(10));
        assert!(result.get(20));
        assert_eq!(result.fired_count(), 2);
    }

    #[test]
    fn test_bitmap_and_or() {
        let mut a = DetectorBitmap::new(64);
        a.set(0, true);
        a.set(5, true);

        let mut b = DetectorBitmap::new(64);
        b.set(5, true);
        b.set(10, true);

        let and_result = a.and(&b);
        assert!(!and_result.get(0));
        assert!(and_result.get(5));
        assert!(!and_result.get(10));
        assert_eq!(and_result.fired_count(), 1);

        let or_result = a.or(&b);
        assert!(or_result.get(0));
        assert!(or_result.get(5));
        assert!(or_result.get(10));
        assert_eq!(or_result.fired_count(), 3);
    }

    #[test]
    fn test_bitmap_clear() {
        let mut bitmap = DetectorBitmap::new(64);
        bitmap.set(0, true);
        bitmap.set(10, true);

        assert_eq!(bitmap.fired_count(), 2);

        bitmap.clear();

        assert_eq!(bitmap.fired_count(), 0);
        assert!(bitmap.is_empty());
    }

    #[test]
    fn test_bitmap_large() {
        let mut bitmap = DetectorBitmap::new(1024);

        // Set every 100th detector
        for i in (0..1024).step_by(100) {
            bitmap.set(i, true);
        }

        let fired: Vec<usize> = bitmap.iter_fired().collect();
        assert_eq!(fired.len(), 11); // 0, 100, 200, ..., 1000
    }

    #[test]
    #[should_panic(expected = "count exceeds maximum")]
    fn test_bitmap_overflow() {
        DetectorBitmap::new(2000);
    }

    // ---------- SyndromeRound tests ----------

    #[test]
    fn test_round_new() {
        let detectors = DetectorBitmap::new(64);
        let round = SyndromeRound::new(1, 100, 1000000, detectors, 5);

        assert_eq!(round.round_id, 1);
        assert_eq!(round.cycle, 100);
        assert_eq!(round.timestamp, 1000000);
        assert_eq!(round.source_tile, 5);
        assert_eq!(round.fired_count(), 0);
    }

    #[test]
    fn test_round_delta_to() {
        let mut d1 = DetectorBitmap::new(64);
        d1.set(0, true);
        d1.set(5, true);

        let mut d2 = DetectorBitmap::new(64);
        d2.set(5, true);
        d2.set(10, true);

        let round1 = SyndromeRound::new(1, 100, 1000, d1, 0);
        let round2 = SyndromeRound::new(2, 101, 2000, d2, 0);

        let delta = round1.delta_to(&round2);

        assert_eq!(delta.from_round, 1);
        assert_eq!(delta.to_round, 2);
        assert_eq!(delta.flip_count(), 2); // 0 and 10 flipped
    }

    // ---------- SyndromeBuffer tests ----------

    #[test]
    fn test_buffer_new() {
        let buffer = SyndromeBuffer::new(100);
        assert_eq!(buffer.capacity(), 100);
        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
        assert!(!buffer.is_full());
    }

    #[test]
    fn test_buffer_push() {
        let mut buffer = SyndromeBuffer::new(10);

        for i in 0..5 {
            let round = SyndromeRound::new(i, i, i * 1000, DetectorBitmap::new(64), 0);
            buffer.push(round);
        }

        assert_eq!(buffer.len(), 5);
        assert!(!buffer.is_full());
    }

    #[test]
    fn test_buffer_overflow() {
        let mut buffer = SyndromeBuffer::new(5);

        for i in 0..10 {
            let round = SyndromeRound::new(i, i, i * 1000, DetectorBitmap::new(64), 0);
            buffer.push(round);
        }

        assert_eq!(buffer.len(), 5);
        assert!(buffer.is_full());

        // Oldest should be round 5 (rounds 0-4 evicted)
        assert!(buffer.get(4).is_none());
        assert!(buffer.get(5).is_some());
        assert!(buffer.get(9).is_some());
    }

    #[test]
    fn test_buffer_window() {
        let mut buffer = SyndromeBuffer::new(100);

        for i in 0..50 {
            let round = SyndromeRound::new(i, i, i * 1000, DetectorBitmap::new(64), 0);
            buffer.push(round);
        }

        let window = buffer.window(10);
        assert_eq!(window.len(), 10);
        assert_eq!(window[0].round_id, 40);
        assert_eq!(window[9].round_id, 49);
    }

    #[test]
    fn test_buffer_window_larger_than_buffer() {
        let mut buffer = SyndromeBuffer::new(100);

        for i in 0..5 {
            let round = SyndromeRound::new(i, i, i * 1000, DetectorBitmap::new(64), 0);
            buffer.push(round);
        }

        let window = buffer.window(100);
        assert_eq!(window.len(), 5);
    }

    #[test]
    fn test_buffer_get() {
        let mut buffer = SyndromeBuffer::new(100);

        for i in 0..50 {
            let round = SyndromeRound::new(i, i, i * 1000, DetectorBitmap::new(64), 0);
            buffer.push(round);
        }

        assert!(buffer.get(0).is_some());
        assert!(buffer.get(49).is_some());
        assert!(buffer.get(50).is_none());
        assert!(buffer.get(1000).is_none());
    }

    #[test]
    fn test_buffer_statistics() {
        let mut buffer = SyndromeBuffer::new(10);

        for i in 0..20u64 {
            let mut detectors = DetectorBitmap::new(64);
            for j in 0..(i % 5) as usize {
                detectors.set(j, true);
            }
            let round = SyndromeRound::new(i, i, i * 1000, detectors, 0);
            buffer.push(round);
        }

        let stats = buffer.statistics();
        assert_eq!(stats.total_rounds, 20);
        assert_eq!(stats.current_size, 10);
        assert_eq!(stats.capacity, 10);
        assert_eq!(stats.evicted_rounds, 10);
        assert!(stats.avg_firing_rate > 0.0);
    }

    #[test]
    fn test_buffer_clear() {
        let mut buffer = SyndromeBuffer::new(10);

        for i in 0..5 {
            let round = SyndromeRound::new(i, i, i * 1000, DetectorBitmap::new(64), 0);
            buffer.push(round);
        }

        buffer.clear();

        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_buffer_iter() {
        let mut buffer = SyndromeBuffer::new(100);

        for i in 0..10 {
            let round = SyndromeRound::new(i, i, i * 1000, DetectorBitmap::new(64), 0);
            buffer.push(round);
        }

        let ids: Vec<u64> = buffer.iter().map(|r| r.round_id).collect();
        assert_eq!(ids, vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    #[should_panic(expected = "capacity must be positive")]
    fn test_buffer_zero_capacity() {
        SyndromeBuffer::new(0);
    }

    // ---------- SyndromeDelta tests ----------

    #[test]
    fn test_delta_compute() {
        let mut d1 = DetectorBitmap::new(64);
        d1.set(0, true);
        d1.set(5, true);

        let mut d2 = DetectorBitmap::new(64);
        d2.set(5, true);
        d2.set(10, true);

        let round1 = SyndromeRound::new(1, 100, 1000, d1, 0);
        let round2 = SyndromeRound::new(2, 101, 2000, d2, 0);

        let delta = SyndromeDelta::compute(&round1, &round2);

        assert_eq!(delta.from_round, 1);
        assert_eq!(delta.to_round, 2);
        assert_eq!(delta.flip_count(), 2);
        assert!(!delta.is_quiet());
    }

    #[test]
    fn test_delta_quiet() {
        let mut d1 = DetectorBitmap::new(64);
        d1.set(5, true);

        let mut d2 = DetectorBitmap::new(64);
        d2.set(5, true);

        let round1 = SyndromeRound::new(1, 100, 1000, d1, 0);
        let round2 = SyndromeRound::new(2, 101, 2000, d2, 0);

        let delta = SyndromeDelta::compute(&round1, &round2);

        assert!(delta.is_quiet());
        assert_eq!(delta.flip_count(), 0);
        assert_eq!(delta.activity_level(), 0.0);
    }

    #[test]
    fn test_delta_activity_level() {
        let mut d1 = DetectorBitmap::new(100);
        // All zeros

        let mut d2 = DetectorBitmap::new(100);
        for i in 0..10 {
            d2.set(i, true);
        }

        let round1 = SyndromeRound::new(1, 100, 1000, d1, 0);
        let round2 = SyndromeRound::new(2, 101, 2000, d2, 0);

        let delta = SyndromeDelta::compute(&round1, &round2);

        assert_eq!(delta.flip_count(), 10);
        assert!((delta.activity_level() - 0.1).abs() < 0.001);
    }

    #[test]
    fn test_delta_span() {
        let d1 = DetectorBitmap::new(64);
        let d2 = DetectorBitmap::new(64);

        let round1 = SyndromeRound::new(100, 100, 1000, d1, 0);
        let round2 = SyndromeRound::new(110, 110, 2000, d2, 0);

        let delta = SyndromeDelta::compute(&round1, &round2);

        assert_eq!(delta.span(), 10);
    }

    #[test]
    fn test_delta_iter_flipped() {
        let mut d1 = DetectorBitmap::new(64);
        d1.set(0, true);

        let mut d2 = DetectorBitmap::new(64);
        d2.set(10, true);
        d2.set(20, true);

        let round1 = SyndromeRound::new(1, 100, 1000, d1, 0);
        let round2 = SyndromeRound::new(2, 101, 2000, d2, 0);

        let delta = SyndromeDelta::compute(&round1, &round2);
        let flipped: Vec<usize> = delta.iter_flipped().collect();

        assert_eq!(flipped, vec![0, 10, 20]);
    }
}
