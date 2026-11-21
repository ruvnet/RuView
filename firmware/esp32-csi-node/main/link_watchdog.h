/**
 * @file link_watchdog.h
 * @brief ADR-427 link-rate watchdog -- ESP-IDF glue around link_watchdog_core.h.
 *
 * Owns everything the pure core (link_watchdog_core.h) deliberately does not:
 * reading the esp_ping RTTs, NVS persistence of the rate-limit state across a
 * reboot, the ESP-NOW peer table behind the quorum hold, calling
 * esp_wifi_disconnect() / esp_restart(), node_log, and the /config "active"
 * report.
 */
#pragma once

#include <stdbool.h>
#include <stdint.h>

#include "esp_err.h"
#include "ping/ping_sock.h"   /* esp_ping_handle_t for the callbacks below */

#ifdef __cplusplus
extern "C" {
#endif

/** Call once at boot, after NVS and g_nvs_config are loaded. Reads the
 *  `linkwd` NVS mode and the persisted rate-limit state. Safe to call even
 *  when CONFIG_LINK_WATCHDOG is off (becomes a no-op). */
void link_watchdog_init(void);

/** Call from the main loop roughly every 10 s (matches the cadence
 *  node_log_periodic() already uses). Internally batches ping samples into
 *  CONFIG_LINK_WATCHDOG_WINDOW_S windows and runs the ladder once per
 *  completed window. No-op in NVS mode 0 (off) or when the Kconfig symbol
 *  is not set. */
void link_watchdog_tick(void);

/** esp_ping callbacks -- wire these in as csi_collector.c's
 *  on_ping_success/on_ping_timeout in place of csi_ping_cb_noop. Reads
 *  ESP_PING_PROF_TIMEGAP for a success; a timeout carries no RTT. Both are
 *  cheap counter updates, safe at CONFIG_CSI_SELF_PING_HZ. */
void link_watchdog_ping_success_cb(esp_ping_handle_t hdl, void *args);
void link_watchdog_ping_timeout_cb(esp_ping_handle_t hdl, void *args);

/** True when this node's own most recently completed window was classified
 *  DEGRADED -- independent of NVS mode 1 vs 2, so an observe-mode node would
 *  still contribute real signal to its peers' quorum. Always false in mode 0
 *  or when the feature is compiled out.
 *
 *  NOT YET ADVERTISED: the ESP-NOW sync beacon (c6_sync_espnow.c, proto v1)
 *  has no field for it. Carrying it is a beacon-format change and is left to
 *  a separate change; see link_watchdog_note_peer(). */
bool link_watchdog_is_degraded(void);

/** Feed one received peer beacon into the peer table behind guard 2 (quorum
 *  hold).
 *
 *  NO CALLER IN THIS BUILD: nothing receives a peer's degraded bit yet, so
 *  the table stays empty, valid_peers is 0, and guard 2 never holds (see
 *  lwd_quorum_t). Guards 1 (outage is not degradation) and 3 (random delay),
 *  the inhibits and the rate limits still apply. Act mode (NVS linkwd=2)
 *  therefore runs WITHOUT the peer-quorum guard until a beacon feed exists.
 *
 *  @param mac       sender's 6-byte MAC (ESP-NOW src_addr)
 *  @param now_us    esp_timer_get_time() at reception
 *  @param degraded  that peer's degraded bit */
void link_watchdog_note_peer(const uint8_t mac[6], uint64_t now_us, bool degraded);

/** Call from main.c's WIFI_EVENT_STA_DISCONNECTED handler, after
 *  node_log_note_disconnect(). Restarts the post-disconnect inhibit. */
void link_watchdog_note_wifi_disconnect(void);

/** Snapshot for GET /config "active"."link_wd" and for a human reading logs.
 *  Field names deliberately match the ADR-427 Configuration section. */
typedef struct {
    uint8_t  mode;                 /* 0 off, 1 observe, 2 act */
    uint32_t baseline_ms;
    bool     baseline_seeded;
    uint32_t last_median_ms;
    uint32_t last_p90_ms;
    uint32_t last_loss_pct;
    uint32_t degraded_streak;
    bool     in_grace;
    uint32_t actions_taken;        /* real reassoc+reboot, this boot */
    uint32_t would_actions_taken;  /* observe-mode would-act, this boot */
    uint8_t  last_action;          /* lwd_action_t */
    uint64_t last_action_lifetime_s;
    uint8_t  last_inhibit;         /* lwd_inhibit_t */
    uint32_t peers_valid;
    uint32_t peers_degraded;
} link_watchdog_report_t;

/** Fill `out` with the current state. Zeroes it and returns if the feature
 *  is compiled out or NVS mode is 0. */
void link_watchdog_get_report(link_watchdog_report_t *out);

/** Human-readable names for the report's `last_action` / `last_inhibit`
 *  codes (lwd_action_t / lwd_inhibit_t), for /config and log lines. Never
 *  returns NULL. */
const char *link_watchdog_action_name(uint8_t action);
const char *link_watchdog_inhibit_name(uint8_t inhibit);

#ifdef __cplusplus
}
#endif
