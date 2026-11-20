#ifndef RUVIEW_TEST_ESP_MAC_H
#define RUVIEW_TEST_ESP_MAC_H
#include <stdint.h>
typedef int esp_err_t;
#define ESP_OK 0
esp_err_t esp_base_mac_addr_get(uint8_t *mac);
esp_err_t esp_efuse_mac_get_default(uint8_t *mac);
#endif
