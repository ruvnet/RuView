/**
 * @file link_watchdog_core.h
 * @brief ADR-427 link-rate watchdog -- pure decision logic, no ESP-IDF.
 *
 * This module is the part of ADR-427 that has to be provably correct:
 * windowing, baseline learning, degraded/outage classification, the
 * reassociate-then-reboot ladder, rate limiting that survives a reboot loop,
 * the peer-quorum hold, and the three guards against fleet-wide action.
 *
 * It has no I/O, no clock of its own, and no FreeRTOS/ESP-IDF includes --
 * every external fact (window stats, inhibits, quorum snapshot, a lifetime
 * uptime counter, a random draw for jitter) is passed in by the caller, and
 * every decision comes back as a return value. That split is what lets
 * tests/host/test_link_watchdog.c run the whole ladder on a desktop with
 * plain gcc, the same pattern already used for adaptive_controller_decide()
 * and rv_mesh.c in this codebase.
 *
 * The ESP-IDF glue (link_watchdog.c) owns: reading the ping RTTs, reading
 * and persisting the NVS-backed state this module needs across a reboot,
 * calling esp_wifi_disconnect()/esp_restart(), and writing node_log /
 * /config reports from the state this module exposes.
 */
#pragma once

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* NVS key `linkwd`: 0 off (detector does not run at all), 1 observe (detect
 * and log "would act", never act), 2 act. Defaults to 1 (ADR-427 Validation
 * phase 1): act mode is a deliberate per-node /config push. */
#define LWD_MODE_OFF     0
#define LWD_MODE_OBSERVE 1
#define LWD_MODE_ACT     2

typedef enum {
    LWD_ACTION_NONE = 0,
    /* mode < ACT: the ladder reached the point of acting but did not. */
    LWD_ACTION_WOULD_REASSOC,
    LWD_ACTION_WOULD_REBOOT,
    /* mode == ACT: the caller must now call esp_wifi_disconnect() /
     * esp_restart() respectively. */
    LWD_ACTION_REASSOC,
    LWD_ACTION_REBOOT,
} lwd_action_t;

/* Why an action that the ladder otherwise wanted to take did not fire this
 * window. NONE when no action was pending. Logged so a human reading
 * node_log can tell "nothing was wrong" from "something was wrong and the
 * fleet-safety guards correctly held it back". */
typedef enum {
    LWD_INHIBIT_NONE = 0,
    LWD_INHIBIT_PENDING_VERIFY,
    LWD_INHIBIT_CONFIG_TRIAL,
    LWD_INHIBIT_BOOT_GRACE,
    LWD_INHIBIT_DISCONNECT_GRACE,
    LWD_INHIBIT_QUORUM_HOLD,     /* guard 2 */
    LWD_INHIBIT_RATE_LIMITED,
    LWD_INHIBIT_JITTER,          /* guard 3: still waiting out the 0-5 min roll */
} lwd_inhibit_t;

/* A window's classification. OUTAGE is deliberately not a degree of
 * DEGRADED -- guard 1 depends on them being mutually exclusive categories
 * that a node can tell apart from its own counters alone. */
typedef enum {
    LWD_WINDOW_HEALTHY = 0,
    LWD_WINDOW_DEGRADED,
    LWD_WINDOW_OUTAGE,
} lwd_window_class_t;

/* One window's raw measurement, as the esp_ping callbacks accumulate it.
 * Counts, not rates -- the core does the division so every caller (firmware
 * and host tests) does it the same way. */
typedef struct {
    uint32_t median_rtt_ms;
    uint32_t p90_rtt_ms;
    uint32_t ping_total;      /* successes + timeouts this window */
    uint32_t ping_success;
    bool     associated;      /* esp_wifi_sta_get_ap_info() == ESP_OK */
} lwd_window_sample_t;

/* Reasons an action must not fire even though the ladder reached threshold.
 * All four can be true at once; the core reports the highest-priority one
 * (see lwd_core_tick doc). */
typedef struct {
    bool ota_pending_verify;   /* a reboot here would roll the image back */
    bool config_trial_active;
    bool within_boot_grace;        /* < boot_grace_s since this boot */
    bool within_disconnect_grace;  /* < boot_grace_s since the last disconnect */
} lwd_inhibits_t;

/* Guard 2: what this node heard from its peers' ESP-NOW sync beacons in the
 * last 5 minutes. valid_peers == 0 (a 1-2 node bench mesh, a mesh that has
 * not finished booting, or a build with no beacon feed wired -- see
 * link_watchdog_note_peer()) is defined as "no quorum data": the guard
 * cannot hold what it cannot see, so it does not hold. */
typedef struct {
    uint32_t valid_peers;
    uint32_t degraded_peers;
} lwd_quorum_t;

typedef struct {
    uint32_t window_s;             /* default 60 */
    uint32_t k;                    /* default 3 */
    uint32_t abs_ms;                /* default 40 */
    uint32_t baseline_floor_ms;     /* default 5 */
    uint32_t outage_loss_pct;       /* default 50 */
    uint32_t healthy_loss_pct;      /* default 5 -- baseline learns below this */
    uint32_t n_reassoc;             /* default 10 consecutive degraded windows */
    uint32_t n_reboot;              /* default 10, counted AFTER the grace */
    uint32_t grace_windows;         /* default 10 (10 min re-learn grace) */
    uint32_t reassoc_min_period_s;  /* default 1800 (30 min) */
    uint32_t reboot_min_period_s;   /* default 21600 (6 h), doubles per ADR */
    uint32_t reboot_backoff_max_mult; /* cap on the doubling, default 8 (48h) */
    uint32_t boot_grace_s;          /* default 900 (15 min) */
    uint32_t max_jitter_windows;    /* default 5 (0-5 min at a 60s window) */
} lwd_config_t;

/* Fields that must survive a reboot so a crash/reboot loop cannot reset the
 * rate limits (ADR-427: "persisted in NVS"). The glue layer loads this from
 * NVS before the first tick and persists it back after every tick in which
 * any field changed -- lwd_core_tick() never mutates it in place; it reads
 * from *inout and writes the (possibly unchanged) new value back so the
 * caller can diff-and-skip the NVS write cheaply. */
typedef struct {
    uint64_t lifetime_s;            /* monotonic across reboots -- see .c */
    uint64_t next_reassoc_allowed_s;
    uint64_t next_reboot_allowed_s;
    uint32_t reboot_backoff_mult;   /* starts at 1, doubles, capped */
    bool     reboot_cure_pending;   /* true from a reboot action until the
                                      * next reboot_min_period_s passes clean */
} lwd_persist_t;

typedef struct {
    lwd_config_t cfg;
    uint8_t      mode;              /* LWD_MODE_* */

    uint32_t baseline_ms;
    bool     baseline_seeded;

    uint32_t degraded_streak;
    bool     in_grace;
    uint32_t grace_windows_left;
    bool     reboot_eligible;       /* a grace has completed at least once
                                      * since boot -- reboot can only follow
                                      * a reassociate, never fire first */

    enum {
        LWD_PHASE_NORMAL = 0,
        LWD_PHASE_PENDING_REASSOC,
        LWD_PHASE_PENDING_REBOOT,
    } phase;
    uint32_t jitter_windows_left;

    lwd_persist_t persist;

    /* Last-window snapshot, for /config and node_log reporting only -- never
     * read back by the logic itself. */
    lwd_window_class_t last_class;
    uint32_t            last_median_ms;
    uint32_t            last_p90_ms;
    uint32_t            last_loss_pct;
    lwd_inhibit_t        last_inhibit;
    lwd_action_t          last_action;
    uint64_t              last_action_lifetime_s;
    uint32_t              actions_taken;      /* real REASSOC+REBOOT, this boot */
    uint32_t              would_actions_taken; /* WOULD_* , this boot */
} lwd_core_t;

/** Fill `cfg` with the ADR-427 defaults. */
void lwd_config_defaults(lwd_config_t *cfg);

/** Zero-initialize a core with these config/mode/persisted values. Call once
 *  at boot after NVS is read. */
void lwd_core_init(lwd_core_t *core, const lwd_config_t *cfg, uint8_t mode,
                   const lwd_persist_t *persisted);

/**
 * Classify one completed window. Exposed separately from lwd_core_tick() so
 * tests (and the /config reporter) can classify without mutating ladder
 * state.
 */
lwd_window_class_t lwd_classify(const lwd_core_t *core,
                                const lwd_window_sample_t *w,
                                uint32_t *out_loss_pct);

/**
 * Advance the ladder by exactly one window. `now_lifetime_s` is the node's
 * total uptime across every boot since NVS was last erased (see
 * link_watchdog.c for how the glue layer maintains it) -- rate limits are
 * expressed in this unit specifically so a reboot cannot rewind them.
 * `jitter_draw` is only consumed the instant the ladder enters a PENDING_*
 * phase; pass esp_random() in firmware and a fixed or PRNG value in tests.
 *
 * Returns the action for THIS window. The inhibit that blocked an
 * otherwise-ready action, if any, is left in core->last_inhibit.
 */
lwd_action_t lwd_core_tick(lwd_core_t *core,
                          const lwd_window_sample_t *w,
                          const lwd_inhibits_t *inhibits,
                          const lwd_quorum_t *quorum,
                          uint64_t now_lifetime_s,
                          uint32_t jitter_draw);

/** True when the most recently classified window was DEGRADED (not OUTAGE,
 *  not HEALTHY). This is the bit a node puts on its own ESP-NOW sync beacon
 *  for its peers' quorum math -- deliberately independent of mode and of
 *  the ladder/rate-limit state, so an observe-mode node still contributes
 *  real signal to its peers. */
bool lwd_core_is_degraded(const lwd_core_t *core);

#ifdef __cplusplus
}
#endif
