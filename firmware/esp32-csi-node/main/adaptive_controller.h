/**
 * @file adaptive_controller.h
 * @brief ADR-081 Layer 2 — Adaptive sensing controller.
 *
 * Closed-loop firmware control over cadence, capture profile, channel, and
 * mesh role. Three cooperating loops:
 *
 *   Fast   (~200 ms): packet rate, active probing
 *   Medium (~1 s)   : channel selection, role transitions
 *   Slow   (~30 s)  : baseline recalibration
 *
 * Outputs are routed through:
 *   - rv_radio_ops_t (Layer 1) for set_channel / set_capture_profile
 *   - swarm_bridge / mesh plane (Layer 3) for CHANNEL_PLAN, ROLE_ASSIGN
 *   - edge_processing (Layer 4) for cadence and threshold updates
 *
 * Default policy is conservative — matches today's behavior. Aggressive
 * adaptation is opt-in via Kconfig (ADAPTIVE_CONTROLLER_AGGRESSIVE).
 */

#ifndef ADAPTIVE_CONTROLLER_H
#define ADAPTIVE_CONTROLLER_H

#include <stdint.h>
#include <stdbool.h>
#include "esp_err.h"

#ifdef __cplusplus
extern "C" {
#endif

/** Controller-level state machine (ADR-081 firmware FSM). */
typedef enum {
    ADAPT_STATE_BOOT          = 0,
    ADAPT_STATE_SELF_TEST     = 1,
    ADAPT_STATE_RADIO_INIT    = 2,
    ADAPT_STATE_TIME_SYNC     = 3,
    ADAPT_STATE_CALIBRATION   = 4,
    ADAPT_STATE_SENSE_IDLE    = 5,
    ADAPT_STATE_SENSE_ACTIVE  = 6,
    ADAPT_STATE_ALERT         = 7,
    ADAPT_STATE_DEGRADED      = 8,
} adapt_state_t;

/** Observation window aggregated each fast tick. */
typedef struct {
    uint16_t pkt_yield_per_sec;   /**< From rv_radio_health.pkt_yield_per_sec. */
    uint16_t send_fail_count;     /**< UDP/socket send failures. */
    int8_t   rssi_median_dbm;
    int8_t   noise_floor_dbm;
    float    motion_score;        /**< Pulled from edge_processing. */
    float    presence_score;
    float    anomaly_score;
    float    node_coherence;      /**< Inter-link coherence; 1.0 if single node. */
} adapt_observation_t;

/** Decisions emitted by a controller tick. */
typedef struct {
    bool     change_profile;
    uint8_t  new_profile;         /**< rv_capture_profile_t. */
    bool     change_channel;
    uint8_t  new_channel;
    bool     change_state;
    uint8_t  new_state;           /**< adapt_state_t. */
    bool     request_calibration; /**< Coordinator should issue CALIBRATION_START. */
    uint16_t suggested_vital_interval_ms;
} adapt_decision_t;

/** Controller config (loaded from NVS / Kconfig). */
typedef struct {
    uint16_t fast_loop_ms;        /**< Default 200 ms. */
    uint16_t medium_loop_ms;      /**< Default 1000 ms. */
    uint16_t slow_loop_ms;        /**< Default 30000 ms. */
    bool     aggressive;          /**< true = react sooner / more often. */
    bool     enable_channel_switch; /**< false = controller may never hop. */
    bool     enable_role_change;
    float    motion_threshold;    /**< 0..1, enter SENSE_ACTIVE above this. */
    float    anomaly_threshold;   /**< 0..1, enter ALERT above this. */
    uint16_t min_pkt_yield;       /**< pps below this → DEGRADED. */
} adapt_config_t;

/**
 * Initialize the adaptive controller.
 *
 * Spawns one FreeRTOS task that runs the three loops via FreeRTOS timers.
 * Idempotent — second call is a no-op.
 *
 * @param cfg  Config (NULL = use Kconfig defaults).
 * @return ESP_OK on success.
 */
esp_err_t adaptive_controller_init(const adapt_config_t *cfg);

/** Get the current state. */
adapt_state_t adaptive_controller_state(void);

/**
 * Snapshot the latest observation (most recent fast-loop sample).
 * Useful for telemetry and the `HEALTH` mesh message.
 *
 * @param out  Output buffer.
 * @return true if a valid observation has been recorded.
 */
bool adaptive_controller_observation(adapt_observation_t *out);

/**
 * Force a state transition (e.g. from a remote ROLE_ASSIGN message).
 * Logged at INFO; controller may immediately transition again on next tick.
 */
void adaptive_controller_force_state(adapt_state_t st);

/**
 * Pure-function policy: given an observation + current state + config,
 * compute the decision. Exposed in the header so it can be unit-tested
 * offline (no FreeRTOS / ESP-IDF dependency in the body).
 */
void adaptive_controller_decide(const adapt_config_t *cfg,
                                adapt_state_t current,
                                const adapt_observation_t *obs,
                                adapt_decision_t *out);

#ifdef __cplusplus
}
#endif

#endif /* ADAPTIVE_CONTROLLER_H */
