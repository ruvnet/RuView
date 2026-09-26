#include <assert.h>
#include <stdio.h>
#include <string.h>
#include <stdint.h>
#include <stdlib.h>

#include "device_digest_shim.h"
#include "esp_stubs.h"

/* -------------------------------------------------------------
 * Tiny SHA‑256 implementation (public‑domain, minimal). This header
 * provides: void sha256(const uint8_t *data, size_t len, uint8_t out[32]);
 * ------------------------------------------------------------- */

static void run_test(const char *label, const uint8_t mac[6], const char *expected_hex)
{
    char out[17] = {0};
    /* Force the stub to return the supplied MAC and success */
    esp_stub_set_mac(mac, 6);
    esp_stub_set_base_mac_result(ESP_OK);

    device_digest(out);

    if (strcmp(out, expected_hex) != 0) {
        fprintf(stderr, "FAIL: %s – got=%s expected=%s\n", label, out, expected_hex);
        exit(1);
    }
}

static void test_fallback(void)
{
    char out[17] = {0};
    esp_stub_set_base_mac_result(ESP_FAIL);   // simulate failure
    device_digest(out);
    if (strcmp(out, "0000000000000000") != 0) {
        fprintf(stderr, "FAIL: fallback – got=%s expected=%s\n", out, "0000000000000000");
        exit(1);
    }
}

int main(void)
{
    const uint8_t mac_s3[6] = {0x00,0x11,0x22,0x33,0x44,0x55};
    const char *exp_s3 = "a09b14d68b42d074";   // SHA‑256(domain||mac_s3)[0:16]

    const uint8_t mac_c6[6] = {0x20,0x6e,0xf1,0x17,0x27,0x8c};
    const char *exp_c6 = "b7cf79cc7abe0d2b";   // SHA‑256(domain||mac_c6)[0:16]

    run_test("S3 MAC vector", mac_s3, exp_s3);
    run_test("C6 MAC vector", mac_c6, exp_c6);
    test_fallback();

    puts("PASS: device_digest() – all vectors verified");
    return 0;
}
