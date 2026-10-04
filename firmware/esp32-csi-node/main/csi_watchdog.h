/**
 * @file csi_watchdog.h
 * @brief CSI capture watchdog (RuView#1941).
 *
 * Detects a CSI capture path that has stopped while the node otherwise looks
 * healthy, and walks the csi_stall_policy ladder: re-arm, reassociate, restart.
 */
#ifndef CSI_WATCHDOG_H
#define CSI_WATCHDOG_H

#include <stdint.h>
#include "esp_err.h"

/** Start the watchdog task. Call once, after csi_collector_init(). */
esp_err_t csi_watchdog_start(void);

/** Number of stalls detected since boot. */
uint32_t csi_watchdog_stall_count(void);

/** Number of stalls cleared without a reboot since boot. */
uint32_t csi_watchdog_recovery_count(void);

#endif /* CSI_WATCHDOG_H */
