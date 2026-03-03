//! Sliced Wasserstein distance for geometric motion detection (ADR-041).
//!
//! Computes 1D Wasserstein distance between current/previous CSI amplitude
//! distributions via 4 fixed random projections.  Detects "subtle motion"
//! when Wasserstein is elevated but total variance is stable.
//! Events: WASSERSTEIN_DISTANCE(725), DISTRIBUTION_SHIFT(726), SUBTLE_MOTION(727).

use libm::fabsf;

const MAX_SC: usize = 32;
const N_PROJ: usize = 4;
const ALPHA: f32 = 0.15;
const VAR_ALPHA: f32 = 0.1;
const WASS_SHIFT: f32 = 0.25;
const WASS_SUBTLE: f32 = 0.10;
const VAR_STABLE: f32 = 0.15;
const SHIFT_DEB: u8 = 3;
const SUBTLE_DEB: u8 = 5;

pub const EVENT_WASSERSTEIN_DISTANCE: i32 = 725;
pub const EVENT_DISTRIBUTION_SHIFT: i32 = 726;
pub const EVENT_SUBTLE_MOTION: i32 = 727;

/// Deterministic projection directions via LCG PRNG, L2-normalized.
const PROJ: [[f32; MAX_SC]; N_PROJ] = gen_proj();

const fn gen_proj() -> [[f32; MAX_SC]; N_PROJ] {
    let seeds = [42u32, 137, 2718, 31415];
    let mut dirs = [[0.0f32; MAX_SC]; N_PROJ];
    let mut p = 0;
    while p < N_PROJ {
        let mut st = seeds[p];
        let mut raw = [0.0f32; MAX_SC];
        let mut i = 0;
        while i < MAX_SC {
            st = st.wrapping_mul(1103515245).wrapping_add(12345) & 0x7FFF_FFFF;
            raw[i] = (st as f32 / 1_073_741_823.0) * 2.0 - 1.0;
            i += 1;
        }
        let mut sq = 0.0f32;
        i = 0; while i < MAX_SC { sq += raw[i] * raw[i]; i += 1; }
        // Newton-Raphson sqrt (6 iters).
        let mut norm = sq * 0.5;
        if norm < 1e-9 { norm = 1.0; }
        let mut k = 0; while k < 6 { norm = 0.5 * (norm + sq / norm); k += 1; }
        i = 0; while i < MAX_SC { dirs[p][i] = raw[i] / norm; i += 1; }
        p += 1;
    }
    dirs
}

/// Shell sort with Ciura gap sequence -- O(n^1.3) vs insertion sort's O(n^2).
/// For n=32 this reduces worst-case from ~1024 to ~128 comparisons per sort.
/// 8 sorts per frame (2 per projection * 4 projections) = significant savings.
fn shell_sort(a: &mut [f32], n: usize) {
    // Ciura gap sequence (truncated for n<=32).
    const GAPS: [usize; 4] = [10, 4, 1, 0];
    let mut gi = 0;
    while gi < 3 {
        let gap = GAPS[gi];
        if gap >= n { gi += 1; continue; }
        let mut i = gap;
        while i < n {
            let k = a[i];
            let mut j = i;
            while j >= gap && a[j - gap] > k {
                a[j] = a[j - gap];
                j -= gap;
            }
            a[j] = k;
            i += 1;
        }
        gi += 1;
    }
}

/// Sliced Wasserstein motion detector.
pub struct OptimalTransportDetector {
    prev_amps: [f32; MAX_SC],
    smoothed_dist: f32,
    smoothed_var: f32,
    prev_var: f32,
    initialized: bool,
    frame_count: u32,
    shift_streak: u8,
    subtle_streak: u8,
}

impl OptimalTransportDetector {
    pub const fn new() -> Self {
        Self { prev_amps: [0.0; MAX_SC], smoothed_dist: 0.0, smoothed_var: 0.0, prev_var: 0.0,
               initialized: false, frame_count: 0, shift_streak: 0, subtle_streak: 0 }
    }

    fn w1_sorted(a: &[f32], b: &[f32], n: usize) -> f32 {
        if n == 0 { return 0.0; }
        let mut s = 0.0f32;
        let mut i = 0; while i < n { s += fabsf(a[i] - b[i]); i += 1; }
        s / n as f32
    }

    fn sliced_w(cur: &[f32], prev: &[f32], n: usize) -> f32 {
        let mut total = 0.0f32;
        let mut p = 0;
        while p < N_PROJ {
            let mut pc = [0.0f32; MAX_SC];
            let mut pp = [0.0f32; MAX_SC];
            let mut i = 0;
            while i < n { pc[i] = cur[i] * PROJ[p][i]; pp[i] = prev[i] * PROJ[p][i]; i += 1; }
            shell_sort(&mut pc, n);
            shell_sort(&mut pp, n);
            total += Self::w1_sorted(&pc, &pp, n);
            p += 1;
        }
        total / N_PROJ as f32
    }

    fn variance(a: &[f32], n: usize) -> f32 {
        if n == 0 { return 0.0; }
        let mut m = 0.0f32;
        let mut i = 0; while i < n { m += a[i]; i += 1; } m /= n as f32;
        let mut v = 0.0f32;
        i = 0; while i < n { let d = a[i] - m; v += d * d; i += 1; }
        v / n as f32
    }

    /// Process one frame of amplitude data. Returns events.
    pub fn process_frame(&mut self, amplitudes: &[f32]) -> &[(i32, f32)] {
        let n = amplitudes.len().min(MAX_SC);
        if n < 2 { return &[]; }
        self.frame_count += 1;
        let mut cur = [0.0f32; MAX_SC];
        let mut i = 0; while i < n { cur[i] = amplitudes[i]; i += 1; }

        if !self.initialized {
            i = 0; while i < n { self.prev_amps[i] = cur[i]; i += 1; }
            self.smoothed_var = Self::variance(&cur, n);
            self.prev_var = self.smoothed_var;
            self.initialized = true;
            return &[];
        }

        let raw_w = Self::sliced_w(&cur, &self.prev_amps, n);
        self.smoothed_dist = ALPHA * raw_w + (1.0 - ALPHA) * self.smoothed_dist;

        let cv = Self::variance(&cur, n);
        self.prev_var = self.smoothed_var;
        self.smoothed_var = VAR_ALPHA * cv + (1.0 - VAR_ALPHA) * self.smoothed_var;
        let vc = if self.prev_var > 1e-6 { fabsf(self.smoothed_var - self.prev_var) / self.prev_var } else { 0.0 };

        i = 0; while i < n { self.prev_amps[i] = cur[i]; i += 1; }

        static mut EV: [(i32, f32); 4] = [(0, 0.0); 4];
        let mut ne = 0usize;

        if self.frame_count % 5 == 0 && ne < 4 {
            unsafe { EV[ne] = (EVENT_WASSERSTEIN_DISTANCE, self.smoothed_dist); } ne += 1;
        }
        if self.smoothed_dist > WASS_SHIFT {
            self.shift_streak = self.shift_streak.saturating_add(1);
            if self.shift_streak >= SHIFT_DEB && ne < 4 {
                unsafe { EV[ne] = (EVENT_DISTRIBUTION_SHIFT, self.smoothed_dist); } ne += 1;
                self.shift_streak = 0;
            }
        } else { self.shift_streak = 0; }

        if self.smoothed_dist > WASS_SUBTLE && vc < VAR_STABLE {
            self.subtle_streak = self.subtle_streak.saturating_add(1);
            if self.subtle_streak >= SUBTLE_DEB && ne < 4 {
                unsafe { EV[ne] = (EVENT_SUBTLE_MOTION, self.smoothed_dist); } ne += 1;
                self.subtle_streak = 0;
            }
        } else { self.subtle_streak = 0; }

        unsafe { &EV[..ne] }
    }

    pub fn distance(&self) -> f32 { self.smoothed_dist }
    pub fn variance_smoothed(&self) -> f32 { self.smoothed_var }
    pub fn frame_count(&self) -> u32 { self.frame_count }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init() { let d = OptimalTransportDetector::new(); assert_eq!(d.frame_count(), 0); }

    #[test]
    fn test_identical_zero() {
        let mut d = OptimalTransportDetector::new();
        let a = [1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        d.process_frame(&a); d.process_frame(&a);
        assert!(d.distance() < 0.01, "identical => ~0, got {}", d.distance());
    }

    #[test]
    fn test_different_nonzero() {
        let mut d = OptimalTransportDetector::new();
        d.process_frame(&[1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        d.process_frame(&[8.0f32, 7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0]);
        assert!(d.distance() > 0.0);
    }

    #[test]
    fn test_shift_event() {
        let mut d = OptimalTransportDetector::new();
        d.process_frame(&[1.0f32; 16]);
        let mut found = false;
        // Alternate between two very different distributions so every frame
        // produces a large Wasserstein distance, allowing the EMA to exceed
        // WASS_SHIFT and the debounce counter to reach SHIFT_DEB.
        for i in 0..40 {
            let amps = if i % 2 == 0 { [20.0f32; 16] } else { [1.0f32; 16] };
            for &(t, _) in d.process_frame(&amps) {
                if t == EVENT_DISTRIBUTION_SHIFT { found = true; }
            }
        }
        assert!(found, "large shift should trigger event");
    }

    #[test]
    fn test_sort() {
        let mut a = [5.0f32, 3.0, 8.0, 1.0, 4.0]; shell_sort(&mut a, 5);
        assert_eq!([a[0], a[1], a[2], a[3], a[4]], [1.0, 3.0, 4.0, 5.0, 8.0]);
    }

    #[test]
    fn test_w1() {
        let a = [1.0f32, 2.0, 3.0, 4.0]; let b = [2.0f32, 3.0, 4.0, 5.0];
        assert!(fabsf(OptimalTransportDetector::w1_sorted(&a, &b, 4) - 1.0) < 0.001);
    }

    #[test]
    fn test_proj_normalized() {
        for p in 0..N_PROJ {
            let mut sq = 0.0f32; for i in 0..MAX_SC { sq += PROJ[p][i] * PROJ[p][i]; }
            assert!(fabsf(libm::sqrtf(sq) - 1.0) < 0.05, "proj {p} norm err");
        }
    }

    #[test]
    fn test_variance_calc() {
        let v = OptimalTransportDetector::variance(&[2.0f32, 4.0, 6.0, 8.0], 4);
        assert!(fabsf(v - 5.0) < 0.01, "var={v}");
    }

    #[test]
    fn test_stable_no_events() {
        let mut d = OptimalTransportDetector::new();
        d.process_frame(&[3.0f32; 16]);
        for _ in 0..50 {
            for &(t, _) in d.process_frame(&[3.0f32; 16]) {
                assert!(t != EVENT_DISTRIBUTION_SHIFT && t != EVENT_SUBTLE_MOTION);
            }
        }
    }
}
