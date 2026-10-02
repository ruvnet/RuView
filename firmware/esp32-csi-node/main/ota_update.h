/**
 * @file ota_update.h
 * @brief HTTP OTA firmware update endpoint for ESP32-S3 CSI Node.
 *
 * Provides an HTTP server endpoint that accepts firmware binaries
 * for over-the-air updates without physical access to the device.
 */

#ifndef OTA_UPDATE_H
#define OTA_UPDATE_H

#include "esp_err.h"

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

/**
 * Log the httpd task's stack high-water mark (bytes never used). Call from an
 * HTTP handler on the OTA server after its deepest work, so the configured
 * CONFIG_OTA_HTTPD_STACK_SIZE is backed by a measured margin.
 *
 * @param what  Short label for the handler, e.g. "POST /ota".
 */
void ota_update_log_httpd_stack(const char *what);

#endif /* OTA_UPDATE_H */
