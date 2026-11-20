/* On-chip thermal monitoring and radio throttling. See thermal.c for why the
 * thresholds are set against die temperature rather than the plastic. */
#pragma once

#include "esp_err.h"
#include <stdint.h>

typedef enum {
    THERMAL_OK = 0,
    THERMAL_WARN,
    THERMAL_THROTTLED,
    THERMAL_CRITICAL,
} thermal_state_t;

/**
 * Decide the next thermal state from a reading and the current state.
 *
 * Pure: no globals, no hardware. Kept in the public header so a host test can
 * exercise the exact function the device build runs, rather than a copy of it
 * that is free to drift.
 *
 * Rising edges act immediately -- heat is a reason to react now. EVERY falling
 * edge requires cooling a full `hyst_c` below the threshold that was crossed on
 * the way up, because recovering at the same point that tripped oscillates, and
 * each oscillation steps this node's transmit power, which moves the RSSI of
 * every link it has. A reader cannot distinguish that from a wall moving.
 *
 * The CRITICAL -> THROTTLED edge is the reason this exists as a function. It
 * used to be unconditional: any reading below `critical_c` dropped the state
 * immediately, so a node sitting on the critical boundary flapped between
 * minimum and step-2 transmit power indefinitely -- precisely the oscillation
 * the hysteresis elsewhere was added to prevent. With defaults (critical 80,
 * hysteresis 8) leaving CRITICAL now needs 72 C, which is exactly the THROTTLED
 * entry point.
 *
 * @param cur         Current state.
 * @param c           Die temperature, degrees C.
 * @param warn_c      Warn threshold.
 * @param throttle_c  Throttle threshold.
 * @param critical_c  Critical threshold.
 * @param hyst_c      Degrees of cooling required before any step down.
 * @return The state to adopt.
 */
static inline thermal_state_t thermal_next_state(thermal_state_t cur, float c,
                                                 int warn_c, int throttle_c,
                                                 int critical_c, int hyst_c)
{
    const float h = (float)hyst_c;

    /* Rising: immediate, at the plain thresholds. */
    if (c >= (float)critical_c) return THERMAL_CRITICAL;

    /* Falling out of CRITICAL needs a full hysteresis band, like every other
     * downward step. Without this the state tracks `c` across critical_c with
     * no damping at all. */
    if (cur == THERMAL_CRITICAL && c >= (float)critical_c - h) {
        return THERMAL_CRITICAL;
    }

    if (c >= (float)throttle_c) return THERMAL_THROTTLED;

    if (cur == THERMAL_THROTTLED && c >= (float)throttle_c - h) {
        return THERMAL_THROTTLED;
    }

    if (c >= (float)warn_c) return THERMAL_WARN;

    if (cur == THERMAL_WARN && c >= (float)warn_c - h) {
        return THERMAL_WARN;
    }

    return THERMAL_OK;
}

#if defined(CONFIG_THERMAL_MONITOR) && defined(CONFIG_IDF_TARGET_ESP32C6)

esp_err_t thermal_init(void);

/* Sample and apply policy. Call from a slow loop -- the die does not change
 * fast, and reading it more often buys nothing. */
void thermal_tick(void);

float           thermal_celsius(void);
float           thermal_peak_celsius(void);
thermal_state_t thermal_state(void);

/* Current WiFi transmit ceiling in whole dBm.
 *
 * Exposed because a throttled node's RSSI falls at both ends, which is
 * indistinguishable from an obstruction in the link diagnostics. Anything
 * reading link strength should check this before concluding a wall moved. */
int8_t          thermal_tx_dbm(void);

const char     *thermal_state_name(thermal_state_t s);

#else  /* stubs: S3 build and when monitoring is off */

static inline esp_err_t thermal_init(void) { return ESP_OK; }
static inline void      thermal_tick(void) {}
static inline float     thermal_celsius(void) { return -273.0f; }
static inline float     thermal_peak_celsius(void) { return -273.0f; }
static inline thermal_state_t thermal_state(void) { return THERMAL_OK; }
static inline int8_t    thermal_tx_dbm(void) { return 20; }
static inline const char *thermal_state_name(thermal_state_t s) { (void)s; return "n/a"; }

#endif
