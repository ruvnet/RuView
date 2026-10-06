/**
 * @file c6_lp_core.h
 * @brief LP-core wake-on-motion hibernation helper — ADR-110 Phase 5.
 *
 * Arms the C6 LP RISC-V coprocessor as an always-on watchdog that
 * monitors a GPIO (typically a PIR or accelerometer interrupt line) and
 * wakes the HP core only when motion is detected. Targets ~5 µA
 * hibernation current for battery-powered Cognitum Seed nodes.
 *
 * Only built when CONFIG_IDF_TARGET_ESP32C6 + CONFIG_ULP_COPROC_TYPE_LP_CORE.
 *
 * P5 skeleton: the LP-core program is shipped as inline C compiled into
 * the main image. A follow-up turn migrates it to a separate
 * lp_core/main.c subproject with its own CMake.
 */

#pragma once

#ifdef __cplusplus
extern "C" {
#endif

#include "esp_err.h"
#include <stdint.h>
#include <stdbool.h>

#if (defined(CONFIG_IDF_TARGET_ESP32C6) || defined(CONFIG_IDF_TARGET_ESP32C5)) && defined(CONFIG_ULP_COPROC_TYPE_LP_CORE)

/**
 * Configure the LP-core wake-on-motion watcher.
 *
 * @param wake_gpio  GPIO pin to monitor (must be an RTC/LP-domain GPIO).
 * @param active_high  true = wake on rising edge, false = falling.
 * @return ESP_OK on success.
 */
esp_err_t c6_lp_core_arm(int wake_gpio, bool active_high);

/**
 * Enter deep sleep with the LP-core armed as the wake source. Does not
 * return — the next boot will see ESP_SLEEP_WAKEUP_LP_CORE in
 * esp_sleep_get_wakeup_cause().
 */
void c6_lp_core_hibernate_and_wait(void);

/**
 * Returns true if the most recent boot was a wake from LP-core motion
 * detection (vs a cold boot or different wake source).
 */
bool c6_lp_core_was_motion_wake(void);

/**
 * Monotonic counter of wake-triggering motion events observed by the
 * LP-core program since the last cold boot. Returns 0 when
 * CONFIG_C6_LP_CORE_ENABLE is unset (fallback path).
 */
uint32_t c6_lp_core_motion_count(void);

/**
 * Total LP-timer poll iterations executed by the LP-core program.
 * Useful as a sanity check that the LP-core is actually running;
 * returns 0 on the fallback path.
 */
uint32_t c6_lp_core_poll_count(void);

#else

static inline esp_err_t c6_lp_core_arm(int g, bool h) { (void)g; (void)h; return ESP_OK; }
static inline void      c6_lp_core_hibernate_and_wait(void) { }
static inline bool      c6_lp_core_was_motion_wake(void) { return false; }
static inline uint32_t  c6_lp_core_motion_count(void) { return 0; }
static inline uint32_t  c6_lp_core_poll_count(void)   { return 0; }

#endif

#ifdef __cplusplus
}
#endif
