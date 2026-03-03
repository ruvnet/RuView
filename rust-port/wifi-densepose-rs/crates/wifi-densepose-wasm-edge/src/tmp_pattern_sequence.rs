//! Temporal pattern sequence detector -- ADR-041 WASM edge module.
//!
//! Detects recurring daily activity patterns via LCS (Longest Common Subsequence).
//! Each minute is discretized into a motion symbol, stored in a 24-hour circular
//! buffer (1440 entries). Hourly LCS comparison yields routine confidence.
//!
//! Event IDs: 790-793 (Temporal category).

const DAY_LEN: usize = 1440;  // Symbols per day (1/min * 24h).
const MAX_PATTERNS: usize = 32;
const PATTERN_LEN: usize = 16;
const MIN_PATTERN_LEN: usize = 5;
const LCS_WINDOW: usize = 60;  // 1 hour comparison window.
const THRESH_STILL: f32 = 0.05;
const THRESH_LOW: f32 = 0.3;
const THRESH_HIGH: f32 = 0.7;

pub const EVENT_PATTERN_DETECTED: i32 = 790;
pub const EVENT_PATTERN_CONFIDENCE: i32 = 791;
pub const EVENT_ROUTINE_DEVIATION: i32 = 792;
pub const EVENT_PREDICTION_NEXT: i32 = 793;

#[derive(Clone, Copy, Debug, PartialEq)] #[repr(u8)]
pub enum Symbol { Empty=0, Still=1, LowMotion=2, HighMotion=3, MultiPerson=4 }
impl Symbol {
    pub fn from_readings(presence: i32, motion: f32, n_persons: i32) -> Self {
        if presence == 0 { Symbol::Empty }
        else if n_persons > 1 { Symbol::MultiPerson }
        else if motion > THRESH_HIGH { Symbol::HighMotion }
        else if motion > THRESH_LOW { Symbol::LowMotion }
        else { Symbol::Still }
    }
}

#[derive(Clone, Copy)]
struct PatternEntry { symbols: [u8; PATTERN_LEN], len: u8, hit_count: u16 }
impl PatternEntry { const fn empty() -> Self { Self { symbols: [0; PATTERN_LEN], len: 0, hit_count: 0 } } }

/// Temporal pattern sequence analyzer.
pub struct PatternSequenceAnalyzer {
    /// Two-day history: [0..DAY_LEN)=yesterday, [DAY_LEN..2*DAY_LEN)=today.
    history: [u8; DAY_LEN * 2],
    minute_counter: u16,
    day_offset: u32,
    pattern_lib: [PatternEntry; MAX_PATTERNS],
    n_patterns: u8,
    routine_confidence: f32,
    frame_votes: [u16; 5],
    frames_in_minute: u16,
    timer_count: u32,
    lcs_prev: [u16; LCS_WINDOW + 1],
    lcs_curr: [u16; LCS_WINDOW + 1],
}

impl PatternSequenceAnalyzer {
    pub const fn new() -> Self {
        Self {
            history: [0; DAY_LEN * 2], minute_counter: 0, day_offset: 0,
            pattern_lib: [PatternEntry::empty(); MAX_PATTERNS], n_patterns: 0,
            routine_confidence: 0.0, frame_votes: [0; 5], frames_in_minute: 0,
            timer_count: 0, lcs_prev: [0; LCS_WINDOW + 1], lcs_curr: [0; LCS_WINDOW + 1],
        }
    }

    /// Called per CSI frame (~20 Hz). Accumulates votes for current minute.
    pub fn on_frame(&mut self, presence: i32, motion: f32, n_persons: i32) {
        let idx = Symbol::from_readings(presence, motion, n_persons) as usize;
        if idx < 5 { self.frame_votes[idx] = self.frame_votes[idx].saturating_add(1); }
        self.frames_in_minute = self.frames_in_minute.saturating_add(1);
    }

    /// Called at ~1 Hz. Commits symbols and runs hourly LCS comparison.
    pub fn on_timer(&mut self) -> &[(i32, f32)] {
        self.timer_count += 1;
        static mut EVENTS: [(i32, f32); 4] = [(0, 0.0); 4];
        let mut n = 0usize;

        if self.timer_count % 60 == 0 && self.frames_in_minute > 0 {
            let sym = self.majority_symbol();
            let idx = DAY_LEN + self.minute_counter as usize;
            if idx < DAY_LEN * 2 { self.history[idx] = sym as u8; }
            // Deviation check against yesterday.
            if self.day_offset > 0 {
                let predicted = self.history[self.minute_counter as usize];
                if sym as u8 != predicted && n < 4 {
                    unsafe { EVENTS[n] = (EVENT_ROUTINE_DEVIATION, self.minute_counter as f32); }
                    n += 1;
                }
                let next_min = (self.minute_counter + 1) % DAY_LEN as u16;
                if n < 4 {
                    unsafe { EVENTS[n] = (EVENT_PREDICTION_NEXT, self.history[next_min as usize] as f32); }
                    n += 1;
                }
            }
            self.minute_counter += 1;
            if self.minute_counter >= DAY_LEN as u16 { self.rollover_day(); self.minute_counter = 0; }
            self.frame_votes = [0; 5]; self.frames_in_minute = 0;
        }

        if self.timer_count % 3600 == 0 && self.day_offset > 0 {
            let end = self.minute_counter as usize;
            let start = if end >= LCS_WINDOW { end - LCS_WINDOW } else { 0 };
            let wlen = end - start;
            if wlen >= MIN_PATTERN_LEN {
                let lcs = self.compute_lcs(start, wlen);
                self.routine_confidence = if wlen > 0 { lcs as f32 / wlen as f32 } else { 0.0 };
                if n < 4 { unsafe { EVENTS[n] = (EVENT_PATTERN_CONFIDENCE, self.routine_confidence); } n += 1; }
                if lcs >= MIN_PATTERN_LEN {
                    self.store_pattern(start, wlen);
                    if n < 4 { unsafe { EVENTS[n] = (EVENT_PATTERN_DETECTED, lcs as f32); } n += 1; }
                }
            }
        }
        unsafe { &EVENTS[..n] }
    }

    fn majority_symbol(&self) -> Symbol {
        let mut best = 0u8; let mut bc = 0u16; let mut i = 0u8;
        while (i as usize) < 5 {
            if self.frame_votes[i as usize] > bc { bc = self.frame_votes[i as usize]; best = i; }
            i += 1;
        }
        match best { 0=>Symbol::Empty, 1=>Symbol::Still, 2=>Symbol::LowMotion,
                      3=>Symbol::HighMotion, 4=>Symbol::MultiPerson, _=>Symbol::Empty }
    }

    fn rollover_day(&mut self) {
        let mut i = 0usize;
        while i < DAY_LEN { self.history[i] = self.history[DAY_LEN + i]; i += 1; }
        i = 0;
        while i < DAY_LEN { self.history[DAY_LEN + i] = 0; i += 1; }
        self.day_offset += 1;
    }

    /// Two-row DP LCS between yesterday[start..start+len] and today[start..start+len].
    fn compute_lcs(&mut self, start: usize, len: usize) -> usize {
        let len = len.min(LCS_WINDOW);
        let mut j = 0usize;
        while j <= len { self.lcs_prev[j] = 0; self.lcs_curr[j] = 0; j += 1; }
        let mut i = 1usize;
        while i <= len {
            j = 1;
            while j <= len {
                let y = self.history[start + i - 1];
                let t = self.history[DAY_LEN + start + j - 1];
                self.lcs_curr[j] = if y == t { self.lcs_prev[j - 1] + 1 }
                    else if self.lcs_prev[j] >= self.lcs_curr[j - 1] { self.lcs_prev[j] }
                    else { self.lcs_curr[j - 1] };
                j += 1;
            }
            j = 0;
            while j <= len { self.lcs_prev[j] = self.lcs_curr[j]; self.lcs_curr[j] = 0; j += 1; }
            i += 1;
        }
        self.lcs_prev[len] as usize
    }

    fn store_pattern(&mut self, start: usize, len: usize) {
        let pl = len.min(PATTERN_LEN);
        let mut cand = [0u8; PATTERN_LEN];
        let mut k = 0usize;
        while k < pl { cand[k] = self.history[DAY_LEN + start + k]; k += 1; }
        // Check existing patterns.
        let mut p = 0usize;
        while p < self.n_patterns as usize {
            if self.pattern_lib[p].len as usize >= pl {
                let mut m = true; k = 0;
                while k < pl { if self.pattern_lib[p].symbols[k] != cand[k] { m = false; break; } k += 1; }
                if m { self.pattern_lib[p].hit_count = self.pattern_lib[p].hit_count.saturating_add(1); return; }
            }
            p += 1;
        }
        if (self.n_patterns as usize) < MAX_PATTERNS {
            let idx = self.n_patterns as usize;
            self.pattern_lib[idx].symbols = cand;
            self.pattern_lib[idx].len = pl as u8;
            self.pattern_lib[idx].hit_count = 1;
            self.n_patterns += 1;
        }
    }

    pub fn routine_confidence(&self) -> f32 { self.routine_confidence }
    pub fn pattern_count(&self) -> u8 { self.n_patterns }
    pub fn current_minute(&self) -> u16 { self.minute_counter }
    pub fn day_offset(&self) -> u32 { self.day_offset }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test] fn test_symbol_discretization() {
        assert_eq!(Symbol::from_readings(0, 0.0, 0), Symbol::Empty);
        assert_eq!(Symbol::from_readings(1, 0.02, 1), Symbol::Still);
        assert_eq!(Symbol::from_readings(1, 0.5, 1), Symbol::LowMotion);
        assert_eq!(Symbol::from_readings(1, 0.9, 1), Symbol::HighMotion);
        assert_eq!(Symbol::from_readings(1, 0.5, 3), Symbol::MultiPerson);
    }

    #[test] fn test_init() {
        let a = PatternSequenceAnalyzer::new();
        assert_eq!(a.current_minute(), 0);
        assert_eq!(a.day_offset(), 0);
        assert_eq!(a.pattern_count(), 0);
    }

    #[test] fn test_frame_accumulation() {
        let mut a = PatternSequenceAnalyzer::new();
        for _ in 0..60 { a.on_frame(1, 0.5, 1); }
        assert_eq!(a.majority_symbol(), Symbol::LowMotion);
    }

    #[test] fn test_minute_commit() {
        let mut a = PatternSequenceAnalyzer::new();
        for _ in 0..20 { a.on_frame(1, 0.5, 1); }
        for _ in 0..60 { a.on_timer(); }
        assert_eq!(a.current_minute(), 1);
    }

    #[test] fn test_day_rollover() {
        let mut a = PatternSequenceAnalyzer::new();
        a.minute_counter = DAY_LEN as u16 - 1;
        a.frames_in_minute = 10; a.frame_votes[2] = 10;
        for _ in 0..60 { a.on_timer(); }
        assert_eq!(a.day_offset(), 1);
        assert_eq!(a.current_minute(), 0);
    }

    #[test] fn test_lcs_identical() {
        let mut a = PatternSequenceAnalyzer::new();
        for i in 0..60 { let s = (i % 5) as u8; a.history[i] = s; a.history[DAY_LEN + i] = s; }
        a.day_offset = 1;
        assert_eq!(a.compute_lcs(0, 60), 60);
    }

    #[test] fn test_lcs_different() {
        let mut a = PatternSequenceAnalyzer::new();
        for i in 0..20 { a.history[i] = 1; a.history[DAY_LEN + i] = 2; }
        a.day_offset = 1;
        assert_eq!(a.compute_lcs(0, 20), 0);
    }

    #[test] fn test_pattern_storage() {
        let mut a = PatternSequenceAnalyzer::new();
        for i in 0..10 { a.history[DAY_LEN + i] = (i % 3) as u8; }
        a.store_pattern(0, 10);
        assert_eq!(a.pattern_count(), 1);
        a.store_pattern(0, 10); // duplicate -> increment hit count
        assert_eq!(a.pattern_count(), 1);
    }
}
