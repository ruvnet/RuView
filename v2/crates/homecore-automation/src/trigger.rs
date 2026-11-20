//! `Trigger` enum and `EvaluateTrigger` trait.
//!
//! Covers the four most common HA trigger platforms as required by ADR-129 P1:
//! `state`, `numeric_state`, `time`, and `event`. Additional platforms land
//! in P2 (template, zone, sun, MQTT, webhook, etc.).

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use homecore::{EntityId, State};

/// Context produced by a fired trigger. Passed into condition evaluation and
/// template rendering as `trigger.*` variables.
#[derive(Clone, Debug)]
pub struct TriggerContext {
    /// Which trigger platform fired.
    pub platform: String,
    /// Entity ID (for state / numeric_state triggers).
    pub entity_id: Option<EntityId>,
    /// New state snapshot (for state / numeric_state triggers).
    pub to_state: Option<Arc<State>>,
    /// Previous state snapshot (for state / numeric_state triggers).
    pub from_state: Option<Arc<State>>,
    /// When the trigger fired.
    pub fired_at: DateTime<Utc>,
    /// Event type (for event triggers).
    pub event_type: Option<String>,
}

impl TriggerContext {
    pub fn state_changed(
        entity_id: EntityId,
        from: Option<Arc<State>>,
        to: Option<Arc<State>>,
    ) -> Self {
        Self {
            platform: "state".into(),
            entity_id: Some(entity_id),
            to_state: to,
            from_state: from,
            fired_at: Utc::now(),
            event_type: None,
        }
    }

    pub fn event(event_type: impl Into<String>) -> Self {
        Self {
            platform: "event".into(),
            entity_id: None,
            to_state: None,
            from_state: None,
            fired_at: Utc::now(),
            event_type: Some(event_type.into()),
        }
    }
}

/// Async evaluation trait. Each trigger variant implements this to decide
/// whether a given `TriggerContext` matches its configuration.
#[async_trait]
pub trait EvaluateTrigger: Send + Sync {
    async fn matches(&self, ctx: &TriggerContext) -> bool;
}

/// Trigger configuration. Deserialized from YAML `trigger:` blocks.
///
/// Only four platforms are implemented in P1 (ADR-129 §6 Phase 1).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "platform", rename_all = "snake_case")]
pub enum Trigger {
    /// Fires when an entity's state changes.
    State {
        entity_id: EntityId,
        /// Optional: only fire if state was previously this value.
        #[serde(default)]
        from: Option<String>,
        /// Optional: only fire if state transitions to this value.
        #[serde(default)]
        to: Option<String>,
    },
    /// Fires when an entity's numeric state crosses a threshold.
    NumericState {
        entity_id: EntityId,
        /// Fire when value rises above this threshold.
        #[serde(default)]
        above: Option<f64>,
        /// Fire when value drops below this threshold.
        #[serde(default)]
        below: Option<f64>,
    },
    /// Fires at a specific time of day (HH:MM:SS).
    Time {
        at: String,
    },
    /// Fires when a named domain event is published on the event bus.
    Event {
        event_type: String,
    },
}

impl Trigger {
    /// Synchronous check — does this trigger configuration match the provided
    /// context? Used directly in tests and by the engine's event loop.
    pub fn matches_sync(&self, ctx: &TriggerContext) -> bool {
        match self {
            Trigger::State { entity_id, from, to } => {
                let eid_match = ctx.entity_id.as_ref() == Some(entity_id);
                if !eid_match {
                    return false;
                }
                // With from/to constraints only a state-value transition
                // qualifies. Unconstrained triggers still observe attributes.
                if (from.is_some() || to.is_some())
                    && ctx.from_state.as_ref().zip(ctx.to_state.as_ref())
                        .is_some_and(|(old, new)| old.state == new.state)
                {
                    return false;
                }
                if let Some(expected_from) = from {
                    let actual_from = ctx.from_state.as_ref().map(|s| s.state.as_str()).unwrap_or("unavailable");
                    if actual_from != expected_from.as_str() {
                        return false;
                    }
                }
                if let Some(expected_to) = to {
                    let actual_to = ctx.to_state.as_ref().map(|s| s.state.as_str()).unwrap_or("unavailable");
                    if actual_to != expected_to.as_str() {
                        return false;
                    }
                }
                true
            }
            Trigger::NumericState { entity_id, above, below } => {
                let eid_match = ctx.entity_id.as_ref() == Some(entity_id);
                if !eid_match {
                    return false;
                }
                let value: f64 = ctx
                    .to_state
                    .as_ref()
                    .and_then(|s| s.state.parse().ok())
                    .unwrap_or(f64::NAN);
                if value.is_nan() {
                    return false;
                }
                if let Some(a) = above {
                    if value <= *a {
                        return false;
                    }
                }
                if let Some(b) = below {
                    if value >= *b {
                        return false;
                    }
                }
                true
            }
            Trigger::Time { .. } => {
                // Time triggers are wall-clock based and have no state-change
                // context to match here. They are evaluated by the engine's
                // 1 Hz timer task (`AutomationEngine::start_timer`, HC-WS-04 /
                // ADR-161), which compares the trigger's `at` against the local
                // wall-clock second. `matches_sync` therefore returns false for
                // `Time` on the state-change path by design.
                false
            }
            Trigger::Event { event_type } => {
                ctx.event_type.as_deref() == Some(event_type.as_str())
            }
        }
    }
}

#[async_trait]
impl EvaluateTrigger for Trigger {
    async fn matches(&self, ctx: &TriggerContext) -> bool {
        self.matches_sync(ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use homecore::{Context, EntityId, State};
    use std::sync::Arc;

    fn make_state(entity_id: &str, state: &str) -> Arc<State> {
        Arc::new(State::new(
            EntityId::parse(entity_id).unwrap(),
            state,
            serde_json::json!({}),
            Context::new(),
        ))
    }

    fn state_ctx(entity_id: &str, from: &str, to: &str) -> TriggerContext {
        let eid = EntityId::parse(entity_id).unwrap();
        TriggerContext::state_changed(
            eid,
            Some(make_state(entity_id, from)),
            Some(make_state(entity_id, to)),
        )
    }

    #[test]
    fn constrained_state_trigger_ignores_attribute_only_changes() {
        let eid = EntityId::parse("light.kitchen").unwrap();
        let ctx = TriggerContext::state_changed(
            eid.clone(),
            Some(Arc::new(State::new(
                eid.clone(),
                "on",
                serde_json::json!({"brightness": 10}),
                Context::new(),
            ))),
            Some(Arc::new(State::new(
                eid.clone(),
                "on",
                serde_json::json!({"brightness": 20}),
                Context::new(),
            ))),
        );
        for (from, to) in [(None, Some("on".into())), (Some("on".into()), None)] {
            assert!(!Trigger::State {
                entity_id: eid.clone(),
                from,
                to
            }
            .matches_sync(&ctx));
        }
        assert!(Trigger::State {
            entity_id: eid,
            from: None,
            to: None
        }
        .matches_sync(&ctx));
    }

    #[test]
    fn state_trigger_exact_from_to_match() {
        let trigger = Trigger::State {
            entity_id: EntityId::parse("light.kitchen").unwrap(),
            from: Some("off".into()),
            to: Some("on".into()),
        };
        let ctx = state_ctx("light.kitchen", "off", "on");
        assert!(trigger.matches_sync(&ctx));
    }

    #[test]
    fn state_trigger_wrong_entity_no_match() {
        let trigger = Trigger::State {
            entity_id: EntityId::parse("light.kitchen").unwrap(),
            from: None,
            to: Some("on".into()),
        };
        let ctx = state_ctx("switch.hallway", "off", "on");
        assert!(!trigger.matches_sync(&ctx));
    }

    #[test]
    fn state_trigger_wrong_to_no_match() {
        let trigger = Trigger::State {
            entity_id: EntityId::parse("light.kitchen").unwrap(),
            from: None,
            to: Some("on".into()),
        };
        let ctx = state_ctx("light.kitchen", "on", "off");
        assert!(!trigger.matches_sync(&ctx));
    }

    #[test]
    fn state_trigger_no_constraints_matches_any_change() {
        let trigger = Trigger::State {
            entity_id: EntityId::parse("light.kitchen").unwrap(),
            from: None,
            to: None,
        };
        let ctx = state_ctx("light.kitchen", "off", "on");
        assert!(trigger.matches_sync(&ctx));
    }

    #[test]
    fn numeric_trigger_above_threshold_fires() {
        let trigger = Trigger::NumericState {
            entity_id: EntityId::parse("sensor.temperature").unwrap(),
            above: Some(25.0),
            below: None,
        };
        let mut ctx = state_ctx("sensor.temperature", "20", "26");
        ctx.to_state = Some(make_state("sensor.temperature", "26"));
        assert!(trigger.matches_sync(&ctx));
    }

    #[test]
    fn numeric_trigger_below_threshold_no_fire() {
        let trigger = Trigger::NumericState {
            entity_id: EntityId::parse("sensor.temperature").unwrap(),
            above: Some(25.0),
            below: None,
        };
        let mut ctx = state_ctx("sensor.temperature", "20", "24");
        ctx.to_state = Some(make_state("sensor.temperature", "24"));
        assert!(!trigger.matches_sync(&ctx));
    }

    #[test]
    fn numeric_trigger_between_bounds() {
        let trigger = Trigger::NumericState {
            entity_id: EntityId::parse("sensor.humidity").unwrap(),
            above: Some(30.0),
            below: Some(80.0),
        };
        let mut ctx = state_ctx("sensor.humidity", "20", "50");
        ctx.to_state = Some(make_state("sensor.humidity", "50"));
        assert!(trigger.matches_sync(&ctx));
    }

    #[test]
    fn event_trigger_matches_type() {
        let trigger = Trigger::Event { event_type: "my_custom_event".into() };
        let ctx = TriggerContext::event("my_custom_event");
        assert!(trigger.matches_sync(&ctx));
    }

    #[test]
    fn event_trigger_no_match_wrong_type() {
        let trigger = Trigger::Event { event_type: "my_custom_event".into() };
        let ctx = TriggerContext::event("other_event");
        assert!(!trigger.matches_sync(&ctx));
    }

    #[tokio::test]
    async fn evaluate_trigger_trait_object() {
        let trigger: Box<dyn EvaluateTrigger> = Box::new(Trigger::Event {
            event_type: "boot".into(),
        });
        let ctx = TriggerContext::event("boot");
        assert!(trigger.matches(&ctx).await);
    }
}
