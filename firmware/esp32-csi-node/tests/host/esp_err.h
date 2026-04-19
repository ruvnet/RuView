/* Host test shim for esp_err.h. Allows us to compile the pure-C
 * portions of the firmware (adaptive_controller_decide, rv_feature_state
 * CRC + finalize) under plain gcc/clang without the ESP-IDF toolchain. */
#ifndef HOST_ESP_ERR_SHIM_H
#define HOST_ESP_ERR_SHIM_H

#include <stdint.h>

typedef int esp_err_t;

#define ESP_OK                   0
#define ESP_FAIL                -1
#define ESP_ERR_NO_MEM         0x101
#define ESP_ERR_INVALID_ARG    0x102
#define ESP_ERR_INVALID_SIZE   0x104
#define ESP_ERR_INVALID_VERSION 0x10A
#define ESP_ERR_INVALID_CRC    0x10B

#endif
