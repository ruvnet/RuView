/**
 * @file wasm_upload.h
 * @brief ADR-040 — HTTP endpoints for WASM module upload and management.
 *
 * Registers endpoints on the existing OTA HTTP server (port 8032):
 *   POST   /wasm/upload   — Upload a .wasm binary (max 128 KB)
 *   GET    /wasm/list      — List loaded modules with status
 *   POST   /wasm/start/:id — Start a loaded module
 *   POST   /wasm/stop/:id  — Stop a running module
 *   DELETE /wasm/:id       — Unload a module
 */

#ifndef WASM_UPLOAD_H
#define WASM_UPLOAD_H

#include "esp_err.h"
#include "esp_http_server.h"

/**
 * Register WASM management HTTP endpoints on the given server.
 *
 * @param server  HTTP server handle (from OTA init).
 * @return ESP_OK on success.
 */
esp_err_t wasm_upload_register(httpd_handle_t server);

#endif /* WASM_UPLOAD_H */
