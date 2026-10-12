/**
 * @file c6_softap_he.h
 * @brief ESP32-C6 soft-AP with Wi-Fi 6 (HE) capability + TWT Responder.
 *
 * ADR-110 §B1/B2 cheap-unblock: turn one C6 board into the iTWT-capable
 * AP that the C6-DevKit-on-the-shelf-only bench is missing. A second C6
 * board in STA mode can then negotiate a real iTWT agreement against
 * this AP and measure deterministic CSI cadence — without buying an
 * 11ax router.
 *
 * Build-gated by CONFIG_C6_SOFTAP_HE_ENABLE (default n). When disabled,
 * all functions become no-ops so non-AP firmwares pay zero overhead.
 *
 * NVS overrides (read at boot if present, fall back to Kconfig defaults):
 *   softap_ssid   (string, up to 32 chars)
 *   softap_psk    (string, 8..63 chars)
 *   softap_chan   (u8, 1..13)
 */

#pragma once

#ifdef __cplusplus
extern "C" {
#endif

#include "esp_err.h"
#include <stdint.h>
#include <stdbool.h>

#if (defined(CONFIG_IDF_TARGET_ESP32C6) || defined(CONFIG_IDF_TARGET_ESP32C5)) && defined(CONFIG_C6_SOFTAP_HE_ENABLE)

/**
 * Bring up the soft-AP in AP+STA mode with HE (Wi-Fi 6) advertised and
 * TWT Responder=1 if the IDF build supports it. Idempotent — safe to
 * call once during boot after `esp_wifi_init()`. Returns the channel
 * the AP is actually running on (may differ from Kconfig if the IDF
 * scanner picks a clearer channel).
 */
esp_err_t c6_softap_he_start(uint8_t *out_channel);

/**
 * True after the IDF reports the AP has started successfully.
 */
bool c6_softap_he_is_up(void);

/**
 * Number of currently associated stations (read-only, refreshed on the
 * WIFI_EVENT_AP_STACONNECTED/DISCONNECTED events).
 */
uint8_t c6_softap_he_sta_count(void);

#else  /* disabled — no-op stubs */

static inline esp_err_t c6_softap_he_start(uint8_t *out_channel)
{
    if (out_channel) *out_channel = 0;
    return ESP_OK;
}
static inline bool    c6_softap_he_is_up(void)     { return false; }
static inline uint8_t c6_softap_he_sta_count(void) { return 0; }

#endif

#ifdef __cplusplus
}
#endif
