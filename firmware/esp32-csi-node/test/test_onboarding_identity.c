/* Exercises the production identity boundary with synthetic IDF/PSA seams.
 * This verifies the complete six-byte MAC reaches PSA, not PSA's SHA-256
 * implementation or physical hardware. No replacement hash ships on device. */
#include <assert.h>
#include "onboarding_identity.h"

static uint8_t board_mac[6] = {0x02, 0x10, 0x20, 0x30, 0x40, 0x50};
static int mac_result, init_result, hash_result;
static size_t hash_length = 32;
static unsigned mac_calls, hash_calls;

esp_err_t esp_base_mac_addr_get(uint8_t *mac)
{
    mac_calls++;
    memcpy(mac, board_mac, sizeof(board_mac));
    return mac_result;
}

esp_err_t esp_efuse_mac_get_default(uint8_t *mac)
{
    (void)mac;
    assert(!"EUI-64 API must never receive the six-byte identity buffer");
    return -1;
}

psa_status_t psa_crypto_init(void) { return init_result; }

psa_status_t psa_hash_compute(psa_algorithm_t algorithm,
                            const uint8_t *input, size_t input_length,
                            uint8_t *hash, size_t hash_size, size_t *length)
{
    static const uint8_t domain[] = "ruview-device-v1";
    hash_calls++;
    assert(algorithm == PSA_ALG_SHA_256);
    assert(input_length == sizeof(domain) - 1 + sizeof(board_mac));
    assert(memcmp(input, domain, sizeof(domain) - 1) == 0);
    assert(memcmp(input + sizeof(domain) - 1, board_mac, sizeof(board_mac)) == 0);
    assert(hash_size == 32);
    /* Synthetic digest exposes lost tail bytes and output hex formatting. */
    memset(hash, 0xab, hash_size);
    memcpy(hash, board_mac, sizeof(board_mac));
    *length = hash_length;
    return hash_result;
}

int main(void)
{
    char first[17], second[17];
    ruview_onboarding_device_digest(first);
    assert(strcmp(first, "021020304050abab") == 0);
    board_mac[5]++;
    ruview_onboarding_device_digest(second);
    assert(strcmp(second, "021020304051abab") == 0);
    assert(strcmp(first, second) != 0);
    assert(mac_calls == 2 && hash_calls == 2);

    mac_result = -1;
    ruview_onboarding_device_digest(second);
    assert(strcmp(second, "0000000000000000") == 0 && hash_calls == 2);
    mac_result = 0;
    init_result = -1;
    ruview_onboarding_device_digest(second);
    assert(strcmp(second, "0000000000000000") == 0 && hash_calls == 2);
    init_result = 0;
    hash_result = -1;
    ruview_onboarding_device_digest(second);
    assert(strcmp(second, "0000000000000000") == 0);
    hash_result = 0;
    hash_length = 31;
    ruview_onboarding_device_digest(second);
    assert(strcmp(second, "0000000000000000") == 0);
    puts("PASS: six-byte onboarding identity input and fail-closed output");
    return 0;
}
