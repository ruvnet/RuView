//! Temporal tensor compression — 3-tier quantized CSI history (ADR-041).
//!
//! Circular buffer of 512 compressed CSI snapshots (8 phase + 8 amplitude).
//! Hot (last 64): 8-bit (<0.5% err), Warm (64-256): 5-bit (<3%), Cold (256-512): 3-bit (<15%).
//! Events: COMPRESSION_RATIO(705), TIER_TRANSITION(706), HISTORY_DEPTH_HOURS(707).

use libm::fabsf;

const SUBS: usize = 8;
const VALS: usize = SUBS * 2; // 8 phase + 8 amplitude
const CAP: usize = 512;
const HOT_END: usize = 64;
const WARM_END: usize = 256;
const HOT_Q: u32 = 255;
const WARM_Q: u32 = 31;
const COLD_Q: u32 = 7;
const RATE_ALPHA: f32 = 0.05;

pub const EVENT_COMPRESSION_RATIO: i32 = 705;
pub const EVENT_TIER_TRANSITION: i32 = 706;
pub const EVENT_HISTORY_DEPTH_HOURS: i32 = 707;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Tier { Hot = 0, Warm = 1, Cold = 2 }

impl Tier {
    const fn levels(self) -> u32 { match self { Tier::Hot => HOT_Q, Tier::Warm => WARM_Q, Tier::Cold => COLD_Q } }
    const fn for_age(age: usize) -> Self {
        if age < HOT_END { Tier::Hot } else if age < WARM_END { Tier::Warm } else { Tier::Cold }
    }
}

#[derive(Clone, Copy)]
struct Snap { data: [u8; VALS], scale: f32, tier: Tier, valid: bool }
impl Snap { const fn empty() -> Self { Self { data: [0; VALS], scale: 1.0, tier: Tier::Hot, valid: false } } }

fn quantize(v: f32, scale: f32, levels: u32) -> u8 {
    if scale < 1e-9 { return (levels / 2) as u8; }
    let n = ((v / scale + 1.0) * 0.5).max(0.0).min(1.0);
    let q = (n * levels as f32 + 0.5) as u32;
    if q > levels { levels as u8 } else { q as u8 }
}

fn dequantize(q: u8, scale: f32, levels: u32) -> f32 {
    (q as f32 / levels as f32 * 2.0 - 1.0) * scale
}

/// Temporal tensor compressor for CSI history.
pub struct TemporalCompressor {
    buf: [Snap; CAP],
    w_idx: usize,
    total: u32,
    frame_rate: f32,
    prev_ts: u32,
    has_ts: bool,
    ratio: f32,
}

impl TemporalCompressor {
    pub const fn new() -> Self {
        const E: Snap = Snap::empty();
        Self { buf: [E; CAP], w_idx: 0, total: 0, frame_rate: 20.0, prev_ts: 0, has_ts: false, ratio: 1.0 }
    }

    fn occ(&self) -> usize { if (self.total as usize) < CAP { self.total as usize } else { CAP } }

    /// Store a frame. Returns events to emit.
    pub fn push_frame(&mut self, phases: &[f32], amps: &[f32], ts_ms: u32) -> &[(i32, f32)] {
        let np = phases.len().min(SUBS);
        let na = amps.len().min(SUBS);
        let mut vals = [0.0f32; VALS];
        let mut i = 0;
        while i < np { vals[i] = phases[i]; i += 1; }
        i = 0;
        while i < na { vals[SUBS + i] = amps[i]; i += 1; }

        // Scale + quantize at hot tier.
        let mut mx = 0.0f32;
        i = 0;
        while i < VALS { let a = fabsf(vals[i]); if a > mx { mx = a; } i += 1; }
        let scale = if mx < 1e-9 { 1.0 } else { mx };
        let mut snap = Snap::empty();
        snap.scale = scale; snap.tier = Tier::Hot; snap.valid = true;
        i = 0;
        while i < VALS { snap.data[i] = quantize(vals[i], scale, HOT_Q); i += 1; }
        self.buf[self.w_idx] = snap;
        self.w_idx = (self.w_idx + 1) % CAP;
        self.total = self.total.wrapping_add(1);

        // Frame rate EMA.
        if self.has_ts && ts_ms > self.prev_ts {
            let dt = ts_ms - self.prev_ts;
            if dt > 0 && dt < 5000 {
                let r = 1000.0 / dt as f32;
                self.frame_rate = RATE_ALPHA * r + (1.0 - RATE_ALPHA) * self.frame_rate;
            }
        }
        self.prev_ts = ts_ms; self.has_ts = true;

        static mut EV: [(i32, f32); 4] = [(0, 0.0); 4];
        let mut ne = 0usize;
        let occ = self.occ();

        // Re-quantize at tier boundaries.
        for &ba in &[HOT_END, WARM_END] {
            if occ > ba {
                let slot = (self.w_idx + CAP - ba - 1) % CAP;
                let new_t = Tier::for_age(ba);
                if self.buf[slot].valid && self.buf[slot].tier != new_t {
                    let old_l = self.buf[slot].tier.levels();
                    let new_l = new_t.levels();
                    let s = self.buf[slot].scale;
                    let mut j = 0;
                    while j < VALS { let d = dequantize(self.buf[slot].data[j], s, old_l); self.buf[slot].data[j] = quantize(d, s, new_l); j += 1; }
                    self.buf[slot].tier = new_t;
                    if ne < 4 { unsafe { EV[ne] = (EVENT_TIER_TRANSITION, new_t as i32 as f32); } ne += 1; }
                }
            }
        }
        self.ratio = self.calc_ratio(occ);
        if self.total % 64 == 0 && ne < 4 { unsafe { EV[ne] = (EVENT_COMPRESSION_RATIO, self.ratio); } ne += 1; }
        unsafe { &EV[..ne] }
    }

    /// Periodic timer events.
    pub fn on_timer(&self) -> &[(i32, f32)] {
        static mut TE: [(i32, f32); 2] = [(0, 0.0); 2];
        let mut n = 0;
        let h = self.history_hours();
        if h > 0.0 { unsafe { TE[n] = (EVENT_HISTORY_DEPTH_HOURS, h); } n += 1; }
        unsafe { TE[n] = (EVENT_COMPRESSION_RATIO, self.ratio); } n += 1;
        unsafe { &TE[..n] }
    }

    fn calc_ratio(&self, occ: usize) -> f32 {
        if occ == 0 { return 1.0; }
        let raw = occ * VALS * 4;
        let mut hot = 0usize; let mut warm = 0usize; let mut cold = 0usize;
        let mut k = 0;
        while k < occ {
            let s = (self.w_idx + CAP - 1 - k) % CAP;
            if self.buf[s].valid { match self.buf[s].tier { Tier::Hot => hot += 1, Tier::Warm => warm += 1, Tier::Cold => cold += 1 } }
            k += 1;
        }
        let oh = 5; // scale(4) + tier(1) per snap
        let comp = hot * (VALS + oh) + warm * ((VALS * 5 + 7) / 8 + oh) + cold * ((VALS * 3 + 7) / 8 + oh);
        if comp == 0 { 1.0 } else { raw as f32 / comp as f32 }
    }

    fn history_hours(&self) -> f32 {
        if self.frame_rate < 0.01 { return 0.0; }
        self.occ() as f32 / self.frame_rate / 3600.0
    }

    /// Retrieve decompressed snapshot by age (0 = newest).
    pub fn get_snapshot(&self, age: usize) -> Option<[f32; VALS]> {
        if age >= self.occ() { return None; }
        let s = &self.buf[(self.w_idx + CAP - 1 - age) % CAP];
        if !s.valid { return None; }
        let l = s.tier.levels();
        let mut out = [0.0f32; VALS];
        let mut i = 0;
        while i < VALS { out[i] = dequantize(s.data[i], s.scale, l); i += 1; }
        Some(out)
    }

    pub fn compression_ratio(&self) -> f32 { self.ratio }
    pub fn frame_rate(&self) -> f32 { self.frame_rate }
    pub fn total_written(&self) -> u32 { self.total }
    pub fn occupied(&self) -> usize { self.occ() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init() { let tc = TemporalCompressor::new(); assert_eq!(tc.total_written(), 0); assert_eq!(tc.occupied(), 0); }

    #[test]
    fn test_push_retrieve() {
        let mut tc = TemporalCompressor::new();
        let ph = [1.0f32, 0.5, -0.3, 0.7, -1.2, 0.1, 0.0, 0.9];
        let am = [2.0f32, 3.5, 1.2, 4.0, 0.8, 2.2, 1.5, 3.0];
        tc.push_frame(&ph, &am, 0);
        let snap = tc.get_snapshot(0).unwrap();
        for i in 0..8 { assert!(fabsf(snap[i] - ph[i]) < fabsf(ph[i]) * 0.02 + 0.15, "phase[{}] err", i); }
    }

    #[test]
    fn test_tiers() {
        assert_eq!(Tier::for_age(0), Tier::Hot); assert_eq!(Tier::for_age(63), Tier::Hot);
        assert_eq!(Tier::for_age(64), Tier::Warm); assert_eq!(Tier::for_age(255), Tier::Warm);
        assert_eq!(Tier::for_age(256), Tier::Cold); assert_eq!(Tier::for_age(511), Tier::Cold);
    }

    #[test]
    fn test_hot_quantize() {
        let s = 3.14;
        for &v in &[-3.14f32, -1.0, 0.0, 1.0, 3.14] {
            let d = dequantize(quantize(v, s, HOT_Q), s, HOT_Q);
            let e = if fabsf(v) > 0.01 { fabsf(d - v) / fabsf(v) } else { fabsf(d - v) };
            assert!(e < 0.02, "hot: v={v} d={d} e={e}");
        }
    }

    #[test]
    fn test_ratio_increases() {
        let mut tc = TemporalCompressor::new();
        let p = [0.5f32; 8]; let a = [1.0f32; 8];
        for i in 0..300u32 { tc.push_frame(&p, &a, i * 50); }
        assert!(tc.compression_ratio() > 1.0, "ratio={}", tc.compression_ratio());
    }

    #[test]
    fn test_wrap() {
        let mut tc = TemporalCompressor::new();
        let p = [0.1f32; 8]; let a = [0.2f32; 8];
        for i in 0..600u32 { tc.push_frame(&p, &a, i * 50); }
        assert_eq!(tc.occupied(), CAP); assert!(tc.get_snapshot(0).is_some()); assert!(tc.get_snapshot(CAP).is_none());
    }

    #[test]
    fn test_frame_rate() {
        let mut tc = TemporalCompressor::new();
        let p = [0.0f32; 8]; let a = [1.0f32; 8];
        for i in 0..100u32 { tc.push_frame(&p, &a, i * 50); }
        assert!(tc.frame_rate() > 15.0 && tc.frame_rate() < 25.0, "rate={}", tc.frame_rate());
    }

    #[test]
    fn test_timer() {
        let mut tc = TemporalCompressor::new();
        let p = [0.0f32; 8]; let a = [1.0f32; 8];
        for i in 0..100u32 { tc.push_frame(&p, &a, i * 50); }
        let ev = tc.on_timer();
        assert!(ev.iter().any(|&(t, _)| t == EVENT_COMPRESSION_RATIO));
    }
}
