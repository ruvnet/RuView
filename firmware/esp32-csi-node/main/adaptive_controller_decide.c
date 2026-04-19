/**
 * @file adaptive_controller_decide.c
 * @brief ADR-081 Layer 2 — pure decision policy.
 *
 * Extracted so host unit tests can link this without ESP-IDF / FreeRTOS.
 * adaptive_controller.c includes this file; the host Makefile links it
 * directly against the test harness.
 */

#include <string.h>
#include "adaptive_controller.h"
#include "rv_radio_ops.h"

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

    /* Degraded gate: pkt yield collapse or severe coherence loss → DEGRADED. */
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

    /* Stable presence + quiet → high-sensitivity respiration. */
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
