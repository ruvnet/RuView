/**
 * @file adaptive_controller.c
 * @brief ADR-081 Layer 2 — Adaptive sensing controller implementation.
 *
 * The decide() function is pure and unit-testable; the FreeRTOS plumbing
 * around it (timers, observation snapshot) is the only ESP-IDF surface.
 *
 * Default policy is conservative: it will not change channels unless
 * enable_channel_switch is true, and it will not change roles unless
 * enable_role_change is true. With both off the controller still tracks
 * state and feeds the mesh plane's HEALTH messages, so it is safe to
 * enable in production before the mesh plane is fully in place.
 */

#include "adaptive_controller.h"
#include "rv_radio_ops.h"
#include "edge_processing.h"

#include <string.h>
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "freertos/timers.h"
#include "esp_log.h"
#include "esp_timer.h"
#include "sdkconfig.h"

static const char *TAG = "adaptive_ctrl";

/* ---- Module state ---- */

static bool                s_inited = false;
static adapt_config_t      s_cfg;
static adapt_state_t       s_state = ADAPT_STATE_BOOT;
static adapt_observation_t s_last_obs;
static bool                s_obs_valid = false;
static portMUX_TYPE        s_obs_lock = portMUX_INITIALIZER_UNLOCKED;

static TimerHandle_t s_fast_timer   = NULL;
static TimerHandle_t s_medium_timer = NULL;
static TimerHandle_t s_slow_timer   = NULL;

/* ---- Defaults ---- */

#ifndef CONFIG_ADAPTIVE_FAST_LOOP_MS
#define CONFIG_ADAPTIVE_FAST_LOOP_MS    200
#endif
#ifndef CONFIG_ADAPTIVE_MEDIUM_LOOP_MS
#define CONFIG_ADAPTIVE_MEDIUM_LOOP_MS  1000
#endif
#ifndef CONFIG_ADAPTIVE_SLOW_LOOP_MS
#define CONFIG_ADAPTIVE_SLOW_LOOP_MS    30000
#endif
#ifndef CONFIG_ADAPTIVE_MIN_PKT_YIELD
#define CONFIG_ADAPTIVE_MIN_PKT_YIELD   5
#endif
/* Defaults expressed as integer permille so Kconfig can carry them. */
#ifndef CONFIG_ADAPTIVE_MOTION_THRESH_PERMIL
#define CONFIG_ADAPTIVE_MOTION_THRESH_PERMIL   200  /* 0.20 */
#endif
#ifndef CONFIG_ADAPTIVE_ANOMALY_THRESH_PERMIL
#define CONFIG_ADAPTIVE_ANOMALY_THRESH_PERMIL  600  /* 0.60 */
#endif

static void apply_defaults(adapt_config_t *cfg)
{
    cfg->fast_loop_ms          = CONFIG_ADAPTIVE_FAST_LOOP_MS;
    cfg->medium_loop_ms        = CONFIG_ADAPTIVE_MEDIUM_LOOP_MS;
    cfg->slow_loop_ms          = CONFIG_ADAPTIVE_SLOW_LOOP_MS;
#ifdef CONFIG_ADAPTIVE_AGGRESSIVE
    cfg->aggressive            = true;
#else
    cfg->aggressive            = false;
#endif
#ifdef CONFIG_ADAPTIVE_ENABLE_CHANNEL_SWITCH
    cfg->enable_channel_switch = true;
#else
    cfg->enable_channel_switch = false;
#endif
#ifdef CONFIG_ADAPTIVE_ENABLE_ROLE_CHANGE
    cfg->enable_role_change    = true;
#else
    cfg->enable_role_change    = false;
#endif
    cfg->motion_threshold  = (float)CONFIG_ADAPTIVE_MOTION_THRESH_PERMIL  / 1000.0f;
    cfg->anomaly_threshold = (float)CONFIG_ADAPTIVE_ANOMALY_THRESH_PERMIL / 1000.0f;
    cfg->min_pkt_yield     = CONFIG_ADAPTIVE_MIN_PKT_YIELD;
}

/* ---- Pure decision function (unit-testable) ---- */

void adaptive_controller_decide(const adapt_config_t *cfg,
                                adapt_state_t current,
                                const adapt_observation_t *obs,
                                adapt_decision_t *out)
{
    if (cfg == NULL || obs == NULL || out == NULL) {
        return;
    }
    memset(out, 0, sizeof(*out));
    out->new_state   = (uint8_t)current;
    out->new_profile = RV_PROFILE_PASSIVE_LOW_RATE;

    /* Degraded gate: any of pkt yield collapse, severe coherence loss → DEGRADED. */
    if (obs->pkt_yield_per_sec < cfg->min_pkt_yield ||
        obs->node_coherence    < 0.20f) {
        if (current != ADAPT_STATE_DEGRADED) {
            out->change_state = true;
            out->new_state    = ADAPT_STATE_DEGRADED;
        }
        out->change_profile = (current != ADAPT_STATE_DEGRADED);
        out->new_profile    = RV_PROFILE_PASSIVE_LOW_RATE;
        out->suggested_vital_interval_ms = 2000;
        return;
    }

    /* Anomaly trumps motion. */
    if (obs->anomaly_score >= cfg->anomaly_threshold) {
        if (current != ADAPT_STATE_ALERT) {
            out->change_state = true;
            out->new_state    = ADAPT_STATE_ALERT;
        }
        out->change_profile = true;
        out->new_profile    = RV_PROFILE_FAST_MOTION;
        out->suggested_vital_interval_ms = 100;
        return;
    }

    /* Motion → SENSE_ACTIVE with FAST_MOTION profile. */
    if (obs->motion_score >= cfg->motion_threshold) {
        if (current != ADAPT_STATE_SENSE_ACTIVE) {
            out->change_state = true;
            out->new_state    = ADAPT_STATE_SENSE_ACTIVE;
        }
        out->change_profile = true;
        out->new_profile    = RV_PROFILE_FAST_MOTION;
        out->suggested_vital_interval_ms = cfg->aggressive ? 100 : 200;
        return;
    }

    /* Stable environment with valid presence → high-sensitivity respiration mode. */
    if (obs->presence_score >= 0.5f && obs->motion_score < 0.05f) {
        if (current != ADAPT_STATE_SENSE_IDLE) {
            out->change_state = true;
            out->new_state    = ADAPT_STATE_SENSE_IDLE;
        }
        out->change_profile = true;
        out->new_profile    = RV_PROFILE_RESP_HIGH_SENS;
        out->suggested_vital_interval_ms = 1000;
        return;
    }

    /* Default: passive low rate. */
    if (current != ADAPT_STATE_SENSE_IDLE) {
        out->change_state = true;
        out->new_state    = ADAPT_STATE_SENSE_IDLE;
    }
    out->change_profile = (current != ADAPT_STATE_SENSE_IDLE);
    out->new_profile    = RV_PROFILE_PASSIVE_LOW_RATE;
    out->suggested_vital_interval_ms = cfg->aggressive ? 500 : 1000;
}

/* ---- Observation collection ---- */

static void collect_observation(adapt_observation_t *out)
{
    memset(out, 0, sizeof(*out));

    /* Radio health from the active binding. */
    const rv_radio_ops_t *ops = rv_radio_ops_get();
    if (ops != NULL && ops->get_health != NULL) {
        rv_radio_health_t h;
        if (ops->get_health(&h) == ESP_OK) {
            out->pkt_yield_per_sec = h.pkt_yield_per_sec;
            out->send_fail_count   = h.send_fail_count;
            out->rssi_median_dbm   = h.rssi_median_dbm;
            out->noise_floor_dbm   = h.noise_floor_dbm;
        }
    }

    /* Edge-derived state. The ADR-039 vitals packet exposes presence_score
     * and motion_energy directly; we treat motion_energy as a proxy for
     * motion_score by clamping to [0,1]. anomaly_score and node_coherence
     * are not yet emitted by edge_processing — placeholder until Layer 4
     * extraction lands. */
    edge_vitals_pkt_t vitals;
    if (edge_get_vitals(&vitals)) {
        out->presence_score = vitals.presence_score;
        float m = vitals.motion_energy;
        if (m < 0.0f) m = 0.0f;
        if (m > 1.0f) m = 1.0f;
        out->motion_score   = m;
    }
    out->anomaly_score  = 0.0f;
    out->node_coherence = 1.0f;
}

/* ---- Decision application ---- */

static void apply_decision(const adapt_decision_t *dec)
{
    const rv_radio_ops_t *ops = rv_radio_ops_get();

    if (dec->change_state) {
        ESP_LOGI(TAG, "state %u → %u",
                 (unsigned)s_state, (unsigned)dec->new_state);
        s_state = (adapt_state_t)dec->new_state;
    }

    if (dec->change_profile && ops != NULL && ops->set_capture_profile != NULL) {
        ops->set_capture_profile(dec->new_profile);
    }

    if (dec->change_channel && s_cfg.enable_channel_switch &&
        ops != NULL && ops->set_channel != NULL) {
        ops->set_channel(dec->new_channel, 20);
    }

    /* suggested_vital_interval_ms: the controller publishes a hint; the
     * edge pipeline picks it up via edge_processing on its next emit. We
     * don't yet have edge_set_vital_interval(); recorded for Phase 3. */
    (void)dec->request_calibration;
}

/* ---- Loop callbacks ---- */

static void fast_loop_cb(TimerHandle_t t)
{
    (void)t;
    adapt_observation_t obs;
    collect_observation(&obs);

    portENTER_CRITICAL(&s_obs_lock);
    s_last_obs  = obs;
    s_obs_valid = true;
    portEXIT_CRITICAL(&s_obs_lock);

    adapt_decision_t dec;
    adaptive_controller_decide(&s_cfg, s_state, &obs, &dec);
    apply_decision(&dec);
}

static void medium_loop_cb(TimerHandle_t t)
{
    (void)t;
    /* Phase 3 stub: when enable_channel_switch is on, choose a channel
     * based on RSSI/noise/yield. Today, log the snapshot so operators can
     * see the controller is running. */
    adapt_observation_t obs;
    portENTER_CRITICAL(&s_obs_lock);
    obs = s_last_obs;
    portEXIT_CRITICAL(&s_obs_lock);

    if (s_obs_valid) {
        ESP_LOGI(TAG, "medium tick: state=%u yield=%upps motion=%.2f presence=%.2f rssi=%d",
                 (unsigned)s_state,
                 (unsigned)obs.pkt_yield_per_sec,
                 (double)obs.motion_score,
                 (double)obs.presence_score,
                 (int)obs.rssi_median_dbm);
    }
}

static void slow_loop_cb(TimerHandle_t t)
{
    (void)t;
    /* Slow loop: publish a HEALTH message, request CALIBRATION_START on
     * sustained drift. Both routed through swarm_bridge once the mesh
     * plane lands. Today we log a rollover so operators see the cadence. */
    ESP_LOGI(TAG, "slow tick (state=%u)", (unsigned)s_state);
}

/* ---- Public API ---- */

esp_err_t adaptive_controller_init(const adapt_config_t *cfg)
{
    if (s_inited) {
        return ESP_OK;
    }

    if (cfg != NULL) {
        s_cfg = *cfg;
    } else {
        apply_defaults(&s_cfg);
    }

    /* Sanity clamps. */
    if (s_cfg.fast_loop_ms   < 50)   s_cfg.fast_loop_ms   = 50;
    if (s_cfg.medium_loop_ms < 200)  s_cfg.medium_loop_ms = 200;
    if (s_cfg.slow_loop_ms   < 1000) s_cfg.slow_loop_ms   = 1000;

    s_state = ADAPT_STATE_RADIO_INIT;

    s_fast_timer = xTimerCreate("adapt_fast",
                                pdMS_TO_TICKS(s_cfg.fast_loop_ms),
                                pdTRUE, NULL, fast_loop_cb);
    s_medium_timer = xTimerCreate("adapt_med",
                                  pdMS_TO_TICKS(s_cfg.medium_loop_ms),
                                  pdTRUE, NULL, medium_loop_cb);
    s_slow_timer = xTimerCreate("adapt_slow",
                                pdMS_TO_TICKS(s_cfg.slow_loop_ms),
                                pdTRUE, NULL, slow_loop_cb);

    if (s_fast_timer == NULL || s_medium_timer == NULL || s_slow_timer == NULL) {
        ESP_LOGE(TAG, "timer create failed");
        return ESP_ERR_NO_MEM;
    }

    if (xTimerStart(s_fast_timer,   0) != pdPASS ||
        xTimerStart(s_medium_timer, 0) != pdPASS ||
        xTimerStart(s_slow_timer,   0) != pdPASS) {
        ESP_LOGE(TAG, "timer start failed");
        return ESP_FAIL;
    }

    s_state  = ADAPT_STATE_SENSE_IDLE;
    s_inited = true;

    ESP_LOGI(TAG,
             "adaptive controller online: fast=%ums med=%ums slow=%ums "
             "(channel_switch=%d role_change=%d aggressive=%d)",
             (unsigned)s_cfg.fast_loop_ms,
             (unsigned)s_cfg.medium_loop_ms,
             (unsigned)s_cfg.slow_loop_ms,
             (int)s_cfg.enable_channel_switch,
             (int)s_cfg.enable_role_change,
             (int)s_cfg.aggressive);
    return ESP_OK;
}

adapt_state_t adaptive_controller_state(void)
{
    return s_state;
}

bool adaptive_controller_observation(adapt_observation_t *out)
{
    if (out == NULL) return false;
    bool ok = false;
    portENTER_CRITICAL(&s_obs_lock);
    if (s_obs_valid) {
        *out = s_last_obs;
        ok = true;
    }
    portEXIT_CRITICAL(&s_obs_lock);
    return ok;
}

void adaptive_controller_force_state(adapt_state_t st)
{
    ESP_LOGI(TAG, "force state %u → %u", (unsigned)s_state, (unsigned)st);
    s_state = st;
}
