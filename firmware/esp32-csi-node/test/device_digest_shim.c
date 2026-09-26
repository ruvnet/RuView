/**
 * @file device_digest_shim.c
 * @brief Host-test shim: exposes device_digest() as a non-static symbol.
 *
 * Compiles only the device_digest logic from serial_onboarding.c without
 * pulling in the full ESP-IDF networking / NVS / FreeRTOS dependencies.
 *
 * Build with:
 *   gcc -std=c99 -I../main -Itest/stubs \
 *       test/device_digest_shim.c test/stubs/esp_stubs.c -o test_device_digest
 */

#include <stdint.h>
#include <string.h>
#include <stdio.h>

#include "esp_stubs.h"   /* gives esp_err_t, ESP_OK, esp_base_mac_addr_get */
#include "psa/crypto.h"  /* gives psa_crypto_init, psa_hash_compute, PSA_ALG_SHA_256 */

/* Expose device_digest as a non-static function for the test harness */
void device_digest(char output[17])
{
    uint8_t base_mac[6] = {0};
    uint8_t digest[32] = {0};
    static const uint8_t domain[] = "ruview-device-v1";
    uint8_t digest_input[sizeof(domain) - 1 + sizeof(base_mac)];
    size_t digest_length = 0;
    memcpy(digest_input, domain, sizeof(domain) - 1);
    const esp_err_t mac_result = esp_base_mac_addr_get(base_mac);
    if (mac_result == ESP_OK) {
        memcpy(digest_input + sizeof(domain) - 1, base_mac, sizeof(base_mac));
    }
    if (mac_result == ESP_OK &&
        psa_crypto_init() == PSA_SUCCESS &&
        psa_hash_compute(PSA_ALG_SHA_256, digest_input, sizeof(digest_input),
                         digest, sizeof(digest), &digest_length) == PSA_SUCCESS &&
        digest_length == sizeof(digest)) {
        for (size_t index = 0; index < 8; index++) {
            snprintf(output + index * 2, 3, "%02x", digest[index]);
        }
    } else {
        memcpy(output, "0000000000000000", 17);
    }
}
