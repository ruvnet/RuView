/**
 * @file link_watchdog_core.c
 * @brief ADR-427 link-rate watchdog -- pure decision logic. See the header
 *        for the module boundary and why it has no ESP-IDF includes.
 */
#include "link_watchdog_core.h"
#include <string.h>

void lwd_config_defaults(lwd_config_t *cfg)
{
    memset(cfg, 0, sizeof(*cfg));
    cfg->window_s               = 60;
    cfg->k                      = 3;
    cfg->abs_ms                 = 40;
    cfg->baseline_floor_ms      = 5;
    cfg->outage_loss_pct        = 50;
    cfg->healthy_loss_pct       = 5;
    cfg->n_reassoc              = 10;
    cfg->n_reboot               = 10;
    cfg->grace_windows          = 10;
    cfg->reassoc_min_period_s   = 30u * 60u;
    cfg->reboot_min_period_s    = 6u * 3600u;
    cfg->reboot_backoff_max_mult = 8;   /* 6h -> 12h -> 24h -> 48h, then caps */
    cfg->boot_grace_s           = 15u * 60u;
    cfg->max_jitter_windows     = 5;    /* 0-5 min at the default 60s window */
}

void lwd_core_init(lwd_core_t *core, const lwd_config_t *cfg, uint8_t mode,
                   const lwd_persist_t *persisted)
{
    memset(core, 0, sizeof(*core));
    core->cfg = *cfg;
    core->mode = mode;
    core->phase = LWD_PHASE_NORMAL;
    if (persisted) {
        core->persist = *persisted;
    }
    if (core->persist.reboot_backoff_mult == 0) {
        core->persist.reboot_backoff_mult = 1;
    }
}

lwd_window_class_t lwd_classify(const lwd_core_t *core,
                                const lwd_window_sample_t *w,
                                uint32_t *out_loss_pct)
{
    uint32_t loss_pct;
    if (w->ping_total == 0) {
        /* No samples this window at all -- cannot distinguish "healthy and
         * quiet" from "collapsed so hard nothing came back". Treat it the
         * safe way: as an outage, which suspends the watchdog rather than
         * accumulating a streak on no evidence. */
        loss_pct = 100;
    } else {
        uint32_t fails = (w->ping_success > w->ping_total)
                         ? 0 : (w->ping_total - w->ping_success);
        loss_pct = (uint32_t)(((uint64_t)fails * 100u) / w->ping_total);
    }
    if (out_loss_pct) {
        *out_loss_pct = loss_pct;
    }

    if (!w->associated || w->ping_total == 0 ||
        loss_pct >= core->cfg.outage_loss_pct) {
        return LWD_WINDOW_OUTAGE;
    }

    /* Before the baseline has ever been seeded, fall back to the floor as
     * the most conservative stand-in: it is the smallest baseline the
     * learned value is ever allowed to reach, so using it up front cannot
     * be more sensitive than the learned baseline will eventually be. */
    uint32_t effective_baseline = core->baseline_seeded
        ? (core->baseline_ms > core->cfg.baseline_floor_ms
               ? core->baseline_ms : core->cfg.baseline_floor_ms)
        : core->cfg.baseline_floor_ms;

    uint64_t k_threshold = (uint64_t)core->cfg.k * effective_baseline;
    uint32_t threshold = (uint32_t)(k_threshold > core->cfg.abs_ms
                                    ? k_threshold : core->cfg.abs_ms);

    return (w->median_rtt_ms > threshold) ? LWD_WINDOW_DEGRADED
                                          : LWD_WINDOW_HEALTHY;
}

static void lwd_baseline_update(lwd_core_t *core, uint32_t sample_ms)
{
    if (!core->baseline_seeded) {
        core->baseline_ms = sample_ms;
        core->baseline_seeded = true;
    } else {
        /* EMA, alpha = 1/8 -- same shift this codebase already uses for the
         * ESP-NOW mesh offset (OFFSET_EMA_SHIFT, c6_sync_espnow.c): cheap
         * integer math, no float, no heap. */
        int32_t delta = (int32_t)sample_ms - (int32_t)core->baseline_ms;
        core->baseline_ms = (uint32_t)((int32_t)core->baseline_ms + delta / 8);
    }
    if (core->baseline_ms < core->cfg.baseline_floor_ms) {
        core->baseline_ms = core->cfg.baseline_floor_ms;
    }
}

lwd_action_t lwd_core_tick(lwd_core_t *core,
                          const lwd_window_sample_t *w,
                          const lwd_inhibits_t *inhibits,
                          const lwd_quorum_t *quorum,
                          uint64_t now_lifetime_s,
                          uint32_t jitter_draw)
{
    core->persist.lifetime_s = now_lifetime_s;

    /* A reboot is "cured" once a full (possibly backed-off) rate-limit
     * period passes without needing another one. This runs every window,
     * independent of this window's classification, so the cure clock keeps
     * ticking even through windows this function returns early for. */
    if (core->persist.reboot_cure_pending &&
        now_lifetime_s >= core->persist.next_reboot_allowed_s) {
        core->persist.reboot_cure_pending = false;
        core->persist.reboot_backoff_mult = 1;
    }

    uint32_t loss_pct = 0;
    lwd_window_class_t cls = lwd_classify(core, w, &loss_pct);

    core->last_class     = cls;
    core->last_median_ms = w->median_rtt_ms;
    core->last_p90_ms    = w->p90_rtt_ms;
    core->last_loss_pct  = loss_pct;
    core->last_inhibit   = LWD_INHIBIT_NONE;
    core->last_action    = LWD_ACTION_NONE;

    /* Baseline learns on a healthy window only, and only below the tighter
     * `healthy_loss_pct` bar -- never while degraded, never during an
     * outage. ADR-427 is explicit about this; test_link_watchdog.c's
     * negative control breaks exactly this line to prove the test catches
     * it. */
    if (cls == LWD_WINDOW_HEALTHY && loss_pct < core->cfg.healthy_loss_pct) {
        lwd_baseline_update(core, w->median_rtt_ms);
    }

    if (core->mode == LWD_MODE_OFF) {
        /* "0 off" means off, including the shared peer-degraded bit --
         * lwd_core_is_degraded() reads last_class, so this must not leave a
         * DEGRADED/OUTAGE classification behind for it to see. */
        core->last_class = LWD_WINDOW_HEALTHY;
        return LWD_ACTION_NONE;
    }

    if (cls == LWD_WINDOW_OUTAGE) {
        /* Guard 1: a whole-AP or channel-wide event ends in a disconnect or
         * >=50% loss for everyone, which must never look like "my link
         * collapsed". Reset the streak and abandon anything pending. */
        core->degraded_streak = 0;
        core->phase = LWD_PHASE_NORMAL;
        return LWD_ACTION_NONE;
    }

    if (core->in_grace) {
        /* Degraded-window counting is suspended for the whole grace, win or
         * lose, so the just-reassociated link gets a clean 10 minutes to
         * reprove itself (and re-seed the baseline -- see the healthy-window
         * branch above, which still runs before this check). */
        if (core->grace_windows_left > 0) {
            core->grace_windows_left--;
        }
        if (core->grace_windows_left == 0) {
            core->in_grace = false;
            core->reboot_eligible = true;
        }
        return LWD_ACTION_NONE;
    }

    if (cls == LWD_WINDOW_HEALTHY) {
        core->degraded_streak = 0;
        if (core->phase == LWD_PHASE_NORMAL) {
            return LWD_ACTION_NONE;
        }
        /* Mid-jitter/mid-hold with a merely-healthy sample: do NOT cancel.
         * One good window during the 0-5 min guard-3 delay or a quorum/rate
         * hold would make those guards meaningless. Only an outage (above)
         * cancels a pending action. Fall through to the pending-phase
         * handling below. */
    } else {
        /* DEGRADED. */
        if (core->phase == LWD_PHASE_NORMAL) {
            core->degraded_streak++;
        }
    }

    if (core->phase == LWD_PHASE_NORMAL) {
        /* The ladder is a one-way ratchet per boot: before the first
         * reassociate+grace this boot, a degraded streak counts toward
         * n_reassoc. After it (reboot_eligible), the SAME kind of streak
         * instead counts toward n_reboot -- reassociate was already tried
         * and did not hold, so the next threshold escalates straight to the
         * reboot tier rather than re-triggering another reassociate at the
         * same count. A real reboot resets reboot_eligible to false via
         * process restart; observe mode's simulated reset (below) leaves it
         * true, since nothing there needed re-trying reassociate first. */
        if (core->reboot_eligible) {
            if (core->degraded_streak >= core->cfg.n_reboot) {
                core->phase = LWD_PHASE_PENDING_REBOOT;
                core->jitter_windows_left =
                    jitter_draw % (core->cfg.max_jitter_windows + 1u);
            } else {
                return LWD_ACTION_NONE;
            }
        } else {
            if (core->degraded_streak >= core->cfg.n_reassoc) {
                core->phase = LWD_PHASE_PENDING_REASSOC;
                core->jitter_windows_left =
                    jitter_draw % (core->cfg.max_jitter_windows + 1u);
            } else {
                return LWD_ACTION_NONE;
            }
        }
    }

    /* phase is now PENDING_REASSOC or PENDING_REBOOT, either just entered
     * above or carried over from an earlier window still waiting on jitter,
     * quorum, a rate limit, or an inhibit. Guard 3 first: wait out the
     * random delay, THEN re-check quorum (ADR-427's own ordering). */
    if (core->jitter_windows_left > 0) {
        core->jitter_windows_left--;
        core->last_inhibit = LWD_INHIBIT_JITTER;
        return LWD_ACTION_NONE;
    }

    /* Guard 2: peer quorum hold. No data (valid_peers == 0) cannot hold
     * anything -- see the header doc on lwd_quorum_t. */
    if (quorum->valid_peers > 0 &&
        quorum->degraded_peers * 2 >= quorum->valid_peers) {
        core->last_inhibit = LWD_INHIBIT_QUORUM_HOLD;
        return LWD_ACTION_NONE;
    }

    bool is_reassoc = (core->phase == LWD_PHASE_PENDING_REASSOC);
    uint64_t allowed_at = is_reassoc ? core->persist.next_reassoc_allowed_s
                                     : core->persist.next_reboot_allowed_s;
    if (now_lifetime_s < allowed_at) {
        core->last_inhibit = LWD_INHIBIT_RATE_LIMITED;
        /* "After the limit the watchdog only logs" (ADR-427): stop trying
         * this cycle instead of re-checking, and re-logging, every window
         * until the limit expires. The ladder must re-earn n_reassoc /
         * n_reboot degraded windows from scratch before trying again. */
        core->phase = LWD_PHASE_NORMAL;
        core->degraded_streak = 0;
        return LWD_ACTION_NONE;
    }

    if (inhibits->ota_pending_verify) {
        core->last_inhibit = LWD_INHIBIT_PENDING_VERIFY;
        return LWD_ACTION_NONE;
    }
    if (inhibits->config_trial_active) {
        core->last_inhibit = LWD_INHIBIT_CONFIG_TRIAL;
        return LWD_ACTION_NONE;
    }
    if (inhibits->within_boot_grace) {
        core->last_inhibit = LWD_INHIBIT_BOOT_GRACE;
        return LWD_ACTION_NONE;
    }
    if (inhibits->within_disconnect_grace) {
        core->last_inhibit = LWD_INHIBIT_DISCONNECT_GRACE;
        return LWD_ACTION_NONE;
    }

    /* Every guard cleared: fire. */
    lwd_action_t real_action  = is_reassoc ? LWD_ACTION_REASSOC : LWD_ACTION_REBOOT;
    lwd_action_t would_action = is_reassoc ? LWD_ACTION_WOULD_REASSOC
                                           : LWD_ACTION_WOULD_REBOOT;
    lwd_action_t fired = (core->mode == LWD_MODE_ACT) ? real_action : would_action;

    if (is_reassoc) {
        core->persist.next_reassoc_allowed_s =
            now_lifetime_s + core->cfg.reassoc_min_period_s;
        core->in_grace = true;
        core->grace_windows_left = core->cfg.grace_windows;
    } else {
        if (core->persist.reboot_cure_pending) {
            /* Still inside the previous reboot's cure window and we are
             * about to reboot again: that reboot did not cure it. Double
             * the backoff (ADR-427), capped. */
            uint32_t next = core->persist.reboot_backoff_mult * 2u;
            core->persist.reboot_backoff_mult =
                (next > core->cfg.reboot_backoff_max_mult)
                    ? core->cfg.reboot_backoff_max_mult : next;
        }
        uint64_t period = (uint64_t)core->cfg.reboot_min_period_s *
                          core->persist.reboot_backoff_mult;
        core->persist.next_reboot_allowed_s = now_lifetime_s + period;
        core->persist.reboot_cure_pending = true;

        /* A real reboot resets baseline/grace state for free via process
         * restart. Observe mode never actually reboots, so simulate that
         * reset explicitly -- otherwise repeated WOULD_REBOOT logging in
         * phase-1 validation would stop meaning anything after the first
         * one. This cannot simulate the glue layer's own boot/disconnect
         * timers, which only a real reboot resets; that is a documented,
         * accepted divergence (see link_watchdog.c). */
        core->in_grace = true;
        core->grace_windows_left = core->cfg.grace_windows;
        if (core->mode != LWD_MODE_ACT) {
            core->baseline_seeded = false;
        }
    }

    core->phase = LWD_PHASE_NORMAL;
    core->degraded_streak = 0;
    core->last_action = fired;
    core->last_action_lifetime_s = now_lifetime_s;
    if (fired == real_action) {
        core->actions_taken++;
    } else {
        core->would_actions_taken++;
    }
    return fired;
}

bool lwd_core_is_degraded(const lwd_core_t *core)
{
    return core->mode != LWD_MODE_OFF && core->last_class == LWD_WINDOW_DEGRADED;
}
