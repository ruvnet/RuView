/**
 * @file power_mgmt.h
 * @brief Power management for battery-powered ESP32-S3 CSI nodes.
 *
 * Implements light sleep between CSI collection bursts to reduce
 * power consumption for battery-powered deployments.
 */

#ifndef POWER_MGMT_H
#define POWER_MGMT_H

#include <stdint.h>
#include "esp_err.h"

/**
 * Initialize power management.
 * Configures automatic light sleep when WiFi is idle.
 *
 * @param duty_cycle_pct  Active duty cycle percentage (10-100).
 *                        100 = always on (default behavior).
 *                        50 = active 50% of the time.
 * @return ESP_OK on success.
 */
esp_err_t power_mgmt_init(uint8_t duty_cycle_pct);

/**
 * Get current power management statistics.
 *
 * @param active_ms     Output: total active time in ms.
 * @param sleep_ms      Output: total sleep time in ms.
 * @param wake_count    Output: number of wake events.
 */
void power_mgmt_stats(uint32_t *active_ms, uint32_t *sleep_ms, uint32_t *wake_count);

#endif /* POWER_MGMT_H */
