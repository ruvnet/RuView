/**
 * @file camera_node.h
 * @brief Onboard camera (XIAO ESP32S3 Sense, OV3660) + HTTP snapshot/MJPEG server.
 *
 * Compiled only when CONFIG_CAMERA_ENABLE=y (see main/CMakeLists.txt).
 * Call camera_node_start() AFTER the network is up. A camera failure is
 * non-fatal: the function logs and returns an error, CSI streaming continues.
 */

#ifndef CAMERA_NODE_H
#define CAMERA_NODE_H

#include "esp_err.h"

#ifdef __cplusplus
extern "C" {
#endif

/**
 * @brief Initialize the camera and start the HTTP server.
 *
 * Endpoints (port CONFIG_CAMERA_HTTP_PORT, default 8081):
 *   GET /snap   — one fresh JPEG (stale framebuffer discarded first)
 *   GET /stream — multipart/x-mixed-replace MJPEG at ~5 fps
 *
 * @return ESP_OK on success; error code if camera init or httpd start failed
 *         (caller should log and continue — CSI operation is unaffected).
 */
esp_err_t camera_node_start(void);

#ifdef __cplusplus
}
#endif

#endif /* CAMERA_NODE_H */
