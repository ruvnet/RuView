//! MiniJinja-based template environment with HA-compatible globals.
//!
//! ADR-129 §2.1 — P1 ships four HA globals: `states()`, `state_attr()`,
//! `is_state()`, `now()`. The `utcnow()`, `as_timestamp()`, `distance()`,
//! and `iif()` globals plus custom filters land in P2.

use std::sync::Arc;

use chrono::Utc;
use minijinja::{Environment, Value};

use homecore::{EntityId, StateMachine};

use crate::error::AutomationError;

/// Instruction budget for a single template render (HC-SEC-01).
///
/// Templates come from user automation config; without a bound a single
/// `template:` condition like
/// `{% for i in range(10000) %}{% for j in range(10000) %}x{% endfor %}{% endfor %}`
/// renders a multi-gigabyte string and pins a CPU for tens of seconds —
/// a memory/CPU denial-of-service (the bfld-class "unbounded expansion").
/// MiniJinja's `fuel` feature charges ~1 unit per VM instruction; a
/// nested loop burns one unit per iteration, so the budget caps total
/// work regardless of how the loops are nested. 1,000,000 instructions is
/// far more than any legitimate HA template needs (a typical condition is
/// a few dozen) while killing the attack in well under a second.
const TEMPLATE_FUEL: u64 = 1_000_000;

/// Hard cap on the source length of a template (HC-SEC-01, defense in
/// depth). A legitimate HA `value_template` is a one-liner; anything past
/// 64 KiB is rejected before compilation so a pathological source string
/// can neither be compiled nor emitted verbatim.
const MAX_TEMPLATE_SOURCE_BYTES: usize = 64 * 1024;

/// MiniJinja environment pre-loaded with HA-compatible globals.
///
/// Constructed once per `AutomationEngine` and shared via `Arc`. The
/// globals close over an `Arc<StateMachine>` so every template render
/// sees the live current state.
pub struct TemplateEnvironment {
    env: Environment<'static>,
}

impl TemplateEnvironment {
    /// Build a new environment backed by the given state machine.
    pub fn new(states: Arc<StateMachine>) -> Self {
        let mut env = Environment::new();

        // Bound per-render work so a hostile `template:` condition cannot
        // DoS the engine via nested loops / huge repeats (HC-SEC-01).
        env.set_fuel(Some(TEMPLATE_FUEL));

        // --- states(entity_id) ---
        // Returns the current state string of an entity, or "unavailable".
        let states_sm = Arc::clone(&states);
        env.add_global(
            "states",
            Value::from_function(move |entity_id: String| -> String {
                EntityId::parse(&entity_id)
                    .ok()
                    .and_then(|eid| states_sm.get(&eid))
                    .map(|s| s.state.clone())
                    .unwrap_or_else(|| "unavailable".into())
            }),
        );

        // --- state_attr(entity_id, attribute) ---
        // Preserve JSON types so numeric comparisons and nested access work.
        let attr_sm = Arc::clone(&states);
        env.add_global(
            "state_attr",
            Value::from_function(move |entity_id: String, attr: String| -> Value {
                EntityId::parse(&entity_id)
                    .ok()
                    .and_then(|eid| attr_sm.get(&eid))
                    .and_then(|s| s.attributes.get(&attr).cloned())
                    .map(Value::from_serialize)
                    .unwrap_or_else(|| Value::from_serialize(serde_json::Value::Null))
            }),
        );

        // --- is_state(entity_id, state) ---
        // Returns true if the entity's current state matches the given value.
        let is_state_sm = Arc::clone(&states);
        env.add_global(
            "is_state",
            Value::from_function(move |entity_id: String, expected: String| -> bool {
                EntityId::parse(&entity_id)
                    .ok()
                    .and_then(|eid| is_state_sm.get(&eid))
                    .map(|s| s.state == expected)
                    .unwrap_or(false)
            }),
        );

        // --- now() ---
        // Returns the current UTC datetime as an ISO 8601 string.
        // HA returns a Python datetime; MiniJinja returns a string which
        // templates can further format with the `strftime` filter.
        env.add_global(
            "now",
            Value::from_function(|| -> String {
                Utc::now().format("%Y-%m-%dT%H:%M:%S%.6f+00:00").to_string()
            }),
        );

        Self { env }
    }

    /// Render a template string and return the string output.
    ///
    /// Renders are bounded by an instruction budget ([`TEMPLATE_FUEL`]) and
    /// a source-length cap ([`MAX_TEMPLATE_SOURCE_BYTES`]); a malicious
    /// template that exhausts the budget returns a [`AutomationError::TemplateRender`]
    /// error rather than running unbounded (HC-SEC-01).
    pub fn render(&self, template_str: &str) -> Result<String, AutomationError> {
        // Reject pathologically large sources before compilation (defense
        // in depth — fuel already bounds runtime work).
        if template_str.len() > MAX_TEMPLATE_SOURCE_BYTES {
            return Err(AutomationError::TemplateRender(format!(
                "template source too large: {} bytes (max {})",
                template_str.len(),
                MAX_TEMPLATE_SOURCE_BYTES
            )));
        }
        // Wrap bare expressions like `{{ states('light.kitchen') }}`
        // in a minimal template wrapper.
        let tmpl = self
            .env
            .template_from_str(template_str)
            .map_err(|e| AutomationError::TemplateRender(e.to_string()))?;
        tmpl.render(())
            .map_err(|e| AutomationError::TemplateRender(e.to_string()))
    }

    /// Render a template and interpret the output as a boolean.
    /// "true", "1", "yes", "on" → true. Everything else → false.
    pub fn render_bool(&self, template_str: &str) -> Result<bool, AutomationError> {
        let raw = self.render(template_str)?;
        let v = raw.trim().to_ascii_lowercase();
        Ok(matches!(v.as_str(), "true" | "1" | "yes" | "on"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use homecore::{Context, EntityId, StateMachine};
    use std::sync::Arc;

    fn sm_with(entity_id: &str, state: &str, attrs: serde_json::Value) -> Arc<StateMachine> {
        let sm = Arc::new(StateMachine::new());
        sm.set(EntityId::parse(entity_id).unwrap(), state, attrs, Context::new());
        sm
    }

    #[test]
    fn states_global_returns_current_state() {
        let sm = sm_with("light.kitchen", "on", serde_json::json!({}));
        let env = TemplateEnvironment::new(sm);
        let out = env.render("{{ states('light.kitchen') }}").unwrap();
        assert_eq!(out.trim(), "on");
    }

    #[test]
    fn states_global_unknown_entity_returns_unavailable() {
        let sm = Arc::new(StateMachine::new());
        let env = TemplateEnvironment::new(sm);
        let out = env.render("{{ states('sensor.unknown') }}").unwrap();
        assert_eq!(out.trim(), "unavailable");
    }

    #[test]
    fn state_attr_preserves_types_for_conditions_and_nested_access() {
        let env = TemplateEnvironment::new(sm_with(
            "light.kitchen",
            "on",
            serde_json::json!({
                "brightness": 200, "active": false, "rooms": ["hall", "kitchen"], "meta": {"count": 2}
            }),
        ));
        for template in [
            "{{ state_attr('light.kitchen', 'brightness') > 100 }}",
            "{{ state_attr('light.kitchen', 'active') == false }}",
            "{{ state_attr('light.kitchen', 'rooms')[1] == 'kitchen' }}",
            "{{ state_attr('light.kitchen', 'meta').count == 2 }}",
            "{{ state_attr('light.kitchen', 'missing') is none }}",
        ] {
            assert!(env.render_bool(template).unwrap(), "{template}");
        }
    }

    #[test]
    fn state_attr_returns_attribute_value() {
        let sm = sm_with(
            "light.kitchen",
            "on",
            serde_json::json!({"brightness": 200}),
        );
        let env = TemplateEnvironment::new(sm);
        let out = env.render("{{ state_attr('light.kitchen', 'brightness') }}").unwrap();
        assert_eq!(out.trim(), "200");
    }

    #[test]
    fn is_state_global_true_when_matches() {
        let sm = sm_with("switch.fan", "on", serde_json::json!({}));
        let env = TemplateEnvironment::new(sm);
        let out = env.render("{{ is_state('switch.fan', 'on') }}").unwrap();
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn is_state_global_false_when_no_match() {
        let sm = sm_with("switch.fan", "off", serde_json::json!({}));
        let env = TemplateEnvironment::new(sm);
        let out = env.render("{{ is_state('switch.fan', 'on') }}").unwrap();
        assert_eq!(out.trim(), "false");
    }

    #[test]
    fn now_global_returns_timestamp_string() {
        let sm = Arc::new(StateMachine::new());
        let env = TemplateEnvironment::new(sm);
        let out = env.render("{{ now() }}").unwrap();
        // Should be an ISO 8601 datetime string containing 'T'
        assert!(out.contains('T'), "now() returned: {out}");
    }

    #[test]
    fn render_bool_true_values() {
        let sm = Arc::new(StateMachine::new());
        let env = TemplateEnvironment::new(sm);
        for tmpl in &["true", "1", "yes", "on"] {
            let result = env.render_bool(tmpl).unwrap();
            assert!(result, "expected true for: {tmpl}");
        }
    }

    #[test]
    fn render_bool_false_for_other() {
        let sm = Arc::new(StateMachine::new());
        let env = TemplateEnvironment::new(sm);
        assert!(!env.render_bool("false").unwrap());
        assert!(!env.render_bool("0").unwrap());
        assert!(!env.render_bool("off").unwrap());
    }

    // ── HC-SEC-01: template DoS is bounded by fuel ─────────────────────
    //
    // A `template:` condition is user config. Before the fuel bound a
    // nested-loop template rendered a multi-GB string over ~11 s (proven
    // empirically). With fuel enabled it must fail FAST with an error
    // instead of expanding unboundedly. On the pre-fix code (no `fuel`
    // feature / `set_fuel`) this render succeeds and burns CPU+RAM, so
    // this test fails on old (it would `Ok` and exceed the time bound).
    #[test]
    fn nested_loop_template_is_bounded_not_unbounded_dos() {
        use std::time::Instant;
        let sm = Arc::new(StateMachine::new());
        let env = TemplateEnvironment::new(sm);
        // 5000 * 5000 = 25M iterations on the old engine (~100 MB, ~11 s).
        let malicious =
            "{% for i in range(5000) %}{% for j in range(5000) %}xxxx{% endfor %}{% endfor %}";
        let start = Instant::now();
        let result = env.render(malicious);
        let elapsed = start.elapsed();
        assert!(
            result.is_err(),
            "malicious nested-loop template must be rejected (ran out of fuel), got Ok"
        );
        assert!(
            elapsed.as_secs() < 3,
            "bounded render must fail fast; took {elapsed:?} (unbounded DoS on old engine)"
        );
    }

    // ── HC-SEC-01: a single huge repeat is also bounded ────────────────
    #[test]
    fn single_huge_repeat_template_is_bounded() {
        let sm = Arc::new(StateMachine::new());
        let env = TemplateEnvironment::new(sm);
        // range() caps at 10k per call, but multiplied bodies still need a
        // bound; drive enough instructions to exhaust fuel via deep nesting.
        let malicious = "{% for a in range(9999) %}{% for b in range(9999) %}\
            {% for c in range(9999) %}z{% endfor %}{% endfor %}{% endfor %}";
        let result = env.render(malicious);
        assert!(result.is_err(), "deeply nested loops must exhaust fuel and error");
    }

    // ── HC-SEC-01: oversized template source is rejected pre-compile ───
    #[test]
    fn oversized_template_source_is_rejected() {
        let sm = Arc::new(StateMachine::new());
        let env = TemplateEnvironment::new(sm);
        // 128 KiB of literal text — exceeds MAX_TEMPLATE_SOURCE_BYTES.
        let big = "x".repeat(128 * 1024);
        let result = env.render(&big);
        assert!(result.is_err(), "oversized template source must be rejected");
    }

    // ── A legitimate small template still renders fine within budget ───
    #[test]
    fn legitimate_template_still_renders_within_fuel() {
        let sm = sm_with("light.kitchen", "on", serde_json::json!({}));
        let env = TemplateEnvironment::new(sm);
        // A normal HA condition with a modest loop — well under budget.
        let ok = "{% for i in range(50) %}{{ states('light.kitchen') }}{% endfor %}";
        let out = env.render(ok).expect("legitimate template must render");
        assert!(out.contains("on"));
    }
}
