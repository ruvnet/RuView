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
#include "rv_feature_state.h"
#include "rv_mesh.h"
#include "edge_processing.h"
#include "stream_sender.h"
#include "csi_collector.h"

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

/* Forward decl: defined below, called from fast_loop_cb. */
static void emit_feature_state(void);

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

/* Pure decision policy lives in its own file so it can link under
 * host unit tests without FreeRTOS. It is part of this translation
 * unit via #include to preserve a single object at build time. */
#include "adaptive_controller_decide.c"

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

/* ADR-081 L3: epoch monotonically advances per mesh session. Seeded at
 * init; every major state transition or role change bumps it so
 * receivers can order events. */
static uint32_t s_mesh_epoch = 1;

/* ADR-081 L3: current node role. Updated by ROLE_ASSIGN receipt (future
 * mesh-plane RX path) or forced by tests. Default Observer. */
static uint8_t s_role = RV_ROLE_OBSERVER;

/* 8-byte node id. Upper 7 bytes are zero by default; byte 0 is the
 * legacy CSI node id for compatibility with the ADR-018 header. */
static void node_id_bytes(uint8_t out[8])
{
    memset(out, 0, 8);
    out[0] = csi_collector_get_node_id();
}

static void apply_decision(const adapt_decision_t *dec)
{
    const rv_radio_ops_t *ops = rv_radio_ops_get();
    adapt_state_t prev = s_state;

    if (dec->change_state) {
        ESP_LOGI(TAG, "state %u → %u",
                 (unsigned)s_state, (unsigned)dec->new_state);
        s_state = (adapt_state_t)dec->new_state;

        /* ADR-081 L3: on transition to ALERT, emit ANOMALY_ALERT on the
         * mesh plane. On any role-relevant transition, bump the epoch. */
        if (s_state == ADAPT_STATE_ALERT && prev != ADAPT_STATE_ALERT) {
            uint8_t nid[8];
            node_id_bytes(nid);
            adapt_observation_t obs;
            float motion = 0.0f, anomaly = 0.0f;
            portENTER_CRITICAL(&s_obs_lock);
            if (s_obs_valid) { obs = s_last_obs; motion = obs.motion_score;
                               anomaly = obs.anomaly_score; }
            portEXIT_CRITICAL(&s_obs_lock);
            uint8_t severity = (uint8_t)(anomaly * 255.0f);
            rv_mesh_send_anomaly(s_role, s_mesh_epoch, nid,
                                 RV_ANOMALY_COHERENCE_LOSS, severity,
                                 anomaly, motion);
        }
        if (s_state == ADAPT_STATE_DEGRADED && prev != ADAPT_STATE_DEGRADED) {
            uint8_t nid[8];
            node_id_bytes(nid);
            rv_mesh_send_anomaly(s_role, s_mesh_epoch, nid,
                                 RV_ANOMALY_PKT_YIELD_COLLAPSE,
                                 200, 1.0f, 0.0f);
        }
        s_mesh_epoch++;
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

    /* ADR-081 Layer 4/5: emit compact feature state on every fast tick
     * (default 200 ms → 5 Hz, within the 1–10 Hz spec). Replaces raw
     * ADR-018 CSI as the default upstream; raw remains available as a
     * debug stream gated by the channel plan. */
    emit_feature_state();
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

/* ADR-081 Layer 4: emit one rv_feature_state_t packet onto the wire.
 *
 * Pulls from the latest observation + latest vitals + the active capture
 * profile. Send is best-effort — stream_sender will report its own
 * failures; we don't re-queue. At 5 Hz default cadence this is 300 B/s
 * per node, vs. ~100 KB/s for raw ADR-018 CSI. */
static uint16_t s_feature_state_seq = 0;

static void emit_feature_state(void)
{
    rv_feature_state_t pkt;
    memset(&pkt, 0, sizeof(pkt));

    adapt_observation_t obs;
    bool have_obs = false;
    portENTER_CRITICAL(&s_obs_lock);
    if (s_obs_valid) {
        obs = s_last_obs;
        have_obs = true;
    }
    portEXIT_CRITICAL(&s_obs_lock);

    if (have_obs) {
        pkt.motion_score    = obs.motion_score;
        pkt.presence_score  = obs.presence_score;
        pkt.anomaly_score   = obs.anomaly_score;
        pkt.node_coherence  = obs.node_coherence;
    }

    /* Fill vitals from edge_processing's latest packet. */
    edge_vitals_pkt_t v;
    if (edge_get_vitals(&v)) {
        pkt.respiration_bpm  = (float)v.breathing_rate / 100.0f;
        pkt.heartbeat_bpm    = (float)v.heartrate / 10000.0f;
        /* Confidence proxies: presence score for resp, 1.0 if heart BPM
         * is within physiological range. */
        pkt.respiration_conf = (v.breathing_rate > 0) ? v.presence_score : 0.0f;
        pkt.heartbeat_conf   = (v.heartrate > 400000u && v.heartrate < 1800000u)
                                 ? 0.8f : 0.0f;
        if (pkt.respiration_bpm > 0.0f) pkt.quality_flags |= RV_QFLAG_RESPIRATION_VALID;
        if (pkt.heartbeat_bpm   > 0.0f) pkt.quality_flags |= RV_QFLAG_HEARTBEAT_VALID;
        if (pkt.presence_score >= 0.5f) pkt.quality_flags |= RV_QFLAG_PRESENCE_VALID;
        if (v.flags & 0x02)             pkt.quality_flags |= RV_QFLAG_ANOMALY_TRIGGERED;  /* fall bit */
    }

    if (s_state == ADAPT_STATE_DEGRADED)   pkt.quality_flags |= RV_QFLAG_DEGRADED_MODE;
    if (s_state == ADAPT_STATE_CALIBRATION) pkt.quality_flags |= RV_QFLAG_CALIBRATING;

    /* Active profile, for receiver-side weighting. */
    const rv_radio_ops_t *ops = rv_radio_ops_get();
    uint8_t profile = RV_PROFILE_PASSIVE_LOW_RATE;
    if (ops != NULL && ops->get_health != NULL) {
        rv_radio_health_t h;
        if (ops->get_health(&h) == ESP_OK) profile = h.current_profile;
    }

    rv_feature_state_finalize(&pkt,
                              csi_collector_get_node_id(),
                              s_feature_state_seq++,
                              (uint64_t)esp_timer_get_time(),
                              profile);

    int sent = stream_sender_send((const uint8_t *)&pkt, sizeof(pkt));
    if (sent < 0) {
        ESP_LOGW(TAG, "feature_state emit failed");
    }
}

static void slow_loop_cb(TimerHandle_t t)
{
    (void)t;
    /* ADR-081 L3: publish a HEALTH mesh message every slow tick
     * (default 30 s). The coordinator uses these to track liveness and
     * detect sync-error drift. */
    uint8_t nid[8];
    node_id_bytes(nid);
    rv_mesh_send_health(s_role, s_mesh_epoch, nid);

    ESP_LOGI(TAG, "slow tick (state=%u, feature_state_seq=%u, role=%u, epoch=%u) HEALTH sent",
             (unsigned)s_state, (unsigned)s_feature_state_seq,
             (unsigned)s_role, (unsigned)s_mesh_epoch);
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
