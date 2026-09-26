#ifndef ESP_MAC_H
#define ESP_MAC_H

#include <stddef.h>
#include <stdint.h>
#include "esp_err.h"

/**
 * @brief Retrieve the base MAC address of the ESP device.
 *
 * This stub mirrors the ESP-IDF prototype:
 *   esp_err_t esp_base_mac_addr_get(uint8_t mac[6]);
 *
 * The implementation is provided in `esp_stubs.c`.
 */
esp_err_t esp_base_mac_addr_get(uint8_t mac[6]);

#endif // ESP_MAC_H
