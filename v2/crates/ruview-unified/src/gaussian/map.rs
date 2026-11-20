//! The Gaussian map: spatial-hash-indexed storage with fusion, decay, and
//! spatial/semantic queries (ADR-275 §3).

use std::collections::HashMap;

use super::primitive::{RfGaussian, SEMANTIC_DIM};

/// Confidence floor below which a decayed Gaussian is pruned.
pub const PRUNE_CONFIDENCE: f64 = 0.02;

/// Squared-Mahalanobis merge gate: an incoming Gaussian whose centre lies
/// within this metric distance of an existing one *of the same entity kind*
/// fuses instead of inserting (3² = within 3σ).
pub const MERGE_MAHALANOBIS_SQ: f64 = 9.0;

/// Persistent Gaussian scene memory with an O(1) spatial-hash index.
#[derive(Debug, Default)]
pub struct GaussianMap {
    gaussians: Vec<RfGaussian>,
    /// Cell → indices. Rebuilt on decay/prune, updated on insert.
    grid: HashMap<(i64, i64, i64), Vec<usize>>,
    cell_size: f64,
}

impl GaussianMap {
    /// New map. `cell_size` is the spatial-hash pitch in metres; it should
    /// be on the order of the largest expected Gaussian extent (≈ 1 m for
    /// rooms).
    #[must_use]
    pub fn new(cell_size: f64) -> Self {
        Self { gaussians: Vec::new(), grid: HashMap::new(), cell_size: cell_size.max(1e-3) }
    }

    /// Number of live Gaussians.
    #[must_use]
    pub fn len(&self) -> usize {
        self.gaussians.len()
    }

    /// Whether the map is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.gaussians.is_empty()
    }

    /// Read-only view of the store.
    #[must_use]
    pub fn gaussians(&self) -> &[RfGaussian] {
        &self.gaussians
    }

    /// Mutable access for the inverse-gain updater (crate-internal).
    pub(crate) fn gaussians_mut(&mut self) -> &mut Vec<RfGaussian> {
        &mut self.gaussians
    }

    fn cell_of(&self, p: [f64; 3]) -> (i64, i64, i64) {
        (
            (p[0] / self.cell_size).floor() as i64,
            (p[1] / self.cell_size).floor() as i64,
            (p[2] / self.cell_size).floor() as i64,
        )
    }

    /// Rebuilds the spatial index from scratch (after decay/prune or
    /// occupancy edits that may have moved nothing — cheap: O(n)).
    pub(crate) fn rebuild_grid(&mut self) {
        self.grid.clear();
        for (i, g) in self.gaussians.iter().enumerate() {
            let cell = self.cell_of(g.position);
            self.grid.entry(cell).or_default().push(i);
        }
    }

    /// Inserts a Gaussian, fusing with an existing same-kind neighbor when
    /// the merge gate fires (ADR-275 §3.2).
    ///
    /// Fusion is confidence-weighted: position, occupancy, semantics,
    /// reflectivity, and Doppler average with weights `(c_old, c_new)`;
    /// confidence combines as noisy-OR `c = c₁ + c₂ − c₁c₂` (two independent
    /// pieces of evidence); the newer timestamp and provenance win.
    /// Returns the index of the stored (new or fused) Gaussian.
    pub fn insert(&mut self, g: RfGaussian) -> usize {
        // Candidate neighbors from the 3×3×3 cell neighborhood.
        let cell = self.cell_of(g.position);
        let mut best: Option<usize> = None;
        let mut best_d = MERGE_MAHALANOBIS_SQ;
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let Some(idxs) = self.grid.get(&(cell.0 + dx, cell.1 + dy, cell.2 + dz))
                    else {
                        continue;
                    };
                    for &i in idxs {
                        let existing = &self.gaussians[i];
                        let same_kind = match (existing.links.first(), g.links.first()) {
                            (Some(a), Some(b)) => a.kind == b.kind,
                            (None, None) => true,
                            _ => false,
                        };
                        if !same_kind {
                            continue;
                        }
                        let d = existing.mahalanobis_sq(g.position);
                        if d < best_d {
                            best_d = d;
                            best = Some(i);
                        }
                    }
                }
            }
        }

        if let Some(i) = best {
            let old_cell = self.cell_of(self.gaussians[i].position);
            {
                let e = &mut self.gaussians[i];
                let (wa, wb) = (e.confidence, g.confidence);
                let wsum = (wa + wb).max(1e-12);
                for k in 0..3 {
                    e.position[k] = (wa * e.position[k] + wb * g.position[k]) / wsum;
                    e.scale[k] = (wa * e.scale[k] + wb * g.scale[k]) / wsum;
                }
                e.occupancy = (wa * e.occupancy + wb * g.occupancy) / wsum;
                for k in 0..SEMANTIC_DIM {
                    e.semantic[k] =
                        ((f64::from(e.semantic[k]) * wa + f64::from(g.semantic[k]) * wb) / wsum) as f32;
                }
                for b in 0..e.reflectivity.len() {
                    for a in 0..e.reflectivity[b].len() {
                        e.reflectivity[b][a] =
                            (wa * e.reflectivity[b][a] + wb * g.reflectivity[b][a]) / wsum;
                    }
                }
                e.doppler_mps = (wa * e.doppler_mps + wb * g.doppler_mps) / wsum;
                e.doppler_variance = (wa * e.doppler_variance + wb * g.doppler_variance) / wsum;
                e.confidence = (wa + wb - wa * wb).clamp(0.0, 1.0);
                e.first_seen_ns = e.first_seen_ns.min(g.first_seen_ns);
                e.decayed_to_ns = e.decayed_to_ns.max(g.decayed_to_ns);
                if g.timestamp_ns >= e.timestamp_ns {
                    e.timestamp_ns = g.timestamp_ns;
                    e.provenance = g.provenance;
                    e.motion = g.motion;
                }
                for r in g.source_receipts {
                    if !e.source_receipts.contains(&r)
                        && e.source_receipts.len() < super::primitive::MAX_SOURCE_RECEIPTS
                    {
                        e.source_receipts.push(r);
                    }
                }
                for link in g.links {
                    if !e.links.contains(&link) {
                        e.links.push(link);
                    }
                }
            }
            // Re-index if fusion moved the centre across a cell boundary.
            let new_cell = self.cell_of(self.gaussians[i].position);
            if new_cell != old_cell {
                if let Some(v) = self.grid.get_mut(&old_cell) {
                    v.retain(|&x| x != i);
                }
                self.grid.entry(new_cell).or_default().push(i);
            }
            i
        } else {
            let idx = self.gaussians.len();
            let cell = self.cell_of(g.position);
            self.gaussians.push(g);
            self.grid.entry(cell).or_default().push(idx);
            idx
        }
    }

    /// Applies exponential confidence decay up to `now_ns` and prunes below
    /// [`PRUNE_CONFIDENCE`]. Deterministic: same inputs, same result.
    ///
    /// **Static persistence** (ADR-275 update-loop step 7): the effective
    /// decay constant is stretched by how long the Gaussian has been
    /// repeatedly observed — `τ_eff = τ · (1 + ln(1 + lifetime/τ))` with
    /// `lifetime = last_seen − first_seen`. A wall confirmed for hours
    /// outlives a transient echo seen once, even at equal nominal τ.
    ///
    /// Only the interval since `max(decayed_to_ns, timestamp_ns)` is applied,
    /// so `decay(t1); decay(t2)` equals `decay(t2)`: the result does not
    /// depend on how often the caller ticks.
    pub fn decay(&mut self, now_ns: u64) {
        for g in &mut self.gaussians {
            let from_ns = g.decayed_to_ns.max(g.timestamp_ns);
            let dt_s = (now_ns.saturating_sub(from_ns)) as f64 / 1e9;
            g.decayed_to_ns = from_ns.max(now_ns);
            let lifetime_s = (g.timestamp_ns.saturating_sub(g.first_seen_ns)) as f64 / 1e9;
            // `RfGaussian::new` validates `decay_tau_s > 0`, but the field is
            // mutable after construction (`gaussians_mut`); re-clamp here so a
            // stray zero/negative value can't turn this division into NaN
            // instead of a merely-fast decay.
            let tau = g.decay_tau_s.max(1e-6);
            let tau_eff = tau * (1.0 + (1.0 + lifetime_s / tau).ln());
            g.confidence *= (-dt_s / tau_eff).exp();
        }
        self.gaussians.retain(|g| g.confidence >= PRUNE_CONFIDENCE);
        self.rebuild_grid();
    }

    /// Merge pass (ADR-275 update-loop step 5): collapses pairs whose
    /// centres lie inside each other's merge gate *mutually* and whose
    /// semantics are compatible (cosine ≥ 0.7, or both unlabeled). The
    /// survivor absorbs the partner with the same confidence-weighted rule
    /// as [`Self::insert`] fusion. Returns the number of merges performed.
    pub fn merge_overlapping(&mut self) -> usize {
        let mut merged = 0usize;
        let mut removed = vec![false; self.gaussians.len()];
        for i in 0..self.gaussians.len() {
            if removed[i] {
                continue;
            }
            for j in (i + 1)..self.gaussians.len() {
                if removed[j] {
                    continue;
                }
                let (a, b) = (&self.gaussians[i], &self.gaussians[j]);
                // Same entity-kind gate as `insert` (§3.2): never conflate
                // Gaussians linked to different entity kinds (e.g. a Room
                // structure and a PersonClass detection sitting within each
                // other's merge gate near a doorway).
                let same_kind = match (a.links.first(), b.links.first()) {
                    (Some(la), Some(lb)) => la.kind == lb.kind,
                    (None, None) => true,
                    _ => false,
                };
                if !same_kind {
                    continue;
                }
                let mutual = a.mahalanobis_sq(b.position) < MERGE_MAHALANOBIS_SQ
                    && b.mahalanobis_sq(a.position) < MERGE_MAHALANOBIS_SQ;
                if !mutual {
                    continue;
                }
                let (na, nb): (f64, f64) = (
                    a.semantic.iter().map(|v| f64::from(*v).powi(2)).sum(),
                    b.semantic.iter().map(|v| f64::from(*v).powi(2)).sum(),
                );
                let compatible = if na < 1e-12 && nb < 1e-12 {
                    true // both unlabeled: pure geometry merge
                } else if na < 1e-12 || nb < 1e-12 {
                    false // one labeled, one not: keep separate
                } else {
                    let dot: f64 = a
                        .semantic
                        .iter()
                        .zip(&b.semantic)
                        .map(|(x, y)| f64::from(*x) * f64::from(*y))
                        .sum();
                    dot / (na.sqrt() * nb.sqrt()) >= 0.7
                };
                if !compatible {
                    continue;
                }
                // Fuse j into i (same math as insert-fusion).
                let partner = self.gaussians[j].clone();
                let e = &mut self.gaussians[i];
                let (wa, wb) = (e.confidence, partner.confidence);
                let wsum = (wa + wb).max(1e-12);
                for k in 0..3 {
                    e.position[k] = (wa * e.position[k] + wb * partner.position[k]) / wsum;
                    e.scale[k] = (wa * e.scale[k] + wb * partner.scale[k]) / wsum;
                }
                e.occupancy = (wa * e.occupancy + wb * partner.occupancy) / wsum;
                for k in 0..SEMANTIC_DIM {
                    e.semantic[k] = ((f64::from(e.semantic[k]) * wa
                        + f64::from(partner.semantic[k]) * wb)
                        / wsum) as f32;
                }
                e.confidence = (wa + wb - wa * wb).clamp(0.0, 1.0);
                e.first_seen_ns = e.first_seen_ns.min(partner.first_seen_ns);
                e.timestamp_ns = e.timestamp_ns.max(partner.timestamp_ns);
                e.decayed_to_ns = e.decayed_to_ns.max(partner.decayed_to_ns);
                for r in partner.source_receipts {
                    if !e.source_receipts.contains(&r)
                        && e.source_receipts.len() < super::primitive::MAX_SOURCE_RECEIPTS
                    {
                        e.source_receipts.push(r);
                    }
                }
                for link in partner.links {
                    if !e.links.contains(&link) {
                        e.links.push(link);
                    }
                }
                removed[j] = true;
                merged += 1;
            }
        }
        if merged > 0 {
            let mut keep = removed.iter().map(|r| !r);
            self.gaussians.retain(|_| keep.next().unwrap_or(true));
            self.rebuild_grid();
        }
        merged
    }

    /// Indices of Gaussians whose centres lie within `radius` of `p`, via
    /// the spatial hash (only the covered cell neighborhood is scanned).
    #[must_use]
    pub fn query_radius(&self, p: [f64; 3], radius: f64) -> Vec<usize> {
        let r_cells = (radius / self.cell_size).ceil() as i64;
        let c = self.cell_of(p);
        let mut out = Vec::new();
        let r2 = radius * radius;
        for dx in -r_cells..=r_cells {
            for dy in -r_cells..=r_cells {
                for dz in -r_cells..=r_cells {
                    let Some(idxs) = self.grid.get(&(c.0 + dx, c.1 + dy, c.2 + dz)) else {
                        continue;
                    };
                    for &i in idxs {
                        let q = self.gaussians[i].position;
                        let d2 = (q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2) + (q[2] - p[2]).powi(2);
                        if d2 <= r2 {
                            out.push(i);
                        }
                    }
                }
            }
        }
        out.sort_unstable();
        out
    }

    /// Reference implementation of [`Self::query_radius`] by linear scan —
    /// kept for the equivalence test and the benchmark baseline.
    #[must_use]
    pub fn query_radius_linear(&self, p: [f64; 3], radius: f64) -> Vec<usize> {
        let r2 = radius * radius;
        let mut out: Vec<usize> = self
            .gaussians
            .iter()
            .enumerate()
            .filter(|(_, g)| {
                let q = g.position;
                (q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2) + (q[2] - p[2]).powi(2) <= r2
            })
            .map(|(i, _)| i)
            .collect();
        out.sort_unstable();
        out
    }

    /// Indices of Gaussians whose centres lie within `margin` of the
    /// segment `a → b`, by walking only the hash cells along the corridor —
    /// the hot path of [`super::gain::optical_depth`].
    ///
    /// Sweeps the segment's margin-inflated AABB once; each cell is
    /// prefiltered by its *centre's* distance to the segment (bound:
    /// `margin + √3/2·cell`, which covers any point inside the cell) before
    /// the hash lookup, so no per-sample set building and no duplicate
    /// visits. Candidates are then exact-filtered by centre-to-segment
    /// distance. See `benches/unified_bench.rs` for the measured effect on
    /// `channel_gain` versus both the midpoint-ball query this replaced and
    /// the linear-scan baseline.
    #[must_use]
    pub fn query_near_segment(&self, a: [f64; 3], b: [f64; 3], margin: f64) -> Vec<usize> {
        let lo = |axis: usize| a[axis].min(b[axis]) - margin;
        let hi = |axis: usize| a[axis].max(b[axis]) + margin;
        let c_lo: Vec<i64> = (0..3).map(|k| (lo(k) / self.cell_size).floor() as i64).collect();
        let c_hi: Vec<i64> = (0..3).map(|k| (hi(k) / self.cell_size).floor() as i64).collect();
        let cell_bound = margin + 0.87 * self.cell_size; // √3/2 ≈ 0.866
        let mut out: Vec<usize> = Vec::new();
        for cx in c_lo[0]..=c_hi[0] {
            for cy in c_lo[1]..=c_hi[1] {
                for cz in c_lo[2]..=c_hi[2] {
                    let centre = [
                        (cx as f64 + 0.5) * self.cell_size,
                        (cy as f64 + 0.5) * self.cell_size,
                        (cz as f64 + 0.5) * self.cell_size,
                    ];
                    if Self::dist_point_segment(centre, a, b) > cell_bound {
                        continue;
                    }
                    let Some(idxs) = self.grid.get(&(cx, cy, cz)) else { continue };
                    for &i in idxs {
                        if Self::dist_point_segment(self.gaussians[i].position, a, b) <= margin {
                            out.push(i);
                        }
                    }
                }
            }
        }
        out.sort_unstable();
        out
    }

    /// Reference implementation of [`Self::query_near_segment`] by linear
    /// scan — equivalence-tested and benchmarked as the baseline.
    #[must_use]
    pub fn query_near_segment_linear(&self, a: [f64; 3], b: [f64; 3], margin: f64) -> Vec<usize> {
        (0..self.gaussians.len())
            .filter(|&i| Self::dist_point_segment(self.gaussians[i].position, a, b) <= margin)
            .collect()
    }

    /// Distance from a point to a segment.
    fn dist_point_segment(p: [f64; 3], a: [f64; 3], b: [f64; 3]) -> f64 {
        let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let ap = [p[0] - a[0], p[1] - a[1], p[2] - a[2]];
        let denom = ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2];
        let t = if denom < 1e-18 {
            0.0
        } else {
            ((ap[0] * ab[0] + ap[1] * ab[1] + ap[2] * ab[2]) / denom).clamp(0.0, 1.0)
        };
        let q = [a[0] + t * ab[0], a[1] + t * ab[1], a[2] + t * ab[2]];
        ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt()
    }

    /// `k` nearest Gaussians to `p` by centre distance (expanding-ring
    /// search over the hash grid).
    #[must_use]
    pub fn query_nearest(&self, p: [f64; 3], k: usize) -> Vec<usize> {
        if self.gaussians.is_empty() || k == 0 {
            return Vec::new();
        }
        let mut radius = self.cell_size;
        loop {
            let hits = self.query_radius(p, radius);
            if hits.len() >= k || radius > 1e4 {
                let mut scored: Vec<(f64, usize)> = hits
                    .into_iter()
                    .map(|i| {
                        let q = self.gaussians[i].position;
                        let d2 = (q[0] - p[0]).powi(2)
                            + (q[1] - p[1]).powi(2)
                            + (q[2] - p[2]).powi(2);
                        (d2, i)
                    })
                    .collect();
                scored.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
                scored.truncate(k);
                return scored.into_iter().map(|(_, i)| i).collect();
            }
            radius *= 2.0;
        }
    }

    /// `k` most semantically similar Gaussians by cosine similarity.
    #[must_use]
    pub fn query_semantic(&self, embedding: &[f32; SEMANTIC_DIM], k: usize) -> Vec<usize> {
        let norm_q: f64 = embedding.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>().sqrt();
        let mut scored: Vec<(f64, usize)> = self
            .gaussians
            .iter()
            .enumerate()
            .map(|(i, g)| {
                let dot: f64 = g
                    .semantic
                    .iter()
                    .zip(embedding)
                    .map(|(a, b)| f64::from(*a) * f64::from(*b))
                    .sum();
                let norm_g: f64 =
                    g.semantic.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>().sqrt();
                let sim = if norm_g * norm_q > 0.0 { dot / (norm_g * norm_q) } else { -1.0 };
                (sim, i)
            })
            .collect();
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(k);
        scored.into_iter().map(|(_, i)| i).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gaussian::primitive::Provenance;

    fn prov() -> Provenance {
        Provenance { device_id: "map-test".into(), model_version: 1, synthetic: true }
    }

    fn g_at(p: [f64; 3], confidence: f64, ts: u64) -> RfGaussian {
        RfGaussian::new(p, [0.3, 0.3, 0.3], [1.0, 0.0, 0.0, 0.0], 0.4, confidence, ts, 60.0, prov())
            .expect("valid gaussian")
    }

    #[test]
    fn nearby_same_kind_observations_fuse() {
        let mut map = GaussianMap::new(1.0);
        let a = map.insert(g_at([1.0, 1.0, 1.0], 0.5, 10));
        let b = map.insert(g_at([1.1, 1.0, 1.0], 0.5, 20));
        assert_eq!(a, b, "second observation must fuse, not duplicate");
        assert_eq!(map.len(), 1);
        let g = &map.gaussians()[0];
        // Confidence-weighted midpoint and noisy-OR confidence.
        assert!((g.position[0] - 1.05).abs() < 1e-9);
        assert!((g.confidence - 0.75).abs() < 1e-9);
        assert_eq!(g.timestamp_ns, 20);

        // A far observation creates a new Gaussian.
        map.insert(g_at([5.0, 5.0, 1.0], 0.5, 30));
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn decay_prunes_stale_gaussians_deterministically() {
        let mut map = GaussianMap::new(1.0);
        map.insert(g_at([0.0; 3], 0.9, 0)); // tau = 60 s
        map.insert(g_at([4.0, 0.0, 0.0], 0.9, 240_000_000_000)); // fresh
        // 240 s later: first has decayed by e^{-4} ⇒ 0.9·0.0183 ≈ 0.016 < floor.
        map.decay(240_000_000_000);
        assert_eq!(map.len(), 1);
        assert!((map.gaussians()[0].position[0] - 4.0).abs() < 1e-12);

        // Determinism: replay the same sequence, get the same state.
        let mut replay = GaussianMap::new(1.0);
        replay.insert(g_at([0.0; 3], 0.9, 0));
        replay.insert(g_at([4.0, 0.0, 0.0], 0.9, 240_000_000_000));
        replay.decay(240_000_000_000);
        assert_eq!(replay.len(), map.len());
        assert_eq!(replay.gaussians()[0].confidence, map.gaussians()[0].confidence);
    }

    #[test]
    fn long_lived_structure_outlives_transients_at_equal_tau() {
        let mut map = GaussianMap::new(1.0);
        // A wall confirmed over 30 minutes: first_seen 0, last update at
        // t = 1800 s. A transient echo seen once at t = 1800 s. Same τ = 60 s.
        let mut wall = g_at([0.0, 0.0, 1.0], 0.9, 1_800_000_000_000);
        wall.first_seen_ns = 0;
        let transient = g_at([6.0, 0.0, 1.0], 0.9, 1_800_000_000_000);
        map.insert(wall);
        map.insert(transient);

        // 5 minutes after the last observation (τ = 60 s ⇒ transient decays
        // by e^{-5} ≈ 0.0067 < prune floor; the wall's stretched τ_eff keeps it).
        map.decay(2_100_000_000_000);
        assert_eq!(map.len(), 1, "only the long-lived structure survives");
        assert!((map.gaussians()[0].position[0]).abs() < 1e-9, "survivor is the wall");
    }

    #[test]
    fn decay_is_independent_of_call_frequency() {
        // A transient (lifetime 0 ⇒ τ_eff = τ) and a wall seen for 30 min
        // (stretched τ_eff), both last observed at t = 1800 s.
        let build = || {
            let mut map = GaussianMap::new(1.0);
            let mut wall = g_at([0.0, 0.0, 1.0], 0.9, 1_800_000_000_000);
            wall.first_seen_ns = 0;
            map.insert(wall);
            map.insert(g_at([6.0, 0.0, 1.0], 0.9, 1_800_000_000_000));
            map
        };
        let end_ns = 1_890_000_000_000; // 90 s after the last observation

        let mut once = build();
        once.decay(end_ns);

        let mut stepped = build();
        for k in 1..=90u64 {
            stepped.decay(1_800_000_000_000 + k * 1_000_000_000);
        }
        // Repeating the final timestamp must be a no-op.
        stepped.decay(end_ns);

        assert_eq!(once.len(), 2, "both survive 90 s of single-call decay");
        assert_eq!(stepped.len(), once.len(), "no extra pruning");
        for (a, b) in once.gaussians().iter().zip(stepped.gaussians()) {
            assert!(
                (a.confidence - b.confidence).abs() < 1e-12,
                "decay(t) once gave {} but stepped decay up to t gave {}",
                a.confidence,
                b.confidence
            );
        }
        // Transient: exactly 0.9·e^{-90/60}.
        let transient = &once.gaussians()[1];
        assert!((transient.confidence - 0.9 * (-1.5f64).exp()).abs() < 1e-12);
    }

    #[test]
    fn static_persistence_holds_under_frequent_decay() {
        // Same scenario as `long_lived_structure_outlives_transients_at_equal_tau`,
        // but decay runs every 10 s instead of once at the end.
        let mut map = GaussianMap::new(1.0);
        let mut wall = g_at([0.0, 0.0, 1.0], 0.9, 1_800_000_000_000);
        wall.first_seen_ns = 0;
        map.insert(wall);
        map.insert(g_at([6.0, 0.0, 1.0], 0.9, 1_800_000_000_000));
        for k in 1..=30u64 {
            map.decay(1_800_000_000_000 + k * 10_000_000_000);
        }
        assert_eq!(map.len(), 1, "only the long-lived structure survives");
        assert!(map.gaussians()[0].position[0].abs() < 1e-9, "wall survives");
    }

    #[test]
    fn merge_pass_collapses_mutual_overlaps_but_respects_semantics() {
        // Two overlapping unlabeled Gaussians that insert-fusion misses
        // because its gate only scans the ±1-cell neighborhood: with a
        // 0.05 m cell pitch, 0.40 m spacing is far outside the cell window
        // yet well inside the 3σ Mahalanobis merge gate (σ = 0.3).
        let mut map2 = GaussianMap::new(0.05);
        map2.insert(g_at([1.00, 0.0, 1.0], 0.5, 10));
        map2.insert(g_at([1.40, 0.0, 1.0], 0.5, 20));
        assert_eq!(map2.len(), 2, "insert kept them separate (cell-local gate)");
        let merged = map2.merge_overlapping();
        assert_eq!(merged, 1, "merge pass collapses the mutual overlap");
        assert_eq!(map2.len(), 1);
        let g = &map2.gaussians()[0];
        assert!((g.position[0] - 1.20).abs() < 1e-9, "confidence-weighted midpoint");
        assert!((g.confidence - 0.75).abs() < 1e-9, "noisy-OR confidence");

        // Semantically incompatible pair does NOT merge.
        let mut c = g_at([5.0, 0.0, 1.0], 0.5, 30);
        c.semantic[0] = 1.0;
        let mut d = g_at([5.2, 0.0, 1.0], 0.5, 40);
        d.semantic[1] = 1.0; // orthogonal embedding
        map2.insert(c);
        map2.insert(d);
        let before = map2.len();
        assert_eq!(map2.merge_overlapping(), 0, "orthogonal semantics must not merge");
        assert_eq!(map2.len(), before);
    }

    #[test]
    fn spatial_hash_query_matches_linear_scan() {
        let mut map = GaussianMap::new(0.7);
        // Grid of well-separated Gaussians (spacing 2 m ≫ merge gate at σ=0.3).
        for x in 0..10 {
            for y in 0..10 {
                map.insert(g_at([x as f64 * 2.0, y as f64 * 2.0, 1.0], 0.9, 0));
            }
        }
        assert_eq!(map.len(), 100);
        for (centre, radius) in
            [([5.0, 5.0, 1.0], 3.0), ([0.0, 0.0, 1.0], 1.5), ([18.0, 18.0, 1.0], 5.0)]
        {
            assert_eq!(
                map.query_radius(centre, radius),
                map.query_radius_linear(centre, radius),
                "hash and linear scans must agree at {centre:?} r={radius}"
            );
        }
    }

    #[test]
    fn segment_corridor_query_matches_linear_scan() {
        let mut map = GaussianMap::new(1.0);
        for x in 0..12 {
            for y in 0..12 {
                for z in 0..2 {
                    map.insert(g_at(
                        [x as f64 * 1.7, y as f64 * 1.7, 0.8 + z as f64 * 1.4],
                        0.9,
                        0,
                    ));
                }
            }
        }
        for (a, b, m) in [
            ([0.0, 0.0, 1.0], [18.0, 18.0, 1.5], 3.0),
            ([2.0, 15.0, 1.0], [15.0, 2.0, 2.0], 1.5),
            ([5.0, 5.0, 1.0], [5.0, 5.0, 1.0], 2.0), // degenerate segment
        ] {
            assert_eq!(
                map.query_near_segment(a, b, m),
                map.query_near_segment_linear(a, b, m),
                "corridor hash walk and linear scan must agree for {a:?}→{b:?} m={m}"
            );
        }
    }

    #[test]
    fn nearest_and_semantic_queries() {
        let mut map = GaussianMap::new(1.0);
        map.insert(g_at([0.0, 0.0, 0.0], 0.9, 0));
        map.insert(g_at([3.0, 0.0, 0.0], 0.9, 0));
        let mut tagged = g_at([9.0, 9.0, 0.0], 0.9, 0);
        tagged.semantic[0] = 1.0;
        tagged.semantic[1] = 0.5;
        map.insert(tagged);

        let near = map.query_nearest([0.2, 0.0, 0.0], 2);
        assert_eq!(near.len(), 2);
        assert_eq!(near[0], 0, "closest first");

        let mut q = [0.0f32; SEMANTIC_DIM];
        q[0] = 1.0;
        q[1] = 0.5;
        let sem = map.query_semantic(&q, 1);
        assert_eq!(sem, vec![2], "semantic query finds the tagged Gaussian");
    }
}
