//! Link-line ("radio tomographic") position estimation from per-link motion.
//!
//! # Why this exists
//!
//! The shipping estimators — [`motion_weighted_centroid`] and its Doppler
//! sibling — compute `sum(w_i * pos_i) / sum(w_i)` over node positions with all
//! `w_i >= 0`. That is a convex combination, so the estimate is *mathematically
//! confined* to the convex hull of the nodes. On the 2026-08-28 layout the node
//! triangle encloses 5.23 m² of a 138.98 m² room: the dot can only ever appear
//! in 3.8% of the space, at a fixed height, no matter where the person is. It is
//! not a weak estimator, it is a structurally incapable one.
//!
//! This module drops the "position is near the loud node" heuristic for a
//! forward model: a person perturbs a link when they are near the *line* between
//! its two endpoints. Both endpoints are known, so each link constrains position
//! to a region rather than voting for a point, and the estimate is free to land
//! anywhere the links actually cross.
//!
//! # Model
//!
//! For a cell `c` and a link from `tx` to `rx`, the excess path length
//! `|tx-c| + |c-rx| - |tx-rx|` is zero on the direct line and grows with
//! distance from it; its level sets are ellipses with the endpoints as foci.
//! The predicted weight is `exp(-excess / ELLIPSE_WIDTH_M)` — the usual
//! elliptical RTI kernel, smoothed rather than hard-thresholded so the score
//! surface has no cliffs for the peak search to catch on.
//!
//! Cells are scored by the **Pearson correlation** between the observed
//! per-link responses and the weights that cell predicts. Correlation, not a
//! plain weighted sum, for two reasons that both matter at six links:
//!
//! - A plain sum rewards any cell sitting near several links, whatever the
//!   observations say. Correlation asks whether the *pattern* of which links are
//!   unusually perturbed matches the pattern this cell would produce.
//! - Centering makes the score invariant to overall room activity, so the
//!   estimate does not swing with how vigorously the person is moving.
//!
//! # What this does not fix
//!
//! Escaping the convex hull is a real gain but it is not accuracy. Six
//! independent links (nine directed links, but reciprocal pairs measure the same
//! physical channel) sample a 139 m² room very sparsely, and the three
//! node-to-node links are the *edges* of the node triangle, so they do not cross
//! its interior. Sparse geometry produces genuinely ambiguous score surfaces,
//! which is why [`RtiEstimate::spread_m`] is computed and reported rather than
//! hidden: a multi-modal surface must be visible as one, not silently collapsed
//! to whichever mode happened to win.
//!
//! Nothing in here is calibrated against ground truth. Any accuracy figure for
//! this tier is `CLAIMED` until a labelled walk says otherwise.

/// One link's geometry and its normalised response this tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinkObservation {
    /// Receiver position, metres, room frame.
    pub rx: [f64; 2],
    /// Transmitter position, metres, room frame.
    pub tx: [f64; 2],
    /// Response, normalised so links of different intrinsic sensitivity are
    /// comparable. See [`normalise_response`] — passing raw motion here is a
    /// bug, not a shortcut: measured live, AP links run 0.9-4.0 while
    /// node-to-node links run 0.02-0.55, so raw values let three links decide
    /// everything and silence the three with the useful parallax.
    pub response: f64,
    /// How much this link's vote counts, in `(0, 1]`. See
    /// [`temporal_authority`].
    ///
    /// Exists because `response` alone hides *when* it was measured. Every
    /// link's motion figure is a statistic over that link's own window, and
    /// MEASURED 2026-09-01 those windows span 1.2 s to 4695 s (median 166 s).
    /// Pooling them unweighted tells the solver that what one link saw over
    /// the last 78 minutes and what another saw over the last two seconds are
    /// equally about *now*.
    ///
    /// Deliberately a weight and never a filter. Window length is really
    /// reception rate, so discarding slow links would delete the weak, long,
    /// cross-house and cross-floor links — exactly the ones carrying the
    /// parallax — and MEASURED on this fleet would strip every link from 11 of
    /// 20 illuminators and 6 of 9 of our own boards, leaving 30% of the solve
    /// on the access point and 22% on a 3D printer that is only powered on
    /// while something is being printed. A stale link keeps its geometry and
    /// simply stops outvoting a live one.
    pub authority: f64,
}

/// Search-grid and kernel parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RtiConfig {
    /// North-west corner of the search grid, in room coordinates.
    ///
    /// Not always the room origin. The origin is fixed at the north-west
    /// corner of the building's main block — it is what every node position
    /// was measured against — so a wing extending west or north of that block
    /// lives at negative coordinates. A grid that always started at (0, 0)
    /// could not search there at all, whatever the footprint said, and would
    /// silently confine every estimate to the main block.
    pub origin_m: [f64; 2],
    /// Grid extent from `origin_m`, not from the room origin.
    pub width_m: f64,
    pub depth_m: f64,
    /// Grid pitch. 0.25 m over a 13.4x10.4 m room is ~2200 cells; at six links
    /// and a 2 Hz tick the whole search is negligible next to the CSI work.
    pub cell_m: f64,
    /// Kernel decay constant for excess path length.
    pub ellipse_width_m: f64,
}

impl Default for RtiConfig {
    fn default() -> Self {
        Self {
            origin_m: [0.0, 0.0],
            width_m: 10.0,
            depth_m: 10.0,
            cell_m: 0.25,
            // A person is roughly half a metre wide and perturbs well beyond
            // the first Fresnel zone (~0.36 m at mid-span on a 4 m link at
            // 2.4 GHz). Tighter than this and a real target falls outside its
            // own link's kernel between grid cells.
            ellipse_width_m: 0.6,
        }
    }
}

/// A position estimate plus the honesty metrics needed to decide whether to
/// believe it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RtiEstimate {
    pub x: f64,
    pub y: f64,
    /// Peak Pearson correlation, `[-1, 1]`. How well the best cell explains the
    /// observed pattern — not a probability, and not calibrated.
    pub confidence: f64,
    /// RMS spread of the near-peak cells, metres. Small means the surface has
    /// one clear mode; large means the links admit several separated
    /// explanations and the reported point is one of them arbitrarily. This is
    /// the number that reveals sparse-geometry ambiguity, so callers should
    /// gate on it rather than on `confidence` alone.
    pub spread_m: f64,
    /// Links that contributed.
    pub links_used: usize,
}

/// A 2D position needs at least three links before the geometry constrains
/// anything; with two, the near-peak set is a curve, not a point.
pub const MIN_LINKS: usize = 3;

/// Cells whose total predicted weight is below this are not observed by any
/// link, so their correlation score is a comparison of two noise vectors and
/// can peak anywhere. Excluding them keeps estimates inside the region the
/// links actually illuminate instead of letting an empty corner of the house
/// win on an accident of sign.
pub const MIN_CELL_OBSERVABILITY: f64 = 0.05;

/// Cells scoring at least this fraction of the peak join the centroid that
/// refines the estimate below grid pitch, and define the spread.
const NEAR_PEAK_FRACTION: f64 = 0.9;

/// Predicted response of a link to a target at `cell`.
///
/// `exp(-excess / width)`, where excess is the extra path length through the
/// cell over the direct line. Exactly 1 on the line, decaying with distance
/// from it; the level sets are ellipses focused on the endpoints.
///
/// Deliberately carries no `1/sqrt(link_length)` term. The classic RTI
/// formulation includes one, but every observation here is already normalised
/// per link, so a constant per-link factor cancels out of the correlation.
pub fn link_weight(rx: [f64; 2], tx: [f64; 2], cell: [f64; 2], ellipse_width_m: f64) -> f64 {
    let direct = dist(rx, tx);
    if direct <= f64::EPSILON || ellipse_width_m <= 0.0 {
        return 0.0;
    }
    let through = dist(tx, cell) + dist(cell, rx);
    // Clamped at zero: floating-point error can make `through` a hair under
    // `direct` for a cell exactly on the line, which would otherwise return a
    // weight above 1.
    let excess = (through - direct).max(0.0);
    (-excess / ellipse_width_m).exp()
}

/// Scale a link's raw motion into a comparable response.
///
/// `raw / scale`, where `scale` is that link's own resting level. The result is
/// "how many times its own quiet state is this link running at", which is
/// comparable across links whose absolute sensitivities differ by two orders of
/// magnitude for reasons — path length, RSSI, beacon size versus data frame —
/// that have nothing to do with where anyone is standing.
///
/// The caller supplies `scale` from a continuously adapting baseline, which has
/// a known weakness: a person who stays still long enough is gradually absorbed
/// into it. A recorded empty-room calibration would replace it with a fixed
/// reference and remove that failure mode entirely. Until then, this tier is
/// blind to a motionless target — the same limitation the rest of the pipeline
/// already has.
pub fn normalise_response(raw_motion: f64, scale: f64) -> f64 {
    if !(scale > f64::EPSILON) || !raw_motion.is_finite() {
        return 0.0;
    }
    (raw_motion / scale).max(0.0)
}

/// Timescale, in seconds, that a position estimate is trying to describe.
/// Roughly how long a person spends perturbing one link while walking past it.
const AUTHORITY_TAU_S: f64 = 10.0;

/// How much a link's vote counts, given the wall-clock span its motion figure
/// was computed over.
///
/// `1 / (1 + span/tau)` — hyperbolic, and that choice is the point. An
/// exponential would put a 236 s link at `e^-23`, which is a filter wearing a
/// weight's clothing; the hyperbolic form leaves it at 0.04, so it still
/// contributes its geometry and still cannot outvote a live link. Nothing is
/// ever driven to zero, so no link is silently dropped.
///
/// A non-finite or non-positive span means the span is unknown rather than
/// long, and returns full authority — the behaviour every caller had before
/// this existed.
pub fn temporal_authority(window_span_s: f64) -> f64 {
    if !window_span_s.is_finite() || window_span_s <= 0.0 {
        return 1.0;
    }
    1.0 / (1.0 + window_span_s / AUTHORITY_TAU_S)
}

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    (dx * dx + dy * dy).sqrt()
}

/// Pearson correlation. `None` when either input has no variance — for a cell
/// that means every link predicts the same weight, so it explains no pattern
/// and must not score as a perfect match.
/// Pearson correlation with a per-sample weight.
///
/// `w` scales each link's influence on the means, the covariance and both
/// variances, so a low-authority link shifts the answer less without being
/// removed from it. With every weight equal this is the unweighted Pearson
/// coefficient exactly, which is what keeps the pre-authority behaviour
/// recoverable.
fn correlation(a: &[f64], b: &[f64], w: &[f64]) -> Option<f64> {
    let n = a.len();
    if n < 2 || b.len() != n || w.len() != n {
        return None;
    }
    let tw: f64 = w.iter().sum();
    if !(tw > f64::EPSILON) {
        return None;
    }
    let ma = a.iter().zip(w).map(|(x, wi)| x * wi).sum::<f64>() / tw;
    let mb = b.iter().zip(w).map(|(x, wi)| x * wi).sum::<f64>() / tw;
    let (mut num, mut va, mut vb) = (0.0, 0.0, 0.0);
    for i in 0..n {
        let da = a[i] - ma;
        let db = b[i] - mb;
        num += w[i] * da * db;
        va += w[i] * da * da;
        vb += w[i] * db * db;
    }
    if va <= f64::EPSILON || vb <= f64::EPSILON {
        return None;
    }
    Some(num / (va * vb).sqrt())
}

/// Search the room for the cell whose predicted link weights best explain the
/// observed responses.
///
/// Whether a point lies inside a closed ring, by the even-odd rule.
///
/// The ring is implicitly closed: the last vertex connects back to the first,
/// so callers pass vertices only. Points exactly on an edge are not guaranteed
/// either way, which is fine here — a grid cell centre landing precisely on a
/// wall line is both arbitrary and half a cell from being clearly inside.
///
/// Exists because a bounding rectangle is a poor description of a real house.
/// An L-shaped plan leaves a notch inside `width_m x depth_m` that is not part
/// of the building at all, and `estimate` will happily score those cells and
/// return a peak standing in the garden.
pub fn point_in_polygon(pt: [f64; 2], ring: &[[f64; 2]]) -> bool {
    if ring.len() < 3 {
        return false;
    }
    let (x, y) = (pt[0], pt[1]);
    let mut inside = false;
    let mut j = ring.len() - 1;
    for i in 0..ring.len() {
        let (xi, yi) = (ring[i][0], ring[i][1]);
        let (xj, yj) = (ring[j][0], ring[j][1]);
        // Half-open in y so a vertex is counted by exactly one of its edges.
        if (yi > y) != (yj > y) {
            let t = (y - yi) / (yj - yi);
            if x < xi + t * (xj - xi) {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

/// `None` when there are too few links, when no link shows any response, or
/// when no observed cell produces a positive correlation — all cases where a
/// returned point would be an invention rather than an estimate.
///
/// `rings` optionally restricts the search to the building's real footprint.
/// Each ring is a closed outline in room coordinates; a cell counts as
/// searchable if it falls inside ANY of them, so an L-shaped house, a wing, or
/// a detached structure are all expressible as a union. Empty means "search the
/// whole rectangle", which is the behaviour every caller had before footprints
/// existed.
pub fn estimate(
    observations: &[LinkObservation],
    cfg: &RtiConfig,
    rings: &[Vec<[f64; 2]>],
) -> Option<RtiEstimate> {
    if observations.len() < MIN_LINKS {
        return None;
    }
    if cfg.cell_m <= 0.0 || cfg.width_m <= 0.0 || cfg.depth_m <= 0.0 {
        return None;
    }

    let observed: Vec<f64> = observations.iter().map(|o| o.response).collect();
    if !observed.iter().all(|v| v.is_finite()) {
        return None;
    }
    // A flat observation vector carries no spatial information; correlating
    // against it would return whichever cell won on rounding.
    let mean = observed.iter().sum::<f64>() / observed.len() as f64;
    if observed.iter().all(|v| (v - mean).abs() <= f64::EPSILON) {
        return None;
    }

    let nx = (cfg.width_m / cfg.cell_m).ceil() as usize;
    let ny = (cfg.depth_m / cfg.cell_m).ceil() as usize;
    if nx == 0 || ny == 0 {
        return None;
    }

    let authority: Vec<f64> = observations
        .iter()
        .map(|o| if o.authority.is_finite() { o.authority.clamp(0.0, 1.0) } else { 1.0 })
        .collect();
    if !authority.iter().any(|w| *w > f64::EPSILON) {
        return None;
    }

    let mut weights = vec![0.0_f64; observations.len()];
    let mut best = f64::NEG_INFINITY;
    // Two passes over the grid: one to find the peak, one to collect the cells
    // near it. Cheaper than retaining every score, and the grid is small.
    let mut scored: Vec<([f64; 2], f64)> = Vec::new();

    for iy in 0..ny {
        for ix in 0..nx {
            let cell = [
                cfg.origin_m[0] + (ix as f64 + 0.5) * cfg.cell_m,
                cfg.origin_m[1] + (iy as f64 + 0.5) * cfg.cell_m,
            ];
            // Outside the building is not a place a person can be. Skipping
            // these cells is not cosmetic: they are scored against the same
            // link kernels as real ones and can win outright, which puts the
            // estimate in a part of the bounding box the house does not occupy.
            if !rings.is_empty() && !rings.iter().any(|r| point_in_polygon(cell, r)) {
                continue;
            }
            let mut mass = 0.0;
            for (i, o) in observations.iter().enumerate() {
                let w = link_weight(o.rx, o.tx, cell, cfg.ellipse_width_m);
                weights[i] = w;
                // Deliberately NOT scaled by authority. This gate asks whether
                // any link illuminates the cell at all -- a geometric question
                // -- and scaling it by staleness made the whole grid fail the
                // gate whenever the fleet was quiet, so the solver went blind
                // exactly when the house was still. Authority decides whose
                // vote counts, not whether a place exists.
                mass += w;
            }
            if mass < MIN_CELL_OBSERVABILITY {
                continue;
            }
            let Some(score) = correlation(&observed, &weights, &authority) else {
                continue;
            };
            if score > best {
                best = score;
            }
            scored.push((cell, score));
        }
    }

    // A negative peak means every observed cell predicts the *opposite* pattern
    // to the one measured. Reporting the least-bad of those would be a fiction.
    if scored.is_empty() || best <= 0.0 {
        return None;
    }

    let cutoff = best * NEAR_PEAK_FRACTION;
    let near: Vec<[f64; 2]> = scored
        .iter()
        .filter(|(_, s)| *s >= cutoff)
        .map(|(c, _)| *c)
        .collect();
    if near.is_empty() {
        return None;
    }

    let n = near.len() as f64;
    let cx = near.iter().map(|c| c[0]).sum::<f64>() / n;
    let cy = near.iter().map(|c| c[1]).sum::<f64>() / n;
    let spread = (near
        .iter()
        .map(|c| {
            let dx = c[0] - cx;
            let dy = c[1] - cy;
            dx * dx + dy * dy
        })
        .sum::<f64>()
        / n)
        .sqrt();

    Some(RtiEstimate {
        x: cx,
        y: cy,
        confidence: best,
        spread_m: spread,
        links_used: observations.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The 2026-08-28 layout, from `v2/data/room_config.json`.
    const NODE0: [f64; 2] = [1.2192, 0.0];
    const NODE1: [f64; 2] = [3.81, 0.0];
    const NODE2: [f64; 2] = [2.4384, 4.0386];
    const AP: [f64; 2] = [7.9248, 5.8674];

    fn room() -> RtiConfig {
        RtiConfig {
            origin_m: [0.0, 0.0],
            width_m: 13.4112,
            depth_m: 10.3632,
            cell_m: 0.25,
            ellipse_width_m: 0.6,
        }
    }

    /// The six independent links of the validated mesh, with responses
    /// synthesised from a known truth position through the same forward model
    /// the solver inverts, plus a flat resting level.
    fn observations_for(truth: [f64; 2], cfg: &RtiConfig) -> Vec<LinkObservation> {
        let pairs = [
            (NODE0, AP),
            (NODE1, AP),
            (NODE2, AP),
            (NODE0, NODE1),
            (NODE0, NODE2),
            (NODE1, NODE2),
        ];
        pairs
            .iter()
            .map(|&(rx, tx)| LinkObservation {
                rx,
                tx,
                response: 1.0 + 4.0 * link_weight(rx, tx, truth, cfg.ellipse_width_m),
                authority: 1.0,
            })
            .collect()
    }

    #[test]
    fn authority_falls_with_the_window_span_but_never_reaches_zero() {
        // The measured spread on this fleet, 2026-09-01.
        let live = temporal_authority(1.9);
        let median_fast = temporal_authority(10.4);
        let median_slow = temporal_authority(236.3);
        let worst = temporal_authority(4695.3);

        assert!(live > median_fast && median_fast > median_slow && median_slow > worst,
                "authority must fall monotonically with span");
        assert!(live > 0.8, "a 1.9 s window is about now: {live}");
        assert!(worst > 0.0,
                "nothing may be driven to zero -- that is a filter, and filtering \
                 deletes the weak cross-house links this exists to preserve");
        // The 78-minute link still speaks, at roughly a 400th of a live link's
        // volume. Present, and unable to outvote.
        assert!(worst < live / 100.0, "{worst} vs {live}");
    }

    #[test]
    fn authority_treats_an_unknown_span_as_full_not_stale() {
        // Every caller before authority existed behaved as weight 1.
        assert_eq!(temporal_authority(0.0), 1.0);
        assert_eq!(temporal_authority(-1.0), 1.0);
        assert_eq!(temporal_authority(f64::NAN), 1.0);
        assert_eq!(temporal_authority(f64::INFINITY), 1.0);
    }

    #[test]
    fn equal_authority_reproduces_the_unweighted_estimate_exactly() {
        // The doc comment on `correlation` claims equal weights give the plain
        // Pearson coefficient. If that ever stops holding, every pre-authority
        // result silently moves.
        let cfg = room();
        let truth = [6.0, 5.0];
        let a = estimate(&observations_for(truth, &cfg), &cfg, &[]).unwrap();

        let half: Vec<LinkObservation> = observations_for(truth, &cfg)
            .into_iter()
            .map(|o| LinkObservation { authority: 0.5, ..o })
            .collect();
        let b = estimate(&half, &cfg, &[]).unwrap();

        assert_eq!((a.x, a.y), (b.x, b.y),
                   "a uniform authority scale must not move the answer");
    }

    #[test]
    fn a_stale_link_cannot_outvote_the_live_ones() {
        // THE POINT OF THE WHOLE MECHANISM, stated as the property that
        // actually defines a weight: driving a link's authority toward zero
        // must approach OMITTING it, without ever removing it. Comparing error
        // against truth was the wrong assertion -- with one bad link among six
        // both answers land within a couple of cells and the comparison is
        // decided by rounding.
        let cfg = room();
        let truth = [3.0, 3.0];
        let elsewhere = [11.0, 8.0];

        // One link reports a response synthesised from the far side of the
        // room: the shape of a metric averaged over a window long enough to
        // describe somewhere the subject used to be.
        let mut obs = observations_for(truth, &cfg);
        obs[0].response = observations_for(elsewhere, &cfg)[0].response;

        let full = estimate(&obs, &cfg, &[]).unwrap();
        let full = [full.x, full.y];

        let mut down = obs.clone();
        down[0].authority = temporal_authority(4695.3);
        let down = estimate(&down, &cfg, &[]).unwrap();
        let down = [down.x, down.y];

        let omitted = estimate(&obs[1..], &cfg, &[]).unwrap();
        let omitted = [omitted.x, omitted.y];

        let d = |a: [f64; 2], b: [f64; 2]| ((a[0]-b[0]).powi(2) + (a[1]-b[1]).powi(2)).sqrt();

        // Non-vacuous: the corrupted link must actually matter at full weight.
        assert!(d(full, omitted) > 1e-9,
                "the stale link changes nothing even unweighted -- test proves nothing");
        // And the mechanism: down-weighting moves the answer toward omission.
        assert!(d(down, omitted) < d(full, omitted),
                "low authority must approach omission: down {:?} is {:.3} from                  omitted {:?}, but full-weight {:?} is {:.3}",
                down, d(down, omitted), omitted, full, d(full, omitted));
    }

    #[test]
    fn a_wholly_stale_fleet_still_produces_an_estimate() {
        // Weighting, not filtering: if EVERY link is slow there is still an
        // answer, because the alternative is a system that goes blind exactly
        // when the house is quiet -- which is most of the time.
        let cfg = room();
        let obs: Vec<LinkObservation> = observations_for([7.0, 4.0], &cfg)
            .into_iter()
            .map(|o| LinkObservation { authority: temporal_authority(3000.0), ..o })
            .collect();
        assert!(estimate(&obs, &cfg, &[]).is_some(),
                "an all-slow fleet must still be solvable");
    }

    #[test]
    fn a_target_on_a_link_line_produces_unit_weight() {
        let mid = [(NODE0[0] + NODE1[0]) / 2.0, 0.0];
        let w = link_weight(NODE0, NODE1, mid, 0.6);
        assert!((w - 1.0).abs() < 1e-9, "on-line weight should be 1, got {w}");
    }

    #[test]
    fn weight_decays_with_distance_from_the_link_line() {
        let mid = [(NODE0[0] + NODE1[0]) / 2.0, 0.0];
        let near = link_weight(NODE0, NODE1, [mid[0], 0.3], 0.6);
        let far = link_weight(NODE0, NODE1, [mid[0], 1.5], 0.6);
        assert!(near > far, "closer to the line must weigh more");
        assert!(far > 0.0 && near < 1.0);
    }

    #[test]
    fn normalisation_makes_a_weak_and_a_strong_link_comparable() {
        // The measured live spread: an AP link resting near 20 and a peer link
        // resting near 1.3, both at twice their own quiet level.
        let ap = normalise_response(40.0, 20.0);
        let peer = normalise_response(2.6, 1.3);
        assert!((ap - peer).abs() < 1e-12, "{ap} vs {peer}");
    }

    #[test]
    fn normalisation_refuses_a_zero_or_absent_scale() {
        assert_eq!(normalise_response(5.0, 0.0), 0.0);
        assert_eq!(normalise_response(5.0, -1.0), 0.0);
        assert_eq!(normalise_response(f64::NAN, 1.0), 0.0);
    }

    #[test]
    fn a_target_inside_the_node_triangle_is_recovered() {
        let cfg = room();
        let truth = [2.4, 1.4];
        let est = estimate(&observations_for(truth, &cfg), &cfg, &[]).expect("should solve");
        let err = ((est.x - truth[0]).powi(2) + (est.y - truth[1]).powi(2)).sqrt();
        assert!(err < 1.0, "error {err:.2} m at {:?}", (est.x, est.y));
    }

    /// The point of the whole module: the existing centroid tiers are a convex
    /// combination of node positions and cannot leave the node triangle. This
    /// one must be able to.
    #[test]
    fn a_target_outside_the_node_hull_is_not_dragged_back_into_it() {
        let cfg = room();
        // On the AP-to-node2 line, well beyond the triangle's far edge.
        let truth = [4.5, 4.6];
        let est = estimate(&observations_for(truth, &cfg), &cfg, &[]).expect("should solve");

        // Inside-triangle test by sign-of-cross-product against each edge.
        let inside = {
            let s = |a: [f64; 2], b: [f64; 2], p: [f64; 2]| {
                (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
            };
            let p = [est.x, est.y];
            let (d1, d2, d3) = (s(NODE0, NODE1, p), s(NODE1, NODE2, p), s(NODE2, NODE0, p));
            !((d1 < 0.0 || d2 < 0.0 || d3 < 0.0) && (d1 > 0.0 || d2 > 0.0 || d3 > 0.0))
        };
        assert!(
            !inside,
            "estimate {:?} fell back inside the node triangle",
            (est.x, est.y)
        );
    }

    #[test]
    fn fewer_than_three_links_is_refused() {
        let cfg = room();
        let obs = observations_for([2.4, 1.4], &cfg);
        assert!(estimate(&obs[..2], &cfg, &[]).is_none());
    }

    #[test]
    fn a_flat_response_vector_yields_no_estimate() {
        // Nothing is more perturbed than anything else: no spatial information,
        // so any returned point would be arbitrary.
        let cfg = room();
        let obs: Vec<LinkObservation> = observations_for([2.4, 1.4], &cfg)
            .into_iter()
            .map(|o| LinkObservation { response: 1.0, ..o })
            .collect();
        assert!(estimate(&obs, &cfg, &[]).is_none());
    }

    #[test]
    fn an_unobserved_corner_of_the_house_cannot_win() {
        // Truth placed where no link passes. The solver must not report that
        // corner confidently; the links carry no evidence about it.
        let cfg = room();
        let est = estimate(&observations_for([12.8, 9.8], &cfg), &cfg, &[]);
        if let Some(e) = est {
            let d = ((e.x - 12.8f64).powi(2) + (e.y - 9.8f64).powi(2)).sqrt();
            assert!(
                d > 1.0,
                "solver claimed an unobserved corner at {:?}",
                (e.x, e.y)
            );
        }
    }

    #[test]
    fn ambiguity_is_reported_as_spread_rather_than_hidden() {
        let cfg = room();
        let sharp = estimate(&observations_for([2.4, 1.4], &cfg), &cfg, &[]).expect("solves");

        // Three collinear links along one wall: many cells explain the same
        // pattern, so the near-peak set must be visibly spread out.
        let flat = vec![
            LinkObservation { rx: [0.0, 0.0], tx: [10.0, 0.0], response: 3.0, authority: 1.0 },
            LinkObservation { rx: [0.0, 0.1], tx: [10.0, 0.1], response: 3.0, authority: 1.0 },
            LinkObservation { rx: [0.0, 0.2], tx: [10.0, 0.2], response: 1.0, authority: 1.0 },
        ];
        if let Some(amb) = estimate(&flat, &cfg, &[]) {
            assert!(
                amb.spread_m > sharp.spread_m,
                "ambiguous geometry {:.2} m must spread wider than sharp {:.2} m",
                amb.spread_m,
                sharp.spread_m
            );
        }
    }

    #[test]
    fn a_degenerate_zero_length_link_contributes_nothing_instead_of_dividing_by_zero() {
        assert_eq!(link_weight(NODE0, NODE0, [1.0, 1.0], 0.6), 0.0);
        assert_eq!(link_weight(NODE0, NODE1, [1.0, 1.0], 0.0), 0.0);
    }

    // ── Footprint masking ────────────────────────────────────────────────

    #[test]
    fn a_notch_in_an_l_shaped_plan_is_outside_the_building() {
        // An L: the whole 10x10 box minus the south-east quadrant.
        let l = vec![
            [0.0, 0.0],
            [10.0, 0.0],
            [10.0, 5.0],
            [5.0, 5.0],
            [5.0, 10.0],
            [0.0, 10.0],
        ];
        assert!(point_in_polygon([2.0, 2.0], &l), "north-west block is indoors");
        assert!(point_in_polygon([2.0, 8.0], &l), "the south leg is indoors");
        assert!(point_in_polygon([8.0, 2.0], &l), "the east leg is indoors");
        assert!(
            !point_in_polygon([8.0, 8.0], &l),
            "the notch is the garden, not the house"
        );
    }

    #[test]
    fn a_ring_with_too_few_vertices_encloses_nothing() {
        // Not an error: `estimate` reads "no cell is inside" and returns None
        // rather than searching a shape that does not exist. The config
        // validator rejects such a ring before it can get this far.
        assert!(!point_in_polygon([1.0, 1.0], &[[0.0, 0.0], [2.0, 2.0]]));
        assert!(!point_in_polygon([1.0, 1.0], &[]));
    }

    #[test]
    fn a_peak_outside_the_footprint_cannot_win() {
        let cfg = room();
        // Truth in the middle of the node triangle, which the solver recovers
        // unmasked (see `a_target_inside_the_node_triangle_is_recovered`).
        let truth = [2.4, 1.4];
        let obs = observations_for(truth, &cfg);

        // Now declare that the real building is only the far east strip. The
        // cells that explain these observations best are all outside it, so
        // the answer must come from inside the strip -- or not at all.
        let east_strip = vec![
            [10.0, 0.0],
            [13.4, 0.0],
            [13.4, 10.3],
            [10.0, 10.3],
        ];
        if let Some(e) = estimate(&obs, &cfg, &[east_strip]) {
            assert!(
                e.x >= 10.0,
                "estimate {:?} landed outside the declared footprint",
                (e.x, e.y)
            );
        }
    }

    #[test]
    fn rings_are_a_union_not_a_single_outline() {
        // Two disjoint wings. A cell in either is searchable; the gap is not.
        let west = vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]];
        let east = vec![[8.0, 0.0], [12.0, 0.0], [12.0, 4.0], [8.0, 4.0]];
        let rings = [west, east];
        let inside = |p: [f64; 2]| rings.iter().any(|r| point_in_polygon(p, r));
        assert!(inside([2.0, 2.0]), "west wing");
        assert!(inside([10.0, 2.0]), "east wing");
        assert!(!inside([6.0, 2.0]), "the gap between them is not building");
    }

    // ── Grid origin ──────────────────────────────────────────────────────

    /// A building's origin is wherever its coordinates were first measured
    /// from, which is not necessarily its westmost or southmost point. Any
    /// part of it further out than that origin has negative coordinates, and a
    /// grid that always started at (0, 0) could not place anyone there at all.
    #[test]
    fn a_wing_west_of_the_origin_is_searchable_only_once_the_grid_moves() {
        // Four corners of a wing spanning x in [-4, 0], with both diagonals
        // as links so their crossing point is the strongest cell.
        let (nw, ne) = ([-4.0, 0.0], [0.0, 0.0]);
        let (sw, se) = ([-4.0, 4.0], [0.0, 4.0]);
        let truth = [-2.0, 2.0];
        let ellipse = 0.6;
        let obs: Vec<LinkObservation> = [(nw, se), (ne, sw), (nw, ne), (sw, se)]
            .iter()
            .map(|&(rx, tx)| LinkObservation {
                rx,
                tx,
                response: 1.0 + 4.0 * link_weight(rx, tx, truth, ellipse),
                authority: 1.0,
            })
            .collect();

        let wing = RtiConfig {
            origin_m: [-5.0, -1.0],
            width_m: 6.0,
            depth_m: 6.0,
            cell_m: 0.25,
            ellipse_width_m: ellipse,
        };
        let est = estimate(&obs, &wing, &[]).expect("the wing is inside this grid");
        let err = ((est.x - truth[0]).powi(2) + (est.y - truth[1]).powi(2)).sqrt();
        assert!(err < 1.0, "error {err:.2} m at {:?}", (est.x, est.y));

        // The same observations against a grid pinned to the origin: every
        // cell it can offer is east of the wing, so nothing it returns is the
        // truth. This is the failure the offset exists to remove.
        let pinned = RtiConfig { origin_m: [0.0, 0.0], ..wing };
        if let Some(e) = estimate(&obs, &pinned, &[]) {
            assert!(
                e.x >= 0.0,
                "a grid starting at x=0 reported a cell at x={:.2}",
                e.x
            );
        }
    }

    #[test]
    fn the_default_grid_still_starts_at_the_origin() {
        // Every caller that predates the offset must be unaffected by it.
        assert_eq!(RtiConfig::default().origin_m, [0.0, 0.0]);
    }
}
