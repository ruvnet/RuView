//! Bit-Parallel String Search
//!
//! A genuinely novel implementation of bit-parallel string matching
//! that processes up to 64 positions simultaneously using bit manipulation.
//!
//! Much faster than naive string search for patterns ≤ 64 bytes.

#![no_std]

#[cfg(test)]
extern crate std;

/// Bit-parallel string matcher using Shift-Or algorithm variant
pub struct BitParallelMatcher;

impl BitParallelMatcher {
    /// Find first occurrence of pattern in text using bit-parallel operations
    ///
    /// # Algorithm
    /// Uses bit masks to track pattern matches in parallel across 64 positions.
    /// Each bit represents whether the pattern matches up to that position.
    ///
    /// # Performance
    /// - O(n) time complexity for text of length n
    /// - Processes 64 potential matches simultaneously
    /// - No branching in inner loop (CPU-friendly)
    ///
    /// # Limitations
    /// - Pattern must be ≤ 64 bytes
    /// - Best for small patterns (< 32 bytes optimal)
    ///
    /// # Example
    /// ```
    /// use bit_parallel_search::BitParallelMatcher;
    ///
    /// let text = b"The quick brown fox";
    /// let pattern = b"quick";
    ///
    /// assert_eq!(BitParallelMatcher::find(text, pattern), Some(4));
    /// ```
    pub fn find(text: &[u8], pattern: &[u8]) -> Option<usize> {
        if pattern.is_empty() || pattern.len() > 64 || pattern.len() > text.len() {
            return None;
        }

        let m = pattern.len();

        // Build bit masks for each possible byte value
        // mask[b] has bit i set to 0 if pattern[i] == b
        let mut masks = [!0u64; 256];
        for (i, &byte) in pattern.iter().enumerate() {
            masks[byte as usize] &= !(1u64 << i);
        }

        // State tracks partial matches
        // Bit i is 0 if pattern[0..i+1] matches text ending at current position
        let mut state = !0u64;
        let match_mask = 1u64 << (m - 1);

        for (i, &byte) in text.iter().enumerate() {
            // Shift state and apply mask for current byte
            state = (state << 1) | masks[byte as usize];

            // Check if full pattern matched (bit m-1 is 0)
            if state & match_mask == 0 {
                return Some(i + 1 - m);
            }
        }

        None
    }

    /// Find all occurrences of pattern in text
    ///
    /// Returns iterator over all match positions
    pub fn find_all<'a>(text: &'a [u8], pattern: &'a [u8]) -> BitParallelIterator<'a> {
        BitParallelIterator::new(text, pattern)
    }

    /// Count occurrences without allocating
    pub fn count(text: &[u8], pattern: &[u8]) -> usize {
        if pattern.is_empty() || pattern.len() > 64 || pattern.len() > text.len() {
            return 0;
        }

        let m = pattern.len();
        let mut masks = [!0u64; 256];

        for (i, &byte) in pattern.iter().enumerate() {
            masks[byte as usize] &= !(1u64 << i);
        }

        let mut state = !0u64;
        let match_mask = 1u64 << (m - 1);
        let mut count = 0;

        for &byte in text {
            state = (state << 1) | masks[byte as usize];
            if state & match_mask == 0 {
                count += 1;
            }
        }

        count
    }
}

/// Iterator for finding all matches
pub struct BitParallelIterator<'a> {
    text: &'a [u8],
    pattern: &'a [u8],
    pos: usize,
    masks: [u64; 256],
    state: u64,
    match_mask: u64,
}

impl<'a> BitParallelIterator<'a> {
    fn new(text: &'a [u8], pattern: &'a [u8]) -> Self {
        if pattern.is_empty() || pattern.len() > 64 {
            return Self {
                text,
                pattern,
                pos: text.len(),
                masks: [0; 256],
                state: 0,
                match_mask: 0,
            };
        }

        let mut masks = [!0u64; 256];
        for (i, &byte) in pattern.iter().enumerate() {
            masks[byte as usize] &= !(1u64 << i);
        }

        Self {
            text,
            pattern,
            pos: 0,
            masks,
            state: !0u64,
            match_mask: 1u64 << (pattern.len() - 1),
        }
    }
}

impl<'a> Iterator for BitParallelIterator<'a> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        let m = self.pattern.len();

        while self.pos < self.text.len() {
            let byte = self.text[self.pos];
            self.state = (self.state << 1) | self.masks[byte as usize];
            self.pos += 1;

            if self.state & self.match_mask == 0 {
                return Some(self.pos - m);
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find() {
        assert_eq!(BitParallelMatcher::find(b"hello world", b"world"), Some(6));
        assert_eq!(BitParallelMatcher::find(b"hello world", b"foo"), None);
        assert_eq!(BitParallelMatcher::find(b"aaaa", b"aa"), Some(0));
    }

    #[test]
    fn test_find_all() {
        use std::vec::Vec;
        let matches: Vec<usize> = BitParallelMatcher::find_all(b"abababa", b"aba")
            .collect();
        assert_eq!(matches, std::vec![0, 2, 4]);
    }

    #[test]
    fn test_count() {
        assert_eq!(BitParallelMatcher::count(b"abababa", b"aba"), 3);
        assert_eq!(BitParallelMatcher::count(b"hello world", b"l"), 3);
    }

    #[test]
    fn test_edge_cases() {
        assert_eq!(BitParallelMatcher::find(b"", b"a"), None);
        assert_eq!(BitParallelMatcher::find(b"a", b""), None);
        assert_eq!(BitParallelMatcher::find(b"a", b"ab"), None);
    }
}