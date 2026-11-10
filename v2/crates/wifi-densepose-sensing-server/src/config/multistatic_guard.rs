//! Multistatic guard configuration derived from environment variables (ADR-1031, ADR-1049).

use wifi_densepose_signal::ruvsense::multistatic::MultistaticConfig;

/// Build the multistatic guard config from the environment (#1031, #1049).
///
/// Three precedence layers, most-specific wins:
/// 1. `WDP_GUARD_INTERVAL_US` (+ optional `WDP_SOFT_GUARD_US`) — a **direct**
///    hard-guard override.
/// 2. `WDP_TDM_SLOTS` + `WDP_TDM_SLOT_US` (both positive) — derive the guard
///    from the declared schedule.
/// 3. Otherwise the published default (60 ms hard / 20 ms soft).
pub fn multistatic_guard_config_from_env() -> MultistaticConfig {
    multistatic_guard_config_from(
        std::env::var("WDP_TDM_SLOTS").ok().as_deref(),
        std::env::var("WDP_TDM_SLOT_US").ok().as_deref(),
        std::env::var("WDP_GUARD_INTERVAL_US").ok().as_deref(),
        std::env::var("WDP_SOFT_GUARD_US").ok().as_deref(),
    )
}

/// Pure core of [`multistatic_guard_config_from_env`] for testability.
pub fn multistatic_guard_config_from(
    slots: Option<&str>,
    slot_us: Option<&str>,
    guard_us: Option<&str>,
    soft_us: Option<&str>,
) -> MultistaticConfig {
    let mut cfg = match (
        slots.and_then(|s| s.trim().parse::<usize>().ok()),
        slot_us.and_then(|s| s.trim().parse::<u64>().ok()),
    ) {
        (Some(n), Some(us)) if n >= 1 && us >= 1 => MultistaticConfig::for_tdm_schedule(n, us),
        _ => MultistaticConfig::default(),
    };

    if let Some(g) = guard_us
        .and_then(|s| s.trim().parse::<u64>().ok())
        .filter(|&g| g >= 1)
    {
        cfg.guard_interval_us = g;
        if cfg.soft_guard_us >= g {
            cfg.soft_guard_us = g.saturating_sub(1).max(1);
        }
    }

    if let Some(s) = soft_us
        .and_then(|s| s.trim().parse::<u64>().ok())
        .filter(|&s| s >= 1)
    {
        cfg.soft_guard_us = s.min(cfg.guard_interval_us.saturating_sub(1).max(1));
    }

    cfg
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_when_env_unset() {
        let cfg = multistatic_guard_config_from(None, None, None, None);
        assert_eq!(cfg.guard_interval_us, 60_000);
        assert_eq!(cfg.soft_guard_us, 20_000);
    }

    #[test]
    fn direct_override_lifts_hard_guard_and_preserves_default_soft() {
        let cfg = multistatic_guard_config_from(None, None, Some("200000"), None);
        assert_eq!(cfg.guard_interval_us, 200_000);
        assert_eq!(cfg.soft_guard_us, 20_000);
    }

    #[test]
    fn direct_override_composes_with_tdm_schedule() {
        let cfg = multistatic_guard_config_from(Some("2"), Some("18000"), Some("200000"), None);
        assert_eq!(cfg.guard_interval_us, 200_000);
        assert_eq!(cfg.soft_guard_us, 21_600);
    }

    #[test]
    fn soft_guard_clamped_below_hard_guard() {
        let cfg = multistatic_guard_config_from(None, None, Some("50000"), Some("999999"));
        assert_eq!(cfg.guard_interval_us, 50_000);
        assert_eq!(cfg.soft_guard_us, 49_999);
    }

    #[test]
    fn lowering_hard_guard_drags_soft_down() {
        let cfg = multistatic_guard_config_from(None, None, Some("10000"), None);
        assert_eq!(cfg.guard_interval_us, 10_000);
        assert_eq!(cfg.soft_guard_us, 9_999);
    }

    #[test]
    fn unparseable_or_zero_falls_back_safely() {
        for bad in ["", "0", "-1", "foo", " 0 "] {
            let cfg = multistatic_guard_config_from(None, None, Some(bad), None);
            assert_eq!(cfg.guard_interval_us, 60_000);
            assert_eq!(cfg.soft_guard_us, 20_000);
        }
    }
}

