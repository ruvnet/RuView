//! Check mode: compare measured node ranges with configured node positions.
//!
//! Warn-only. [`check`] returns a [`CheckReport`]; nothing in this module
//! produces positions to write back.

use std::collections::{BTreeMap, BTreeSet};

use crate::positions::distance;
use crate::report::{
    CheckParams, CheckReport, Finding, LinkResidual, LinkStatus, ScaleFit, Verdict,
};
use crate::{median, NodePositions, RangeObservation, SurveyError, MAX_OBSERVATIONS};

/// Largest node set the id-swap search permutes (8! = 40 320 assignments).
pub const MAX_SWAP_NODES: usize = 8;

/// Unit factors tried by the scale check: configured value × factor = metres.
const UNIT_FACTORS: [(&str, f64); 4] = [
    ("feet", 0.3048),
    ("inches", 0.0254),
    ("centimetres", 0.01),
    ("millimetres", 0.001),
];

/// Cap on one link's contribution to the assignment cost, in squared
/// tolerances, so one bad link cannot dominate the id-swap search.
const LINK_COST_CAP: f64 = 4.0;

/// Compare observations with configured positions. Never mutates either.
pub fn check(
    positions: &NodePositions,
    observations: &[RangeObservation],
    params: &CheckParams,
) -> Result<CheckReport, SurveyError> {
    if observations.len() > MAX_OBSERVATIONS {
        return Err(SurveyError::TooMany {
            what: "observations",
            got: observations.len(),
            max: MAX_OBSERVATIONS,
        });
    }
    let mut findings = Vec::new();
    let mut unknown: BTreeMap<u8, usize> = BTreeMap::new();
    let mut by_pair: BTreeMap<(u8, u8), Vec<&RangeObservation>> = BTreeMap::new();
    for o in observations {
        o.validate()?;
        let mut known = true;
        for node in [o.initiator, o.responder] {
            if !positions.contains_key(&node) {
                *unknown.entry(node).or_default() += 1;
                known = false;
            }
        }
        if known {
            by_pair.entry(o.pair()).or_default().push(o);
        }
    }
    for (node, n) in unknown {
        findings.push(Finding::UnknownNode {
            node,
            observations: n,
        });
    }

    let links: Vec<LinkResidual> = by_pair
        .iter()
        .map(|(&(a, b), obs)| link_residual(a, b, obs, positions, params))
        .collect();
    let scored: Vec<&LinkResidual> = links
        .iter()
        .filter(|l| matches!(l.status, LinkStatus::Ok | LinkStatus::Outlier))
        .collect();

    let ranged: BTreeSet<u8> = scored.iter().flat_map(|l| [l.a, l.b]).collect();
    if !scored.is_empty() {
        for node in positions.keys().filter(|n| !ranged.contains(n)) {
            findings.push(Finding::UnrangedNode { node: *node });
        }
    }
    if scored.is_empty() {
        return Ok(CheckReport {
            links,
            scale: None,
            findings,
            verdict: Verdict::Insufficient,
        });
    }

    let scale = fit_scale(&scored);
    let unit = scale
        .as_ref()
        .and_then(|s| unit_mismatch(s.scale, &scored, params));
    let explained = unit.is_some();
    findings.extend(unit);

    let mut swapped = false;
    if !explained {
        if ranged.len() > MAX_SWAP_NODES {
            findings.push(Finding::SwapSearchSkipped {
                nodes: ranged.len(),
            });
        } else if ranged.len() >= 3 {
            if let Some(f) = swap_search(&ranged, &scored, positions, params) {
                swapped = matches!(f, Finding::IdSwapSuggested { .. });
                findings.push(f);
            }
        }
    }

    if !explained && !swapped {
        let moved = moved_nodes(&ranged, &scored);
        for l in scored.iter().filter(|l| l.status == LinkStatus::Outlier) {
            if !moved.contains(&l.a) && !moved.contains(&l.b) {
                findings.push(Finding::LinkOutlier {
                    a: l.a,
                    b: l.b,
                    residual_m: l.residual_m,
                });
            }
        }
        for node in &moved {
            let touching: Vec<&&LinkResidual> = scored
                .iter()
                .filter(|l| l.a == *node || l.b == *node)
                .collect();
            findings.push(Finding::NodeMoved {
                node: *node,
                flagged_links: touching
                    .iter()
                    .filter(|l| l.status == LinkStatus::Outlier)
                    .count(),
                links: touching.len(),
            });
        }
    }

    let verdict = if findings.iter().any(Finding::is_warning) {
        Verdict::Warnings
    } else {
        Verdict::Consistent
    };
    Ok(CheckReport {
        links,
        scale,
        findings,
        verdict,
    })
}

fn link_residual(
    a: u8,
    b: u8,
    obs: &[&RangeObservation],
    positions: &NodePositions,
    params: &CheckParams,
) -> LinkResidual {
    let usable: Vec<&&RangeObservation> = obs
        .iter()
        .filter(|o| o.rssi_dbm.is_none_or(|r| r >= params.min_rssi_dbm))
        .collect();
    let pool: Vec<&&RangeObservation> = if usable.is_empty() {
        obs.iter().collect()
    } else {
        usable.clone()
    };
    let ranges: Vec<f64> = pool.iter().map(|o| o.range_m).collect();
    let sigmas: Vec<f64> = pool.iter().map(|o| o.sigma_m).collect();
    let measured = median(&ranges).unwrap_or(0.0);
    let session_sigma = median(&sigmas).unwrap_or(0.0);
    let spread = if ranges.len() >= 3 {
        let mean = ranges.iter().sum::<f64>() / ranges.len() as f64;
        (ranges.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / ranges.len() as f64).sqrt()
    } else {
        0.0
    };
    let sigma = session_sigma.max(spread);
    let calibrated = pool.iter().all(|o| o.calibrated);
    let expected = distance(&positions[&a], &positions[&b]);
    let floor = if calibrated {
        params.calibrated_floor_m
    } else {
        params.floor_m
    };
    let tolerance = floor + params.sigma_k * sigma;
    let residual = measured - expected;
    let status = if usable.is_empty() {
        LinkStatus::Weak
    } else if expected < params.min_expected_m {
        LinkStatus::TooShort
    } else if sigma > params.max_sigma_m {
        LinkStatus::Noisy
    } else if residual.abs() > tolerance {
        LinkStatus::Outlier
    } else {
        LinkStatus::Ok
    };
    LinkResidual {
        a,
        b,
        measured_m: measured,
        expected_m: expected,
        residual_m: residual,
        tolerance_m: tolerance,
        sigma_m: sigma,
        observations: pool.len(),
        calibrated,
        status,
    }
}

fn fit_scale(scored: &[&LinkResidual]) -> Option<ScaleFit> {
    let usable: Vec<&&LinkResidual> = scored.iter().filter(|l| l.expected_m > 0.0).collect();
    if usable.len() < 2 {
        return None;
    }
    let num: f64 = usable.iter().map(|l| l.measured_m * l.expected_m).sum();
    let den: f64 = usable.iter().map(|l| l.expected_m * l.expected_m).sum();
    (den > 0.0).then(|| ScaleFit {
        scale: num / den,
        links: usable.len(),
    })
}

fn unit_mismatch(scale: f64, scored: &[&LinkResidual], params: &CheckParams) -> Option<Finding> {
    let outliers = scored
        .iter()
        .filter(|l| l.status == LinkStatus::Outlier)
        .count();
    if outliers == 0 {
        return None;
    }
    UNIT_FACTORS.iter().find_map(|&(unit, factor)| {
        if ((scale / factor) - 1.0).abs() > params.unit_band {
            return None;
        }
        let rescaled = scored
            .iter()
            .filter(|l| (l.measured_m - l.expected_m * factor).abs() > l.tolerance_m)
            .count();
        (rescaled < outliers).then(|| Finding::UnitMismatch {
            unit: unit.to_string(),
            factor,
            fitted_scale: scale,
        })
    })
}

fn swap_search(
    ranged: &BTreeSet<u8>,
    scored: &[&LinkResidual],
    positions: &NodePositions,
    params: &CheckParams,
) -> Option<Finding> {
    let nodes: Vec<u8> = ranged.iter().copied().collect();
    let index: BTreeMap<u8, usize> = nodes.iter().enumerate().map(|(i, n)| (*n, i)).collect();
    let pos: Vec<[f64; 3]> = nodes.iter().map(|n| positions[n]).collect();
    let links: Vec<(usize, usize, f64, f64)> = scored
        .iter()
        .map(|l| (index[&l.a], index[&l.b], l.measured_m, l.tolerance_m))
        .collect();
    let cost = |perm: &[usize]| -> f64 {
        links
            .iter()
            .map(|&(i, j, m, tol)| {
                let r = (m - distance(&pos[perm[i]], &pos[perm[j]])) / tol;
                (r * r).min(LINK_COST_CAP)
            })
            .sum()
    };

    let n = nodes.len();
    let identity: Vec<usize> = (0..n).collect();
    let configured_cost = cost(&identity);
    let mut all: Vec<(f64, Vec<usize>)> = Vec::new();
    for_each_permutation(n, |perm| all.push((cost(perm), perm.to_vec())));
    let best = all.iter().map(|(c, _)| *c).fold(f64::INFINITY, f64::min);
    let tie = best + 0.1 * best + 0.05;
    if configured_cost <= tie {
        return None;
    }
    if configured_cost < params.min_swap_cost || best > params.swap_gain * configured_cost {
        return None;
    }
    // Several assignments can fit equally well: ranges cannot tell a layout
    // from its reflections, and corner-mounted nodes in a rectangular room
    // are nearly symmetric. Prefer the assignment that changes the fewest
    // ids (an operator typo moves few ids), and give up only when that
    // minimum is itself tied.
    let changed = |perm: &[usize]| perm.iter().enumerate().filter(|(i, p)| i != *p).count();
    let winners: Vec<&(f64, Vec<usize>)> = all.iter().filter(|(c, _)| *c <= tie).collect();
    let fewest = winners.iter().map(|(_, p)| changed(p)).min()?;
    let minimal: Vec<&&(f64, Vec<usize>)> = winners
        .iter()
        .filter(|(_, p)| changed(p) == fewest)
        .collect();
    if minimal.len() > 1 {
        return Some(Finding::IdSwapAmbiguous {
            equivalent_assignments: winners.len(),
        });
    }
    let (best_cost, perm) = minimal[0];
    let assignment = perm
        .iter()
        .enumerate()
        .filter(|(i, p)| *i != **p)
        .map(|(i, p)| (nodes[i], nodes[*p]))
        .collect();
    Some(Finding::IdSwapSuggested {
        assignment,
        cost_configured: configured_cost,
        cost_suggested: *best_cost,
        equivalent_assignments: winners.len(),
    })
}

/// Visit every permutation of `0..n` (Heap's algorithm, iterative).
fn for_each_permutation(n: usize, mut visit: impl FnMut(&[usize])) {
    let mut perm: Vec<usize> = (0..n).collect();
    let mut c = vec![0usize; n];
    visit(&perm);
    let mut i = 1;
    while i < n {
        if c[i] < i {
            if i % 2 == 0 {
                perm.swap(0, i);
            } else {
                perm.swap(c[i], i);
            }
            visit(&perm);
            c[i] += 1;
            i = 1;
        } else {
            c[i] = 0;
            i += 1;
        }
    }
}

fn moved_nodes(ranged: &BTreeSet<u8>, scored: &[&LinkResidual]) -> BTreeSet<u8> {
    ranged
        .iter()
        .copied()
        .filter(|node| {
            let touching: Vec<&&LinkResidual> = scored
                .iter()
                .filter(|l| l.a == *node || l.b == *node)
                .collect();
            let flagged = touching
                .iter()
                .filter(|l| l.status == LinkStatus::Outlier)
                .count();
            flagged >= 2 && 2 * flagged > touching.len()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heap_visits_every_permutation_once() {
        for n in 0..=5 {
            let mut seen = BTreeSet::new();
            let mut count = 0;
            for_each_permutation(n, |p| {
                seen.insert(p.to_vec());
                count += 1;
            });
            let expected: usize = (1..=n).product();
            assert_eq!(count, expected.max(1));
            assert_eq!(seen.len(), expected.max(1));
        }
    }

    #[test]
    fn no_scored_links_is_insufficient() {
        let positions = crate::parse_node_positions("1:0,0,0;2:0.5,0,0").unwrap();
        let obs = [RangeObservation::ftm(1, 2, 0.0, 0.1)];
        let r = check(&positions, &obs, &CheckParams::ftm_uncalibrated()).unwrap();
        assert_eq!(r.verdict, Verdict::Insufficient);
        assert_eq!(r.links[0].status, LinkStatus::TooShort);
    }

    #[test]
    fn rejects_too_many_observations() {
        let positions = crate::parse_node_positions("1:0,0,0;2:3,0,0").unwrap();
        let obs = vec![RangeObservation::ftm(1, 2, 3.0, 0.1); MAX_OBSERVATIONS + 1];
        assert!(matches!(
            check(&positions, &obs, &CheckParams::ftm_uncalibrated()),
            Err(SurveyError::TooMany { .. })
        ));
    }
}
