/**
 * @file ota_update.h
 * @brief HTTP OTA firmware update endpoint for ESP32-S3 CSI Node.
 *
 * Provides an HTTP server endpoint that accepts firmware binaries
 * for over-the-air updates without physical access to the device.
 */

#ifndef OTA_UPDATE_H
#define OTA_UPDATE_H

#include <stdbool.h>
#include "esp_err.h"
#include "esp_http_server.h"

/**
 * Verify that an HTTP request carries the configured OTA/WASM management PSK.
 *
 * This is fail-closed: if no PSK is provisioned, management endpoints are
 * unavailable until the device is provisioned with ota_psk in NVS.
 *
 * @param req  HTTP request.
 * @return true when Authorization: Bearer <psk> matches the provisioned PSK.
 */
bool ota_check_auth(httpd_req_t *req);

/**
 * Initialize the OTA update HTTP server.
 * Starts a lightweight HTTP server on port 8032 that accepts
 * POST /ota with a firmware binary payload.
 *
 * @return ESP_OK on success.
 */
esp_err_t ota_update_init(void);

/**
 * Initialize the OTA update HTTP server and return the handle.
 * Same as ota_update_init() but exposes the httpd_handle_t so
 * other modules (e.g. WASM upload) can register additional endpoints.
 *
 * @param out_server  Output: HTTP server handle (may be NULL on failure).
 * @return ESP_OK on success.
 */
esp_err_t ota_update_init_ex(void **out_server);

#endif /* OTA_UPDATE_H */
