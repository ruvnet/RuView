#ifndef RUVIEW_ONBOARDING_IDENTITY_H
#define RUVIEW_ONBOARDING_IDENTITY_H

#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include "esp_mac.h"
#include "psa/crypto.h"

/* Shared with the host regression test; hashing remains the IDF PSA API.
 * esp_efuse_mac_get_default writes an eight-byte EUI-64 on C6/H2. The base
 * MAC API always returns the six-byte identity required by ADR-363. */
static inline void ruview_onboarding_device_digest(char output[17])
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

#endif
