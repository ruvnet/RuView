//! Bounded RF descent followed by exact linear-task block descent.
//! Both blocks minimize the same declared joint objective; no validation tuning.
use super::{objective, FitConfig, ObjectiveValues, Row};
use crate::{
    room_learner::model::{extractor, sigmoid, RoomModel},
    trainer::{clip_gradients, estimate_gradient, SgdOptimizer},
};
use std::cell::Cell;
pub(super) fn head_step(
    model: &RoomModel,
    rows: &[Row],
    params: &[f32],
    config: &FitConfig,
    current: ObjectiveValues,
    check: &impl Fn() -> Result<(), String>,
    evaluations: &Cell<usize>,
) -> Result<Option<(Vec<f32>, ObjectiveValues)>, String> {
    let offset = model.representation_parameter_count();
    let mut gradient = vec![0.0; params.len()];
    let mut encoder = extractor(&model.config);
    encoder
        .transformer
        .unflatten_weights(&params[..model.encoder_weights.len()])?;
    let projectors = model.source_projectors(params)?;
    for row in rows {
        check()?;
        let (features, _) = model.represent_with_encoder(&row.tokens, &mut encoder, &projectors)?;
        let residual = (sigmoid(model.logit(&features, &params[offset..])) - row.occupied)
            * config.supervised_weight
            / rows.len() as f32;
        for (g, x) in gradient[offset..].iter_mut().zip(features) {
            *g += residual * x;
        }
        *gradient.last_mut().unwrap() += residual;
    }
    clip_gradients(&mut gradient, 5.0);
    for backtrack in 0..8 {
        check()?;
        let mut next = params.to_vec();
        SgdOptimizer::new(config.learning_rate * 0.5f32.powi(backtrack), 0.0, 0.0)
            .step(&mut next, &gradient);
        evaluations.set(evaluations.get() + 1);
        let loss = objective(model, rows, &next, config)?;
        if loss.total < current.total {
            return Ok(Some((next, loss)));
        }
    }
    Ok(None)
}

/// A second, smaller finite-difference radius handles ReLU boundaries without
/// changing the objective. Every probe remains inside the caller's hard budgets.
pub(super) fn joint_step(
    model: &RoomModel,
    rows: &[Row],
    params: &[f32],
    config: &FitConfig,
    current: ObjectiveValues,
    check: &impl Fn() -> Result<(), String>,
    evaluations: &Cell<usize>,
) -> Result<Option<(Vec<f32>, ObjectiveValues)>, String> {
    for attempt in 0..2 {
        check()?;
        let epsilon = (config.gradient_epsilon * 0.25f32.powi(attempt)).max(0.0001);
        if attempt > 0 && epsilon == config.gradient_epsilon {
            break;
        }
        let mut gradient = estimate_gradient(
            |p| {
                if check().is_err() {
                    return f32::NAN;
                }
                evaluations.set(evaluations.get() + 1);
                objective(model, rows, p, config)
                    .map(|l| l.total)
                    .unwrap_or(f32::NAN)
            },
            params,
            epsilon,
        );
        check()?;
        if !gradient.iter().all(|g| g.is_finite()) {
            return Err("non-finite training gradient".into());
        }
        clip_gradients(&mut gradient, 5.0);
        for backtrack in 0..16 {
            check()?;
            let mut next = params.to_vec();
            SgdOptimizer::new(config.learning_rate * 0.5f32.powi(backtrack), 0.0, 0.0)
                .step(&mut next, &gradient);
            evaluations.set(evaluations.get() + 1);
            let loss = objective(model, rows, &next, config)?;
            if loss.total < current.total {
                return Ok(Some((next, loss)));
            }
        }
    }
    Ok(None)
}
pub(super) fn require_rf_update(
    model: &RoomModel,
    params: &[f32],
    initial: &[f32],
) -> Result<(), String> {
    let backbone = model.encoder_weights.len();
    let representation = model.representation_parameter_count();
    if params[..backbone] == initial[..backbone]
        || params[backbone..representation] == initial[backbone..representation]
    {
        return Err(
            "no improving backbone/projection update; refusing a head-only RF candidate".into(),
        );
    }
    Ok(())
}

/// Task gradients must not be suppressed by a clipped RF gradient or its small
/// accepted step. Optimize both blocks against the same Train-only objective.
pub(super) fn epoch_step(
    model: &RoomModel,
    rows: &[Row],
    params: &[f32],
    config: &FitConfig,
    current: ObjectiveValues,
    check: &impl Fn() -> Result<(), String>,
    evaluations: &Cell<usize>,
) -> Result<Option<(Vec<f32>, ObjectiveValues)>, String> {
    let mut next = params.to_vec();
    let mut loss = current;
    let mut changed = false;
    if let Some((p, l)) = joint_step(model, rows, params, config, current, check, evaluations)? {
        next = p;
        loss = l;
        changed = true;
    }
    if let Some((p, l)) = head_step(model, rows, &next, config, loss, check, evaluations)? {
        next = p;
        loss = l;
        changed = true;
    }
    Ok(if changed { Some((next, loss)) } else { None })
}
