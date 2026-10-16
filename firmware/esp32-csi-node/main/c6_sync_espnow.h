/**
 * @file c6_sync_espnow.h
 * @brief ESP-NOW based cross-node time-sync — ADR-110 D1 workaround.
 *
 * After 4 systematic experiments confirmed the 802.15.4 RX path is broken
 * in this user-code + IDF v5.4 combination (see WITNESS-LOG-110 §D1), the
 * cross-node sync claim was unblocked by switching transport from IEEE
 * 802.15.4 to ESP-NOW (WiFi-based peer-to-peer, runs on the same 2.4 GHz
 * radio but uses the WiFi MAC layer that ESP-IDF's 802.11 driver fully
 * supports).
 *
 * Trade vs. 802.15.4:
 *   - Loses the "frees WiFi airtime for CSI" property (uses WiFi for sync)
 *   - Gains a known-working RX path on every ESP32 family
 *   - Same API surface (epoch_us, is_valid, is_leader) so call sites that
 *     used to depend on c6_timesync drop in unchanged
 *
 * Works on both ESP32-S3 and ESP32-C6 — the cross-node sync becomes a
 * cross-target feature, not C6-only.
 */

#pragma once

#ifdef __cplusplus
extern "C" {
#endif

#include "esp_err.h"
#include <stdint.h>
#include <stdbool.h>

/**
 * Initialize the ESP-NOW sync module. Must be called AFTER WiFi STA is
 * connected (ESP-NOW needs the WiFi driver active).
 *
 * @return ESP_OK on success.
 */
esp_err_t c6_sync_espnow_init(void);

/**
 * Returns the synced wall-clock estimate in microseconds.
 * If no leader heard within the timeout, returns the local
 * esp_timer_get_time() value unchanged (offset = 0).
 */
uint64_t c6_sync_espnow_get_epoch_us(void);

bool    c6_sync_espnow_is_leader(void);
bool    c6_sync_espnow_is_valid(void);
int64_t c6_sync_espnow_get_offset_us(void);

/**
 * EMA-smoothed offset (α=1/8, ~8-sample effective window at the 10 Hz
 * beacon rate). Tracks the ≈1.4 ppm crystal drift between two C6 boards
 * (measured in §A0.8) while suppressing the 540 µs per-beacon WiFi-MAC
 * jitter. CSI frame timestamps should stamp from this value, not the raw
 * offset — `c6_sync_espnow_get_epoch_us()` already does so internally.
 */
int64_t c6_sync_espnow_get_offset_us_smoothed(void);

/* Counters for the witness harness — exposed for tests/diagnostics. */
uint32_t c6_sync_espnow_tx_count(void);
uint32_t c6_sync_espnow_tx_fail(void);
uint32_t c6_sync_espnow_rx_count(void);
uint32_t c6_sync_espnow_rx_magic_match(void);

#ifdef __cplusplus
}
uint32_t c6_sync_espnow_period_ms(void);

#endif
