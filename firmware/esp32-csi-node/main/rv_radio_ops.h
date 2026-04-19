/**
 * @file rv_radio_ops.h
 * @brief ADR-081 Layer 1 — Radio Abstraction Layer.
 *
 * A single function-pointer vtable (rv_radio_ops_t) that isolates chipset
 * specific capture details from the layers above (adaptive controller, mesh
 * plane, feature extraction, Rust handoff).
 *
 * Two bindings ship today:
 *   - rv_radio_ops_esp32.c — wraps csi_collector + esp_wifi_*
 *   - rv_radio_ops_mock.c  — wraps mock_csi.c (when CONFIG_CSI_MOCK_ENABLED)
 *
 * A third binding (Nexmon-patched Broadcom/Cypress) is reserved but not
 * implemented here. The whole point of the vtable is that the controller
 * and mesh-plane code above never need to know which one is active.
 */

#ifndef RV_RADIO_OPS_H
#define RV_RADIO_OPS_H

#include <stdint.h>
#include <stdbool.h>
#include "esp_err.h"

#ifdef __cplusplus
extern "C" {
#endif

/* ---- Modes ---- */

/** Radio operating modes (set_mode argument). */
typedef enum {
    RV_RADIO_MODE_DISABLED       = 0,  /**< Receiver off. */
    RV_RADIO_MODE_PASSIVE_RX     = 1,  /**< Listen-only, no TX. */
    RV_RADIO_MODE_ACTIVE_PROBE   = 2,  /**< Inject NDP frames at high rate. */
    RV_RADIO_MODE_CALIBRATION    = 3,  /**< Synchronized calibration burst. */
} rv_radio_mode_t;

/* ---- Capture profiles ---- */

/**
 * Named capture profiles. The adaptive controller selects one of these
 * via set_capture_profile(); the binding maps it to chipset-specific
 * register/driver state.
 */
typedef enum {
    RV_PROFILE_PASSIVE_LOW_RATE  = 0,  /**< Default idle: minimum cadence. */
    RV_PROFILE_ACTIVE_PROBE      = 1,  /**< High-rate NDP injection. */
    RV_PROFILE_RESP_HIGH_SENS    = 2,  /**< Quietest channel, vitals-only. */
    RV_PROFILE_FAST_MOTION       = 3,  /**< Short window, high cadence. */
    RV_PROFILE_CALIBRATION       = 4,  /**< Synchronized burst across nodes. */
    RV_PROFILE_COUNT
} rv_capture_profile_t;

/* ---- Health snapshot ---- */

/** Radio-layer health, polled by the adaptive controller. */
typedef struct {
    uint16_t pkt_yield_per_sec;   /**< CSI callbacks/second observed. */
    uint16_t send_fail_count;     /**< UDP/socket send failures since last poll. */
    int8_t   rssi_median_dbm;     /**< Median RSSI over the last 1 s. */
    int8_t   noise_floor_dbm;     /**< Latest noise floor estimate. */
    uint8_t  current_channel;     /**< Channel currently configured. */
    uint8_t  current_bw_mhz;      /**< Bandwidth currently configured. */
    uint8_t  current_profile;     /**< Active rv_capture_profile_t. */
    uint8_t  reserved;
} rv_radio_health_t;

/* ---- The vtable ---- */

/**
 * Radio Abstraction Layer ops.
 *
 * All function pointers are required (no NULL slots). Each binding must
 * provide all six. Return values follow ESP-IDF conventions: 0/ESP_OK on
 * success, negative or ESP_ERR_* on failure.
 */
typedef struct {
    /** One-time init (driver register, callback wire-up). */
    int (*init)(void);

    /**
     * Tune to a primary channel with the given bandwidth.
     * @param ch  Channel number (1-13 for 2.4 GHz, 36-177 for 5 GHz).
     * @param bw  Bandwidth in MHz (20 or 40; 80/160 reserved for future).
     */
    int (*set_channel)(uint8_t ch, uint8_t bw);

    /** Switch operating mode (rv_radio_mode_t). */
    int (*set_mode)(uint8_t mode);

    /** Enable or disable the CSI capture path. */
    int (*set_csi_enabled)(bool en);

    /** Apply a named capture profile (rv_capture_profile_t). */
    int (*set_capture_profile)(uint8_t profile_id);

    /** Snapshot the radio-layer health (non-blocking). */
    int (*get_health)(rv_radio_health_t *out);
} rv_radio_ops_t;

/* ---- Registration ---- */

/**
 * Register the active radio ops binding.
 *
 * Called once at boot by the chipset binding's init code (e.g.
 * rv_radio_ops_esp32_register()). The pointer must remain valid for the
 * lifetime of the process — typically a static const inside the binding.
 */
void rv_radio_ops_register(const rv_radio_ops_t *ops);

/**
 * Get the active radio ops binding.
 *
 * @return Pointer to the registered ops table, or NULL if no binding has
 *         been registered yet (e.g. before init).
 */
const rv_radio_ops_t *rv_radio_ops_get(void);

/* ---- Convenience: ESP32 binding registration ---- */

/**
 * Register the ESP32 binding as the active radio ops.
 *
 * Call this once at boot, after csi_collector_init() has run. Idempotent.
 * Defined in rv_radio_ops_esp32.c.
 */
void rv_radio_ops_esp32_register(void);

/**
 * Register the mock binding (QEMU / offline) as the active radio ops.
 *
 * Defined in rv_radio_ops_mock.c; only built when CONFIG_CSI_MOCK_ENABLED.
 */
void rv_radio_ops_mock_register(void);

#ifdef __cplusplus
}
#endif

#endif /* RV_RADIO_OPS_H */
