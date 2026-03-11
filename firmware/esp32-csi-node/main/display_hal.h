/**
 * @file display_hal.h
 * @brief ADR-045: RM67162 QSPI AMOLED + CST816S touch HAL.
 *
 * Hardware abstraction for the LilyGO T-Display-S3 AMOLED panel.
 * Probes hardware at boot; returns ESP_ERR_NOT_FOUND if absent.
 */

#ifndef DISPLAY_HAL_H
#define DISPLAY_HAL_H

#include <stdbool.h>
#include <stdint.h>
#include "esp_err.h"

#ifdef __cplusplus
extern "C" {
#endif

/**
 * Probe and initialize the RM67162 QSPI AMOLED panel.
 *
 * Configures QSPI bus, sends panel init sequence, and fills
 * the screen with dark background to confirm it works.
 * Returns ESP_ERR_NOT_FOUND if the panel does not respond.
 *
 * @return ESP_OK on success, ESP_ERR_NOT_FOUND if no display detected.
 */
esp_err_t display_hal_init_panel(void);

/**
 * Draw a rectangle of pixels to the AMOLED.
 * Sends CASET + RASET + RAMWR directly via QSPI.
 *
 * @param x_start  Left column (inclusive).
 * @param y_start  Top row (inclusive).
 * @param x_end    Right column (exclusive).
 * @param y_end    Bottom row (exclusive).
 * @param color_data  RGB565 pixel data, (x_end-x_start)*(y_end-y_start) pixels.
 */
void display_hal_draw(int x_start, int y_start, int x_end, int y_end,
                      const void *color_data);

/**
 * Probe and initialize the CST816S capacitive touch controller.
 *
 * @return ESP_OK on success, ESP_ERR_NOT_FOUND if no touch IC detected.
 */
esp_err_t display_hal_init_touch(void);

/**
 * Read touch point (non-blocking).
 *
 * @param[out] x  Touch X coordinate (0..535).
 * @param[out] y  Touch Y coordinate (0..239).
 * @return true if touch is active, false if released.
 */
bool display_hal_touch_read(uint16_t *x, uint16_t *y);

/**
 * Set AMOLED brightness via MIPI DCS command.
 *
 * @param percent  Brightness 0-100.
 */
void display_hal_set_brightness(uint8_t percent);

#ifdef __cplusplus
}
#endif

#endif /* DISPLAY_HAL_H */
