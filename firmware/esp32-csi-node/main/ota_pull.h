/**
 * @file ota_pull.h
 * @brief Pull-based OTA client for RuView CSI nodes (#38).
 *
 * Periodically polls the sensing server's firmware registry and
 * applies updates automatically when a newer version is available.
 */

#ifndef OTA_PULL_H
#define OTA_PULL_H

#include "esp_err.h"
#include <stdint.h>

/**
 * Initialize and start the pull-based OTA client task.
 *
 * Creates a FreeRTOS task that:
 *   1. Waits OTA_MIN_UPTIME_SEC (300s) after boot
 *   2. Polls GET /api/v1/firmware/latest every OTA_CHECK_INTERVAL_SEC (300s)
 *   3. Downloads and flashes new firmware when available
 *   4. Verifies SHA256 before rebooting
 *
 * @param server_ip    IPv4 address of the sensing server.
 * @param server_port  HTTP port (typically 4000).
 * @return ESP_OK on success.
 */
esp_err_t ota_pull_init(const char *server_ip, uint16_t server_port);

#endif /* OTA_PULL_H */
