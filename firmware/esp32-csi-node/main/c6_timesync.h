/**
 * @file c6_timesync.h
 * @brief 802.15.4 mesh time-sync — ADR-110 Phase 4.
 *
 * Provides cross-node clock alignment over a separate 802.15.4 radio so
 * the WiFi airtime stays clean for CSI sensing. Solves the multistatic
 * synchronization problem (ADR-029/030) without burning the sensing
 * channel on coordination traffic.
 *
 * Protocol (skeleton — full Thread join deferred to a follow-up phase):
 *   - One node is elected time-leader (lowest 64-bit EUI on the mesh).
 *   - Leader broadcasts a TS_BEACON every 100 ms on 802.15.4 channel 15.
 *   - Followers compute offset = leader_us - local_us, apply lazily.
 *   - Each CSI frame is stamped with c6_timesync_get_epoch_us().
 *
 * Only built when CONFIG_IDF_TARGET_ESP32C6 + CONFIG_IEEE802154_ENABLED.
 */

#pragma once

#ifdef __cplusplus
extern "C" {
#endif

#include "esp_err.h"
#include <stdint.h>
#include <stdbool.h>

#if (defined(CONFIG_IDF_TARGET_ESP32C6) || defined(CONFIG_IDF_TARGET_ESP32C5)) && defined(CONFIG_IEEE802154_ENABLED)

/**
 * Initialize the 802.15.4 radio and time-sync state machine.
 * Picks leader or follower role based on EUI comparison.
 *
 * @param channel 802.15.4 channel (11-26, default 15).
 * @return ESP_OK on success.
 */
esp_err_t c6_timesync_init(uint8_t channel);

/**
 * Returns the synced wall-clock estimate in microseconds.
 * If no leader heard within the timeout, returns the local
 * esp_timer_get_time() value unchanged (offset = 0).
 */
uint64_t c6_timesync_get_epoch_us(void);

/**
 * Returns true if this node is currently the time-leader.
 */
bool c6_timesync_is_leader(void);

/**
 * Returns true if the local clock is synced (heard a beacon within timeout).
 */
bool c6_timesync_is_valid(void);

/**
 * Returns the most-recently-measured offset from the leader (microseconds).
 * 0 if this node is the leader; sign indicates direction.
 */
int64_t c6_timesync_get_offset_us(void);

#else  /* not C6 with 802.15.4 — provide stubs so call sites compile */

#include "esp_timer.h"

static inline esp_err_t c6_timesync_init(uint8_t c) { (void)c; return ESP_OK; }
static inline uint64_t  c6_timesync_get_epoch_us(void) { return (uint64_t)esp_timer_get_time(); }
static inline bool      c6_timesync_is_leader(void) { return false; }
static inline bool      c6_timesync_is_valid(void) { return false; }
static inline int64_t   c6_timesync_get_offset_us(void) { return 0; }

#endif

#ifdef __cplusplus
}
#endif
