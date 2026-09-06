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
#include "esp_http_server.h"  /* httpd_req_t, for ota_auth_check() */

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
 * Validate the Authorization: Bearer <psk> header against the provisioned OTA
 * PSK, in constant time. Fails closed when no PSK is provisioned.
 *
 * Shared with config_api.c so remote configuration is gated by exactly the
 * same secret and the same comparison as firmware upload -- a second
 * implementation would be a second place for the check to rot.
 */
bool ota_auth_check(httpd_req_t *req);

/**
 * Inspect the OTA image state at boot.
 *
 * Detects that the running image is on trial, and detects that the bootloader
 * has already reverted a failed one -- recording why, because a node that
 * quietly reappears on its old firmware is otherwise indistinguishable from
 * one whose update never arrived. Call early in app_main(), after NVS is up.
 */
void ota_rollback_boot_check(void);

/**
 * Report that the node has reached the network, starting the soak after which
 * an image on trial is confirmed. Call from the IP_EVENT_STA_GOT_IP handler.
 */
void ota_rollback_notify_connected(void);

#endif /* OTA_UPDATE_H */
