/**
 * @file link_watchdog.c
 * @brief ADR-427 link-rate watchdog -- ESP-IDF glue. See link_watchdog.h and
 *        link_watchdog_core.h (the pure decision logic this file drives).
 */
#include "sdkconfig.h"
#include "link_watchdog.h"
#include "link_watchdog_core.h"

#include <string.h>
#include <inttypes.h>

/* Name lookups are pure enum->string and have no IDF dependency, so they are
 * available unconditionally -- config_api.c's /config report calls them
 * regardless of whether CONFIG_LINK_WATCHDOG is set (the report is all
 * zeros/"none" in that case, not a missing symbol). */
const char *link_watchdog_action_name(uint8_t action)
{
    switch ((lwd_action_t)action) {
        case LWD_ACTION_NONE:          return "none";
        case LWD_ACTION_WOULD_REASSOC: return "would_reassoc";
        case LWD_ACTION_WOULD_REBOOT:  return "would_reboot";
        case LWD_ACTION_REASSOC:       return "reassoc";
        case LWD_ACTION_REBOOT:        return "reboot";
        default:                       return "unknown";
    }
}

const char *link_watchdog_inhibit_name(uint8_t inhibit)
{
    switch ((lwd_inhibit_t)inhibit) {
        case LWD_INHIBIT_NONE:             return "none";
        case LWD_INHIBIT_PENDING_VERIFY:   return "ota_pending_verify";
        case LWD_INHIBIT_CONFIG_TRIAL:     return "config_trial";
        case LWD_INHIBIT_BOOT_GRACE:       return "boot_grace";
        case LWD_INHIBIT_DISCONNECT_GRACE: return "disconnect_grace";
        case LWD_INHIBIT_QUORUM_HOLD:      return "quorum_hold";
        case LWD_INHIBIT_RATE_LIMITED:     return "rate_limited";
        case LWD_INHIBIT_JITTER:           return "jitter";
        default:                           return "unknown";
    }
}

#ifdef CONFIG_LINK_WATCHDOG

#include "nvs_config.h"
#include "nvs_flash.h"
#include "nvs.h"
#include "esp_log.h"
#include "esp_timer.h"
#include "esp_wifi.h"
#include "esp_system.h"
#include "esp_random.h"
#include "node_log.h"
#include "config_api.h"
#include "ota_health.h"
#include "freertos/FreeRTOS.h"

static const char *TAG = "link_wd";

/* Separate from "csi_cfg" (which holds the mode key `linkwd`, see
 * nvs_config.c) so a config wipe of the live config does not also discard
 * months of rate-limit history, and vice versa. */
#define LWD_NVS_NS       "linkwd_st"

/* RTT histogram instead of storing every sample: ADR-427 explicitly calls
 * for "fixed-size histogram, no heap allocation per ping". 4 ms bins to
 * 1024 ms is ample resolution against the default 40 ms abs_ms threshold.
 * Since #2287 the session's reply timeout is CONFIG_CSI_SELF_PING_TIMEOUT_MS
 * (default 200), so no success is slower than that: a slower reply reaches
 * on_ping_timeout and counts toward loss_pct, i.e. OUTAGE once it is most
 * of the window. Keep that timeout well above abs_ms. */
#define LWD_HIST_BIN_MS  4
#define LWD_HIST_BINS    256

#define LWD_MAX_PEERS    16
/* Guard 2's "last 5 minutes". A peer not heard within this is dropped from
 * the quorum entirely rather than counted as degraded -- see
 * link_watchdog_core.h's lwd_quorum_t doc for why. */
#define LWD_PEER_TTL_US  (5ULL * 60 * 1000000ULL)

static lwd_core_t s_core;
static bool       s_inited = false;

static portMUX_TYPE s_hist_mux = portMUX_INITIALIZER_UNLOCKED;
static uint16_t s_hist[LWD_HIST_BINS];
static uint32_t s_ping_success;
static uint32_t s_ping_timeout;

typedef struct {
    uint8_t  mac[6];
    uint64_t last_heard_us;
    bool     degraded;
    bool     used;
} lwd_peer_t;

static portMUX_TYPE s_peer_mux = portMUX_INITIALIZER_UNLOCKED;
static lwd_peer_t s_peers[LWD_MAX_PEERS];

static int64_t  s_window_start_us;
static int64_t  s_boot_us;
static int64_t  s_last_disconnect_us = -1;   /* -1 = no disconnect yet this boot */
static uint64_t s_lifetime_base_s;           /* persisted cumulative uptime before this boot */

/* ------------------------------------------------------- NVS persistence --
 * Rate limits live here, separately from the rest of the ladder state
 * (which resets every boot by design -- see link_watchdog_core.h), so a
 * crash/reboot loop cannot reset them. Written once per completed window,
 * the same cadence main.c already uses for its own "last_up_s" cell. */

static void lwd_persist_load(lwd_persist_t *p, uint64_t *lifetime_base_s)
{
    memset(p, 0, sizeof(*p));
    *lifetime_base_s = 0;
    nvs_handle_t h;
    if (nvs_open(LWD_NVS_NS, NVS_READONLY, &h) != ESP_OK) {
        return;   /* never written before: zeros are the correct "never acted" state */
    }
    nvs_get_u64(h, "lifetime_s", lifetime_base_s);
    nvs_get_u64(h, "next_reassoc", &p->next_reassoc_allowed_s);
    nvs_get_u64(h, "next_reboot", &p->next_reboot_allowed_s);
    uint32_t mult = 0;
    if (nvs_get_u32(h, "reboot_mult", &mult) == ESP_OK && mult > 0) {
        p->reboot_backoff_mult = mult;
    }
    uint8_t cure = 0;
    nvs_get_u8(h, "reboot_cure", &cure);
    p->reboot_cure_pending = (cure != 0);
    nvs_close(h);
}

static void lwd_persist_save(const lwd_persist_t *p, uint64_t lifetime_s)
{
    nvs_handle_t h;
    if (nvs_open(LWD_NVS_NS, NVS_READWRITE, &h) != ESP_OK) {
        ESP_LOGW(TAG, "cannot persist rate-limit state (NVS open failed)");
        return;
    }
    nvs_set_u64(h, "lifetime_s", lifetime_s);
    nvs_set_u64(h, "next_reassoc", p->next_reassoc_allowed_s);
    nvs_set_u64(h, "next_reboot", p->next_reboot_allowed_s);
    nvs_set_u32(h, "reboot_mult", p->reboot_backoff_mult);
    nvs_set_u8(h, "reboot_cure", p->reboot_cure_pending ? 1 : 0);
    nvs_commit(h);
    nvs_close(h);
}

/* ----------------------------------------------------------------- init -- */

void link_watchdog_init(void)
{
    lwd_config_t cfg;
    lwd_config_defaults(&cfg);
    cfg.window_s                = CONFIG_LINK_WATCHDOG_WINDOW_S;
    cfg.k                       = CONFIG_LINK_WATCHDOG_K;
    cfg.abs_ms                  = CONFIG_LINK_WATCHDOG_ABS_MS;
    cfg.baseline_floor_ms       = CONFIG_LINK_WATCHDOG_BASELINE_FLOOR_MS;
    cfg.n_reassoc               = CONFIG_LINK_WATCHDOG_N_REASSOC;
    cfg.n_reboot                = CONFIG_LINK_WATCHDOG_N_REBOOT;
    cfg.grace_windows           = CONFIG_LINK_WATCHDOG_GRACE_WINDOWS;
    cfg.reassoc_min_period_s    = CONFIG_LINK_WATCHDOG_REASSOC_MIN_PERIOD_S;
    cfg.reboot_min_period_s     = CONFIG_LINK_WATCHDOG_REBOOT_MIN_PERIOD_S;
    cfg.reboot_backoff_max_mult = CONFIG_LINK_WATCHDOG_REBOOT_BACKOFF_MAX_MULT;
    cfg.boot_grace_s            = CONFIG_LINK_WATCHDOG_BOOT_GRACE_S;

    lwd_persist_t persisted;
    lwd_persist_load(&persisted, &s_lifetime_base_s);

    lwd_core_init(&s_core, &cfg, g_nvs_config.linkwd_mode, &persisted);

    portENTER_CRITICAL(&s_hist_mux);
    memset(s_hist, 0, sizeof(s_hist));
    s_ping_success = 0;
    s_ping_timeout = 0;
    portEXIT_CRITICAL(&s_hist_mux);

    portENTER_CRITICAL(&s_peer_mux);
    memset(s_peers, 0, sizeof(s_peers));
    portEXIT_CRITICAL(&s_peer_mux);

    s_boot_us = esp_timer_get_time();
    s_window_start_us = s_boot_us;
    s_last_disconnect_us = -1;

    s_inited = true;
    ESP_LOGI(TAG, "ADR-427 link watchdog init: mode=%u window=%" PRIu32 "s "
                  "n_reassoc=%" PRIu32 " n_reboot=%" PRIu32 " lifetime_base=%" PRIu64 "s "
                  "reboot_mult=%" PRIu32 " cure_pending=%d",
             (unsigned)g_nvs_config.linkwd_mode, cfg.window_s, cfg.n_reassoc,
             cfg.n_reboot, s_lifetime_base_s, s_core.persist.reboot_backoff_mult,
             (int)s_core.persist.reboot_cure_pending);
}

/* ---------------------------------------------------------- ping callbacks
 * Replace csi_collector.c's csi_ping_cb_noop for on_ping_success/timeout.
 * Both run on the ping session's own task (not an ISR), at up to
 * CONFIG_CSI_SELF_PING_HZ (default 50) -- a histogram bump and a counter
 * increment, nothing heavier. */

void link_watchdog_ping_success_cb(esp_ping_handle_t hdl, void *args)
{
    (void)args;
    if (!s_inited) return;
    uint32_t elapsed_ms = 0;
    if (esp_ping_get_profile(hdl, ESP_PING_PROF_TIMEGAP, &elapsed_ms,
                             sizeof(elapsed_ms)) != ESP_OK) {
        return;
    }
    uint32_t bin = elapsed_ms / LWD_HIST_BIN_MS;
    if (bin >= LWD_HIST_BINS) bin = LWD_HIST_BINS - 1;
    portENTER_CRITICAL(&s_hist_mux);
    if (s_hist[bin] < UINT16_MAX) s_hist[bin]++;
    s_ping_success++;
    portEXIT_CRITICAL(&s_hist_mux);
}

void link_watchdog_ping_timeout_cb(esp_ping_handle_t hdl, void *args)
{
    (void)hdl;
    (void)args;
    if (!s_inited) return;
    portENTER_CRITICAL(&s_hist_mux);
    s_ping_timeout++;
    portEXIT_CRITICAL(&s_hist_mux);
}

bool link_watchdog_is_degraded(void)
{
    if (!s_inited) return false;
    return lwd_core_is_degraded(&s_core);
}

/* --------------------------------------------------------------- peers --
 * Not fed in this build (see link_watchdog_note_peer() in the header).
 * Fixed-size, no heap: LWD_MAX_PEERS covers the mesh sizes this firmware is
 * run at with headroom (tdm_node_count's own Kconfig range tops out at 64, but
 * ESP-NOW/the 50 Hz receive gate make meshes that large impractical -- see
 * c6_sync_espnow.c's beacon-period derivation). An unknown peer beyond the
 * table evicts the least-recently-heard entry rather than being dropped
 * silently; a stale mesh member falling out of the quorum this way is the
 * same outcome LWD_PEER_TTL_US gives it anyway. */

void link_watchdog_note_peer(const uint8_t mac[6], uint64_t now_us, bool degraded)
{
    if (!s_inited) return;
    portENTER_CRITICAL(&s_peer_mux);
    int slot = -1;
    for (int i = 0; i < LWD_MAX_PEERS; i++) {
        if (s_peers[i].used && memcmp(s_peers[i].mac, mac, 6) == 0) {
            slot = i;
            break;
        }
    }
    if (slot < 0) {
        for (int i = 0; i < LWD_MAX_PEERS; i++) {
            if (!s_peers[i].used) { slot = i; break; }
        }
    }
    if (slot < 0) {
        uint64_t oldest_us = UINT64_MAX;
        for (int i = 0; i < LWD_MAX_PEERS; i++) {
            if (s_peers[i].last_heard_us < oldest_us) {
                oldest_us = s_peers[i].last_heard_us;
                slot = i;
            }
        }
    }
    memcpy(s_peers[slot].mac, mac, 6);
    s_peers[slot].last_heard_us = now_us;
    s_peers[slot].degraded = degraded;
    s_peers[slot].used = true;
    portEXIT_CRITICAL(&s_peer_mux);
}

static void quorum_snapshot(uint64_t now_us, lwd_quorum_t *q)
{
    uint32_t valid = 0, degraded = 0;
    portENTER_CRITICAL(&s_peer_mux);
    for (int i = 0; i < LWD_MAX_PEERS; i++) {
        if (!s_peers[i].used) continue;
        if (now_us - s_peers[i].last_heard_us > LWD_PEER_TTL_US) continue;
        valid++;
        if (s_peers[i].degraded) degraded++;
    }
    portEXIT_CRITICAL(&s_peer_mux);
    q->valid_peers = valid;
    q->degraded_peers = degraded;
}

void link_watchdog_note_wifi_disconnect(void)
{
    if (!s_inited) return;
    s_last_disconnect_us = esp_timer_get_time();
}

/* ------------------------------------------------------ percentiles -----
 * Median and p90 over the histogram of this window's SUCCESSFUL pings only
 * (ESP_PING_PROF_TIMEGAP has no meaning for a timeout) -- matches the ADR's
 * "median and p90 of ESP_PING_PROF_TIMEGAP". */

static void compute_percentiles(const uint16_t *hist, uint32_t success,
                                uint32_t *median_ms, uint32_t *p90_ms)
{
    *median_ms = 0;
    *p90_ms = 0;
    if (success == 0) return;
    uint32_t target_median = (success + 1) / 2;
    uint32_t target_p90 = (success * 90 + 99) / 100;
    if (target_p90 > success) target_p90 = success;
    uint32_t cum = 0;
    bool got_median = false;
    for (int i = 0; i < LWD_HIST_BINS; i++) {
        cum += hist[i];
        if (!got_median && cum >= target_median) {
            *median_ms = (uint32_t)i * LWD_HIST_BIN_MS;
            got_median = true;
        }
        if (cum >= target_p90) {
            *p90_ms = (uint32_t)i * LWD_HIST_BIN_MS;
            return;
        }
    }
    *p90_ms = (LWD_HIST_BINS - 1) * LWD_HIST_BIN_MS;
}

/* ----------------------------------------------------------------- tick -- */

static uint32_t s_win_ping_success;
static bool     s_win_associated;
static bool     s_win_pending;

bool link_watchdog_take_window(uint32_t *ping_success, bool *associated)
{
    if (!s_win_pending) return false;
    s_win_pending = false;
    *ping_success = s_win_ping_success;
    *associated   = s_win_associated;
    return true;
}

void link_watchdog_tick(void)
{
    if (!s_inited) return;

    int64_t now_us = esp_timer_get_time();
    int64_t window_us = (int64_t)s_core.cfg.window_s * 1000000LL;
    if (now_us - s_window_start_us < window_us) {
        return;   /* window still filling */
    }

    uint16_t hist[LWD_HIST_BINS];
    uint32_t success, timeout_cnt;
    portENTER_CRITICAL(&s_hist_mux);
    memcpy(hist, s_hist, sizeof(hist));
    success = s_ping_success;
    timeout_cnt = s_ping_timeout;
    memset(s_hist, 0, sizeof(s_hist));
    s_ping_success = 0;
    s_ping_timeout = 0;
    portEXIT_CRITICAL(&s_hist_mux);
    s_window_start_us = now_us;

    uint32_t median_ms = 0, p90_ms = 0;
    compute_percentiles(hist, success, &median_ms, &p90_ms);

    wifi_ap_record_t ap_info;
    bool associated = (esp_wifi_sta_get_ap_info(&ap_info) == ESP_OK);

    lwd_window_sample_t w = {
        .median_rtt_ms = median_ms,
        .p90_rtt_ms    = p90_ms,
        .ping_total    = success + timeout_cnt,
        .ping_success  = success,
        .associated    = associated,
    };

    /* Hand the raw window to the uplink watchdog's dead-path rule (main.c),
     * which acts regardless of linkwd mode -- guard 1 below discards exactly
     * the windows that rule needs. Same task as main.c's loop, so no lock. */
    s_win_ping_success = success;
    s_win_associated   = associated;
    s_win_pending      = true;

    int64_t boot_grace_us = (int64_t)s_core.cfg.boot_grace_s * 1000000LL;
    lwd_inhibits_t inh = {
        .ota_pending_verify      = ota_health_is_pending_verify(),
        .config_trial_active     = config_trial_is_pending(),
        .within_boot_grace       = (now_us - s_boot_us) < boot_grace_us,
        .within_disconnect_grace = (s_last_disconnect_us >= 0) &&
                                   ((now_us - s_last_disconnect_us) < boot_grace_us),
    };

    lwd_quorum_t q;
    quorum_snapshot((uint64_t)now_us, &q);

    /* A /config push can flip observe<->act<->off between windows (see
     * config_api.c's is_live("linkwd")); re-read every window rather than
     * caching it once at init. */
    s_core.mode = g_nvs_config.linkwd_mode;

    uint64_t lifetime_s = s_lifetime_base_s + (uint64_t)(now_us / 1000000LL);
    uint32_t jitter = esp_random();

    lwd_action_t action = lwd_core_tick(&s_core, &w, &inh, &q, lifetime_s, jitter);

    lwd_persist_save(&s_core.persist, lifetime_s);

    /* node_log + console: every state-change or action, not every window --
     * at 60 s cadence over months, logging a steady HEALTHY/DEGRADED state
     * every tick would be most of node_log's retention spent on repetition.
     * "Every state change and every action or would-act" (ADR-427) is
     * exactly this condition. */
    static lwd_window_class_t s_prev_class   = LWD_WINDOW_HEALTHY;
    static lwd_inhibit_t      s_prev_inhibit = LWD_INHIBIT_NONE;
    bool changed = (s_core.last_class != s_prev_class) ||
                  (s_core.last_inhibit != s_prev_inhibit) ||
                  (action != LWD_ACTION_NONE);
    if (changed) {
        uint32_t median_clamped = (median_ms > 0xFFFF) ? 0xFFFF : median_ms;
        uint32_t loss_clamped   = (s_core.last_loss_pct > 100) ? 100 : s_core.last_loss_pct;
        uint32_t baseline_field = s_core.baseline_seeded
            ? ((s_core.baseline_ms > 0xFFFF) ? 0xFFFF : s_core.baseline_ms)
            : 0xFFFF;
        uint32_t peers_degraded_c = (q.degraded_peers > 255) ? 255 : q.degraded_peers;
        uint32_t peers_valid_c    = (q.valid_peers > 255) ? 255 : q.valid_peers;

        /* Packing documented in node_log.h next to NODE_LOG_EV_LINK_WD. */
        int32_t a = (int32_t)(((uint32_t)action & 0xFu) << 28) |
                    (int32_t)(((uint32_t)s_core.last_inhibit & 0xFu) << 24) |
                    (int32_t)((median_clamped & 0xFFFFu) << 8) |
                    (int32_t)(loss_clamped & 0xFFu);
        int32_t b = (int32_t)((baseline_field & 0xFFFFu) << 16) |
                    (int32_t)((peers_degraded_c & 0xFFu) << 8) |
                    (int32_t)(peers_valid_c & 0xFFu);
        node_log_event(NODE_LOG_EV_LINK_WD, a, b);

        ESP_LOGI(TAG, "class=%d action=%s inhibit=%s median=%" PRIu32 "ms p90=%" PRIu32
                      "ms loss=%" PRIu32 "%% baseline=%" PRIu32 "ms streak=%" PRIu32
                      " peers=%" PRIu32 "/%" PRIu32,
                 (int)s_core.last_class, link_watchdog_action_name((uint8_t)action),
                 link_watchdog_inhibit_name((uint8_t)s_core.last_inhibit),
                 median_ms, p90_ms, s_core.last_loss_pct,
                 s_core.baseline_seeded ? s_core.baseline_ms : 0,
                 s_core.degraded_streak, q.degraded_peers, q.valid_peers);
    }
    s_prev_class = s_core.last_class;
    s_prev_inhibit = s_core.last_inhibit;

    if (action == LWD_ACTION_REASSOC) {
        ESP_LOGW(TAG, "ADR-427: reassociating -- link degraded %" PRIu32
                      " consecutive windows", s_core.cfg.n_reassoc);
        esp_wifi_disconnect();   /* main.c's existing backoff reconnect brings it back */
    } else if (action == LWD_ACTION_REBOOT) {
        ESP_LOGE(TAG, "ADR-427: rebooting -- still degraded %" PRIu32
                      " windows after the reassociate grace", s_core.cfg.n_reboot);
        esp_restart();
    }
}

void link_watchdog_get_report(link_watchdog_report_t *out)
{
    memset(out, 0, sizeof(*out));
    if (!s_inited) return;
    out->mode                   = s_core.mode;
    out->baseline_ms            = s_core.baseline_ms;
    out->baseline_seeded        = s_core.baseline_seeded;
    out->last_median_ms         = s_core.last_median_ms;
    out->last_p90_ms            = s_core.last_p90_ms;
    out->last_loss_pct          = s_core.last_loss_pct;
    out->degraded_streak        = s_core.degraded_streak;
    out->in_grace                = s_core.in_grace;
    out->actions_taken           = s_core.actions_taken;
    out->would_actions_taken     = s_core.would_actions_taken;
    out->last_action              = (uint8_t)s_core.last_action;
    out->last_action_lifetime_s   = s_core.last_action_lifetime_s;
    out->last_inhibit              = (uint8_t)s_core.last_inhibit;

    lwd_quorum_t q;
    quorum_snapshot((uint64_t)esp_timer_get_time(), &q);
    out->peers_valid    = q.valid_peers;
    out->peers_degraded = q.degraded_peers;
}

#else /* !CONFIG_LINK_WATCHDOG -- compiled in, inert: see link_watchdog.h */

void link_watchdog_init(void) {}
void link_watchdog_tick(void) {}

void link_watchdog_ping_success_cb(esp_ping_handle_t hdl, void *args)
{
    (void)hdl;
    (void)args;
}

void link_watchdog_ping_timeout_cb(esp_ping_handle_t hdl, void *args)
{
    (void)hdl;
    (void)args;
}

bool link_watchdog_is_degraded(void) { return false; }

bool link_watchdog_take_window(uint32_t *ping_success, bool *associated)
{
    (void)ping_success;
    (void)associated;
    return false;
}

void link_watchdog_note_peer(const uint8_t mac[6], uint64_t now_us, bool degraded)
{
    (void)mac;
    (void)now_us;
    (void)degraded;
}

void link_watchdog_note_wifi_disconnect(void) {}

void link_watchdog_get_report(link_watchdog_report_t *out)
{
    memset(out, 0, sizeof(*out));
}

#endif /* CONFIG_LINK_WATCHDOG */
