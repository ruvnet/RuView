/**
 * @file c6_twt.h
 * @brief ESP32-C6 TWT (Target Wake Time) helper — ADR-110 Phase 3.
 *
 * Wraps esp_wifi_sta_itwt_setup() to negotiate a deterministic wake slot
 * with the AP, replacing today's opportunistic CSI capture cadence with
 * a scheduler-bounded one.
 *
 * Only built when CONFIG_IDF_TARGET_ESP32C6 is set — the S3 radio is
 * 802.11n only and cannot speak iTWT.
 *
 * Usage from main.c (after WiFi STA is connected):
 *     c6_twt_setup_default();   // honors CONFIG_C6_TWT_WAKE_INTERVAL_US
 *
 * Graceful failure: if the AP rejects (no 11ax support, doesn't allow
 * iTWT, or returns a NACK), the helper logs and returns ESP_OK — the
 * device keeps doing opportunistic CSI just like the S3.
 */

#pragma once

#ifdef __cplusplus
extern "C" {
#endif

#include "soc/soc_caps.h"

#if (defined(CONFIG_IDF_TARGET_ESP32C6) || defined(CONFIG_IDF_TARGET_ESP32C5)) && SOC_WIFI_HE_SUPPORT

#include "esp_err.h"
#include <stdint.h>
#include <stdbool.h>

/**
 * Set up an individual TWT agreement using the Kconfig defaults
 * (CONFIG_C6_TWT_WAKE_INTERVAL_US, CONFIG_C6_TWT_MIN_WAKE_DURA_US).
 *
 * @return ESP_OK whether or not the AP accepted — the helper never
 *         propagates a TWT NACK as an error to the caller.
 */
esp_err_t c6_twt_setup_default(void);

/**
 * Set up an individual TWT agreement with explicit parameters.
 *
 * @param wake_interval_us  Period between wake events.
 * @param min_wake_dura_us  Minimum awake duration per wake (≥256 µs).
 * @return ESP_OK on success or graceful NACK; ESP_FAIL on local error.
 */
esp_err_t c6_twt_setup(uint32_t wake_interval_us, uint32_t min_wake_dura_us);

/**
 * Tear down any active TWT agreement. Safe to call when none is active.
 * Should be invoked on WIFI_EVENT_STA_DISCONNECTED so the AP scheduler
 * doesn't keep a dead slot reserved.
 */
void c6_twt_teardown(void);

/**
 * Returns true if a TWT agreement is currently active.
 */
bool c6_twt_is_active(void);

#else  /* not C6 with iTWT support — provide stubs so call sites compile */

static inline esp_err_t c6_twt_setup_default(void) { return ESP_OK; }
static inline esp_err_t c6_twt_setup(uint32_t a, uint32_t b) { (void)a; (void)b; return ESP_OK; }
static inline void      c6_twt_teardown(void) { }
static inline bool      c6_twt_is_active(void) { return false; }

#endif  /* CONFIG_IDF_TARGET_ESP32C6 && SOC_WIFI_HE_SUPPORT */

#ifdef __cplusplus
}
#endif
