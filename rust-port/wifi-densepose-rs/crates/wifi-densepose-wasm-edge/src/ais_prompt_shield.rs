//! CSI signal integrity shield — ADR-041 AI Security module.
//!
//! Detects replay, injection, and jamming attacks on the CSI data stream.
//! - **Replay**: FNV-1a hash of quantized features; match against 64-entry ring.
//! - **Injection**: >25% subcarriers with >10x amplitude jump from previous frame.
//! - **Jamming**: SNR proxy < 10% of baseline for 5+ consecutive frames.
//!
//! Events: REPLAY_ATTACK(820), INJECTION_DETECTED(821), JAMMING_DETECTED(822),
//!         SIGNAL_INTEGRITY(823).  Budget: S (< 5 ms).

#[cfg(not(feature = "std"))]
use libm::{log10f, sqrtf};
#[cfg(feature = "std")]
fn sqrtf(x: f32) -> f32 { x.sqrt() }
#[cfg(feature = "std")]
fn log10f(x: f32) -> f32 { x.log10() }

const MAX_SC: usize = 32;
const HASH_RING: usize = 64;
const FNV_OFFSET: u32 = 2166136261;
const FNV_PRIME: u32 = 16777619;
const INJECTION_FACTOR: f32 = 10.0;
const INJECTION_FRAC: f32 = 0.25;
const JAMMING_SNR_FRAC: f32 = 0.10;
const JAMMING_CONSEC: u8 = 5;
const BASELINE_FRAMES: u32 = 100;
const COOLDOWN: u16 = 40;

pub const EVENT_REPLAY_ATTACK: i32 = 820;
pub const EVENT_INJECTION_DETECTED: i32 = 821;
pub const EVENT_JAMMING_DETECTED: i32 = 822;
pub const EVENT_SIGNAL_INTEGRITY: i32 = 823;

/// CSI signal integrity shield.
pub struct PromptShield {
    hashes: [u32; HASH_RING],
    hash_len: usize,
    hash_idx: usize,
    prev_amps: [f32; MAX_SC],
    amps_init: bool,
    baseline_snr: f32,
    cal_amp: f32,
    cal_var: f32,
    cal_n: u32,
    calibrated: bool,
    low_snr_run: u8,
    frame_count: u32,
    cd_replay: u16,
    cd_inject: u16,
    cd_jam: u16,
}

impl PromptShield {
    pub const fn new() -> Self {
        Self {
            hashes: [0; HASH_RING], hash_len: 0, hash_idx: 0,
            prev_amps: [0.0; MAX_SC], amps_init: false,
            baseline_snr: 0.0, cal_amp: 0.0, cal_var: 0.0, cal_n: 0,
            calibrated: false, low_snr_run: 0, frame_count: 0,
            cd_replay: 0, cd_inject: 0, cd_jam: 0,
        }
    }

    /// Process one CSI frame. Returns `(event_id, value)` pairs.
    pub fn process_frame(&mut self, phases: &[f32], amps: &[f32]) -> &[(i32, f32)] {
        let n = phases.len().min(amps.len()).min(MAX_SC);
        if n < 2 { return &[]; }
        self.frame_count += 1;
        self.cd_replay = self.cd_replay.saturating_sub(1);
        self.cd_inject = self.cd_inject.saturating_sub(1);
        self.cd_jam = self.cd_jam.saturating_sub(1);

        static mut EV: [(i32, f32); 4] = [(0, 0.0); 4];
        let mut ne = 0usize;

        // Frame features: mean phase, mean amp, amp variance.
        let (mut m_ph, mut m_a) = (0.0f32, 0.0f32);
        for i in 0..n { m_ph += phases[i]; m_a += amps[i]; }
        m_ph /= n as f32; m_a /= n as f32;
        let mut a_var = 0.0f32;
        for i in 0..n { let d = amps[i] - m_a; a_var += d * d; }
        a_var /= n as f32;

        // ── Calibration ─────────────────────────────────────────────────
        if !self.calibrated {
            self.cal_amp += m_a;
            self.cal_var += a_var;
            self.cal_n += 1;
            if !self.amps_init {
                for i in 0..n { self.prev_amps[i] = amps[i]; }
                self.amps_init = true;
            }
            if self.cal_n >= BASELINE_FRAMES {
                let cnt = self.cal_n as f32;
                self.baseline_snr = (self.cal_amp / cnt)
                    / sqrtf((self.cal_var / cnt).max(0.0001));
                self.calibrated = true;
            }
            let h = self.fnv1a(m_ph, m_a, a_var);
            self.push_hash(h);
            return unsafe { &EV[..0] };
        }

        // ── 1. Replay ───────────────────────────────────────────────────
        let h = self.fnv1a(m_ph, m_a, a_var);
        let replay = self.has_hash(h);
        self.push_hash(h);
        if replay && self.cd_replay == 0 {
            unsafe { EV[ne] = (EVENT_REPLAY_ATTACK, 1.0); }
            ne += 1; self.cd_replay = COOLDOWN;
        }

        // ── 2. Injection ────────────────────────────────────────────────
        let inj_f = if self.amps_init {
            let mut jc = 0u32;
            for i in 0..n {
                if self.prev_amps[i] > 0.0001 && amps[i] / self.prev_amps[i] > INJECTION_FACTOR {
                    jc += 1;
                }
            }
            jc as f32 / n as f32
        } else { 0.0 };
        if inj_f >= INJECTION_FRAC && self.cd_inject == 0 && ne < 4 {
            unsafe { EV[ne] = (EVENT_INJECTION_DETECTED, inj_f); }
            ne += 1; self.cd_inject = COOLDOWN;
        }

        // ── 3. Jamming ──────────────────────────────────────────────────
        let sd = sqrtf(a_var.max(0.0001));
        let cur_snr = if sd > 0.0001 { m_a / sd } else { 0.0 };
        if self.baseline_snr > 0.0 && cur_snr < self.baseline_snr * JAMMING_SNR_FRAC {
            self.low_snr_run = self.low_snr_run.saturating_add(1);
        } else { self.low_snr_run = 0; }
        if self.low_snr_run >= JAMMING_CONSEC && self.cd_jam == 0 && ne < 4 {
            let r = if cur_snr > 0.0001 { self.baseline_snr / cur_snr } else { 1000.0 };
            unsafe { EV[ne] = (EVENT_JAMMING_DETECTED, 10.0 * log10f(r)); }
            ne += 1; self.cd_jam = COOLDOWN;
        }

        // ── 4. Integrity (periodic) ─────────────────────────────────────
        if self.frame_count % 20 == 0 && ne < 4 {
            let mut s = 1.0f32;
            if replay { s -= 0.4; }
            if inj_f > 0.0 { s -= (inj_f / INJECTION_FRAC).min(1.0) * 0.3; }
            if self.baseline_snr > 0.0 && cur_snr < self.baseline_snr {
                let r = cur_snr / self.baseline_snr;
                if r < 0.5 { s -= (1.0 - r * 2.0).min(0.3); }
            }
            unsafe { EV[ne] = (EVENT_SIGNAL_INTEGRITY, if s < 0.0 { 0.0 } else { s }); }
            ne += 1;
        }

        for i in 0..n { self.prev_amps[i] = amps[i]; }
        unsafe { &EV[..ne] }
    }

    fn fnv1a(&self, ph: f32, amp: f32, var: f32) -> u32 {
        let mut h = FNV_OFFSET;
        for v in [(ph * 100.0) as i32, (amp * 100.0) as i32, (var * 100.0) as i32] {
            for &b in &v.to_le_bytes() { h ^= b as u32; h = h.wrapping_mul(FNV_PRIME); }
        }
        h
    }
    fn push_hash(&mut self, h: u32) {
        self.hashes[self.hash_idx] = h;
        self.hash_idx = (self.hash_idx + 1) % HASH_RING;
        if self.hash_len < HASH_RING { self.hash_len += 1; }
    }
    fn has_hash(&self, h: u32) -> bool {
        for i in 0..self.hash_len { if self.hashes[i] == h { return true; } }
        false
    }
    pub fn frame_count(&self) -> u32 { self.frame_count }
    pub fn is_calibrated(&self) -> bool { self.calibrated }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init() {
        let ps = PromptShield::new();
        assert_eq!(ps.frame_count(), 0);
        assert!(!ps.is_calibrated());
    }

    #[test]
    fn test_calibration() {
        let mut ps = PromptShield::new();
        for _ in 0..BASELINE_FRAMES {
            ps.process_frame(&[0.5; 16], &[1.0; 16]);
        }
        assert!(ps.is_calibrated());
    }

    #[test]
    fn test_normal_no_alerts() {
        let mut ps = PromptShield::new();
        for i in 0..BASELINE_FRAMES {
            ps.process_frame(&[(i as f32) * 0.01; 16], &[1.0; 16]);
        }
        for i in 0..50u32 {
            let ev = ps.process_frame(&[5.0 + (i as f32) * 0.03; 16], &[1.0; 16]);
            for &(et, _) in ev {
                assert_ne!(et, EVENT_REPLAY_ATTACK);
                assert_ne!(et, EVENT_INJECTION_DETECTED);
                assert_ne!(et, EVENT_JAMMING_DETECTED);
            }
        }
    }

    #[test]
    fn test_replay_detection() {
        let mut ps = PromptShield::new();
        for i in 0..BASELINE_FRAMES {
            ps.process_frame(&[(i as f32) * 0.02; 16], &[1.0; 16]);
        }
        let rp = [99.0f32; 16]; let ra = [2.5f32; 16];
        ps.process_frame(&rp, &ra);
        let ev = ps.process_frame(&rp, &ra);
        assert!(ev.iter().any(|&(t,_)| t == EVENT_REPLAY_ATTACK), "replay not detected");
    }

    #[test]
    fn test_injection_detection() {
        let mut ps = PromptShield::new();
        for i in 0..BASELINE_FRAMES {
            ps.process_frame(&[(i as f32) * 0.01; 16], &[1.0; 16]);
        }
        ps.process_frame(&[3.14; 16], &[1.0; 16]);
        let ev = ps.process_frame(&[3.15; 16], &[15.0; 16]);
        assert!(ev.iter().any(|&(t,_)| t == EVENT_INJECTION_DETECTED), "injection not detected");
    }

    #[test]
    fn test_jamming_detection() {
        let mut ps = PromptShield::new();
        // Calibrate baseline with high-amplitude, low-variance signal => high SNR.
        for i in 0..BASELINE_FRAMES {
            ps.process_frame(&[(i as f32) * 0.01; 16], &[10.0f32; 16]);
        }
        let mut found = false;
        // Now send very low, near-zero amplitudes (simulating jamming/noise floor).
        // All subcarriers identical => variance ~ 0, so SNR = mean/sqrt(var) ~ 0
        // which is well below 10% of the high baseline SNR.
        for i in 0..20u32 {
            let ev = ps.process_frame(&[5.0 + (i as f32) * 0.1; 16], &[0.001f32; 16]);
            if ev.iter().any(|&(t,_)| t == EVENT_JAMMING_DETECTED) { found = true; }
        }
        assert!(found, "jamming not detected");
    }

    #[test]
    fn test_integrity_score() {
        let mut ps = PromptShield::new();
        for i in 0..BASELINE_FRAMES {
            ps.process_frame(&[(i as f32) * 0.01; 16], &[1.0; 16]);
        }
        let mut found = false;
        for i in 0..20u32 {
            let ev = ps.process_frame(&[5.0 + (i as f32) * 0.05; 16], &[1.0; 16]);
            for &(et, v) in ev {
                if et == EVENT_SIGNAL_INTEGRITY { found = true; assert!(v >= 0.0 && v <= 1.0); }
            }
        }
        assert!(found, "integrity not emitted");
    }
}
