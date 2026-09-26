#include <assert.h>
#include <stdio.h>
#include <string.h>

#include "../main/serial_onboarding_protocol.h"

static void test_hello(void)
{
    char nonce[33] = {0};
    assert(ruview_onboarding_parse_hello(
        "RUVIEW_HELLO_V1 00112233445566778899aabbccddeeff", nonce));
    assert(strcmp(nonce, "00112233445566778899aabbccddeeff") == 0);
    assert(!ruview_onboarding_parse_hello("RUVIEW_HELLO_V1 bad", nonce));
    assert(!ruview_onboarding_parse_hello(
        "RUVIEW_HELLO_V1 00112233445566778899aabbccddeeff extra", nonce));
}

static void test_config(void)
{
    ruview_onboarding_config_t config;
    assert(ruview_onboarding_parse_config(
        "RUVIEW_CONFIG_V1 00112233445566778899aabbccddeeff 5 192.168.1.166 5005 "
        "UnVWaWV3IExhYg== U3Ryb25nUGFzc3dvcmQ=", &config) == RUVIEW_ONBOARDING_OK);
    assert(config.node_id == 5);
    assert(config.target_port == 5005);
    assert(strcmp(config.target_ip, "192.168.1.166") == 0);
    assert(strcmp(config.ssid, "RuView Lab") == 0);
    assert(strcmp(config.password, "StrongPassword") == 0);
    assert(!config.preserve_wifi);
    assert(ruview_onboarding_parse_config(
        "RUVIEW_CONFIG_V1 00112233445566778899aabbccddeeff 6 192.168.1.166 5005 KEEP KEEP",
        &config) == RUVIEW_ONBOARDING_OK);
    assert(config.preserve_wifi);
}

static void test_rejections(void)
{
    ruview_onboarding_config_t config;
    const char *prefix = "RUVIEW_CONFIG_V1 00112233445566778899aabbccddeeff";
    char line[512];
    snprintf(line, sizeof(line), "%s 0 192.168.1.2 5005 UnVWaWV3 U3Ryb25nUGFzc3dvcmQ=", prefix);
    assert(ruview_onboarding_parse_config(line, &config) == RUVIEW_ONBOARDING_ERR_NODE_ID);
    snprintf(line, sizeof(line), "%s 5 999.1.1.1 5005 UnVWaWV3 U3Ryb25nUGFzc3dvcmQ=", prefix);
    assert(ruview_onboarding_parse_config(line, &config) == RUVIEW_ONBOARDING_ERR_TARGET_IP);
    snprintf(line, sizeof(line), "%s 5 8.8.8.8 5005 UnVWaWV3 U3Ryb25nUGFzc3dvcmQ=", prefix);
    assert(ruview_onboarding_parse_config(line, &config) == RUVIEW_ONBOARDING_ERR_TARGET_IP);
    snprintf(line, sizeof(line), "%s 5 172.32.0.1 5005 UnVWaWV3 U3Ryb25nUGFzc3dvcmQ=", prefix);
    assert(ruview_onboarding_parse_config(line, &config) == RUVIEW_ONBOARDING_ERR_TARGET_IP);
    snprintf(line, sizeof(line), "%s 5 192.168.1.2 0 UnVWaWV3 U3Ryb25nUGFzc3dvcmQ=", prefix);
    assert(ruview_onboarding_parse_config(line, &config) == RUVIEW_ONBOARDING_ERR_TARGET_PORT);
    snprintf(line, sizeof(line), "%s 5 192.168.1.2 5005 !!!! U3Ryb25nUGFzc3dvcmQ=", prefix);
    assert(ruview_onboarding_parse_config(line, &config) == RUVIEW_ONBOARDING_ERR_SSID);
    snprintf(line, sizeof(line), "%s 5 192.168.1.2 5005 UnVWaWV3 c2hvcnQ=", prefix);
    assert(ruview_onboarding_parse_config(line, &config) == RUVIEW_ONBOARDING_ERR_PASSWORD);
    snprintf(line, sizeof(line), "%s 5 192.168.1.2 5005 UnVWaWV3 UGFzc3dvcmQxCg==", prefix);
    assert(ruview_onboarding_parse_config(line, &config) == RUVIEW_ONBOARDING_ERR_PASSWORD);
    snprintf(line, sizeof(line), "%s 5 192.168.1.2 5005 KEEP U3Ryb25nUGFzc3dvcmQ=", prefix);
    assert(ruview_onboarding_parse_config(line, &config) == RUVIEW_ONBOARDING_ERR_FORMAT);
}

static void test_device_digest_vectors(void)
{
    char digest[RUVIEW_ONBOARDING_DIGEST_HEX_LEN + 1];

    /* 1. ESP32-C6 hardware ground-truth vector from physical device (14:c1:9f:e0:7b:60) */
    const uint8_t c6_hw_mac[6] = {0x14, 0xc1, 0x9f, 0xe0, 0x7b, 0x60};
    assert(ruview_onboarding_device_digest(c6_hw_mac, sizeof(c6_hw_mac), digest));
    assert(strcmp(digest, "b4976b95e0186806") == 0);

    /* 2. Regression witness vector: verify old buggy truncated EUI-64 prefix produced different digest */
    const uint8_t c6_buggy_eui64_trunc[6] = {0x14, 0xc1, 0x9f, 0xff, 0xfe, 0xe0};
    assert(ruview_onboarding_device_digest(c6_buggy_eui64_trunc, sizeof(c6_buggy_eui64_trunc), digest));
    assert(strcmp(digest, "4bf156cd7773c5e0") == 0);

    /* 3. ESP32-S3 test vector (7c:df:a1:10:20:30) */
    const uint8_t s3_mac[6] = {0x7c, 0xdf, 0xa1, 0x10, 0x20, 0x30};
    assert(ruview_onboarding_device_digest(s3_mac, sizeof(s3_mac), digest));
    assert(strcmp(digest, "0d8b795c3599af0f") == 0);

    /* 4. ESP32-H2 test vector (60:55:f9:12:34:56) */
    const uint8_t h2_mac[6] = {0x60, 0x55, 0xf9, 0x12, 0x34, 0x56};
    assert(ruview_onboarding_device_digest(h2_mac, sizeof(h2_mac), digest));
    assert(strcmp(digest, "8d1f61c922180ffd") == 0);

    /* 5. Production Lot Uniqueness: Adjacent ESP32-C6 devices in same manufacturing lot */
    const uint8_t lot_mac_a[6] = {0x14, 0xc1, 0x9f, 0xe0, 0x7b, 0x61};
    const uint8_t lot_mac_b[6] = {0x14, 0xc1, 0x9f, 0xe0, 0x7b, 0x62};
    char digest_a[RUVIEW_ONBOARDING_DIGEST_HEX_LEN + 1];
    char digest_b[RUVIEW_ONBOARDING_DIGEST_HEX_LEN + 1];

    assert(ruview_onboarding_device_digest(lot_mac_a, sizeof(lot_mac_a), digest_a));
    assert(ruview_onboarding_device_digest(lot_mac_b, sizeof(lot_mac_b), digest_b));
    assert(strcmp(digest_a, "cd6bfacef808c7fe") == 0);
    assert(strcmp(digest_b, "c5e4f9e843d1b28c") == 0);

    /* Assert pairwise uniqueness across all devices in lot */
    assert(strcmp(digest, digest_a) != 0);
    assert(strcmp(digest, digest_b) != 0);
    assert(strcmp(digest_a, digest_b) != 0);
}

static void test_device_digest_width_and_fallback(void)
{
    char digest[RUVIEW_ONBOARDING_DIGEST_HEX_LEN + 1];

    /* 1. NULL pointer fallback */
    memset(digest, 'X', sizeof(digest));
    assert(!ruview_onboarding_device_digest(NULL, 6, digest));
    assert(strcmp(digest, RUVIEW_ONBOARDING_DIGEST_FALLBACK) == 0);

    /* 2. All-zeros MAC fallback */
    const uint8_t all_zeros[6] = {0x00, 0x00, 0x00, 0x00, 0x00, 0x00};
    memset(digest, 'X', sizeof(digest));
    assert(!ruview_onboarding_device_digest(all_zeros, sizeof(all_zeros), digest));
    assert(strcmp(digest, RUVIEW_ONBOARDING_DIGEST_FALLBACK) == 0);

    /* 3. Width enforcement: 8-byte EUI-64 buffer MUST be rejected */
    const uint8_t eui64_mac[8] = {0x14, 0xc1, 0x9f, 0xff, 0xfe, 0xe0, 0x7b, 0x60};
    memset(digest, 'X', sizeof(digest));
    assert(!ruview_onboarding_device_digest(eui64_mac, sizeof(eui64_mac), digest));
    assert(strcmp(digest, RUVIEW_ONBOARDING_DIGEST_FALLBACK) == 0);

    /* 4. Truncated widths (< 6 bytes) MUST be rejected */
    const uint8_t short_mac[5] = {0x14, 0xc1, 0x9f, 0xe0, 0x7b};
    memset(digest, 'X', sizeof(digest));
    assert(!ruview_onboarding_device_digest(short_mac, sizeof(short_mac), digest));
    assert(strcmp(digest, RUVIEW_ONBOARDING_DIGEST_FALLBACK) == 0);

    /* 5. Zero length MUST be rejected */
    memset(digest, 'X', sizeof(digest));
    assert(!ruview_onboarding_device_digest(eui64_mac, 0, digest));
    assert(strcmp(digest, RUVIEW_ONBOARDING_DIGEST_FALLBACK) == 0);
}

static void test_onboarding_session_lifecycle_and_retry(void)
{
    ruview_onboarding_session_t session;
    const char *reason = NULL;
    const char *nonce1 = "0123456789abcdef0123456789abcdef";
    const char *nonce2 = "fedcba9876543210fedcba9876543210";
    const int64_t t0 = 1000000000LL; /* 1000 seconds */

    ruview_onboarding_session_init(&session);

    /* Claim without an active hello challenge must fail */
    assert(!ruview_onboarding_session_validate_claim(
        &session, nonce1, t0, RUVIEW_ONBOARDING_NONCE_TTL_US, &reason));
    assert(reason != NULL && strcmp(reason, "claim_expired") == 0);

    /* Issue HELLO at t0 */
    ruview_onboarding_session_record_hello(&session, nonce1, t0);

    /* Nonce mismatch must be rejected */
    reason = NULL;
    assert(!ruview_onboarding_session_validate_claim(
        &session, nonce2, t0 + 1000000LL, RUVIEW_ONBOARDING_NONCE_TTL_US, &reason));
    assert(reason != NULL && strcmp(reason, "claim_expired") == 0);

    /* Valid claim within TTL (e.g. 5 seconds after hello) */
    reason = NULL;
    assert(ruview_onboarding_session_validate_claim(
        &session, nonce1, t0 + 5000000LL, RUVIEW_ONBOARDING_NONCE_TTL_US, &reason));
    assert(reason == NULL);

    /* Replay attack: second claim with same consumed nonce must fail */
    reason = NULL;
    assert(!ruview_onboarding_session_validate_claim(
        &session, nonce1, t0 + 6000000LL, RUVIEW_ONBOARDING_NONCE_TTL_US, &reason));
    assert(reason != NULL && strcmp(reason, "claim_expired") == 0);

    /* Session expiration test: Issue new hello at t1 */
    const int64_t t1 = 2000000000LL;
    ruview_onboarding_session_record_hello(&session, nonce2, t1);

    /* Try to claim at t1 + 61 seconds (> 60s TTL) */
    reason = NULL;
    assert(!ruview_onboarding_session_validate_claim(
        &session, nonce2, t1 + 61000000LL, RUVIEW_ONBOARDING_NONCE_TTL_US, &reason));
    assert(reason != NULL && strcmp(reason, "claim_expired") == 0);

    /* Retry flow: Client re-issues HELLO after timeout at t2 */
    const int64_t t2 = t1 + 65000000LL;
    const char *nonce3 = "11223344556677889900aabbccddeeff";
    ruview_onboarding_session_record_hello(&session, nonce3, t2);

    /* Retry claim 2 seconds after retry HELLO succeeds */
    reason = NULL;
    assert(ruview_onboarding_session_validate_claim(
        &session, nonce3, t2 + 2000000LL, RUVIEW_ONBOARDING_NONCE_TTL_US, &reason));
    assert(reason == NULL);
}

static void test_hello_response_formatting_and_parsing(void)
{
    char line_buf[256];
    ruview_onboarding_hello_response_t parsed;

    int written = ruview_onboarding_format_hello_response(
        line_buf, sizeof(line_buf),
        "00112233445566778899aabbccddeeff",
        "esp32c6",
        "0.8.12",
        5,
        "192.168.1.166",
        5005,
        true,
        "b4976b95e0186806");

    assert(written > 0);
    assert(strcmp(line_buf,
        "RUVIEW_HELLO_OK_V1 nonce=00112233445566778899aabbccddeeff chip=esp32c6 version=0.8.12 node_id=5 target_ip=192.168.1.166 target_port=5005 configured=1 device_digest=b4976b95e0186806") == 0);

    /* Parse back from wire line */
    assert(ruview_onboarding_parse_hello_response(line_buf, &parsed));
    assert(strcmp(parsed.nonce, "00112233445566778899aabbccddeeff") == 0);
    assert(strcmp(parsed.chip, "esp32c6") == 0);
    assert(strcmp(parsed.version, "0.8.12") == 0);
    assert(parsed.node_id == 5);
    assert(strcmp(parsed.target_ip, "192.168.1.166") == 0);
    assert(parsed.target_port == 5005);
    assert(parsed.configured == true);
    assert(strcmp(parsed.device_digest, "b4976b95e0186806") == 0);

    /* Anti-swap guard simulation: desktop installer verifying physical identity */
    const char *expected_digest = "b4976b95e0186806";
    const char *wrong_board_digest = "cd6bfacef808c7fe";
    assert(strcmp(parsed.device_digest, expected_digest) == 0);
    assert(strcmp(parsed.device_digest, wrong_board_digest) != 0);

    /* Test parsing with trailing carriage return / newline */
    assert(ruview_onboarding_parse_hello_response(
        "RUVIEW_HELLO_OK_V1 nonce=00112233445566778899aabbccddeeff chip=esp32s3 version=0.8.12 node_id=0 target_ip=0.0.0.0 target_port=0 configured=0 device_digest=0d8b795c3599af0f\r\n",
        &parsed));
    assert(strcmp(parsed.chip, "esp32s3") == 0);
    assert(parsed.node_id == 0);
    assert(parsed.configured == false);
    assert(strcmp(parsed.device_digest, "0d8b795c3599af0f") == 0);

    /* Malformed hello responses */
    assert(!ruview_onboarding_parse_hello_response(
        "RUVIEW_HELLO_OK_V1 nonce=00112233445566778899aabbccddeeff chip=esp32c6", &parsed));
    assert(!ruview_onboarding_parse_hello_response(
        "RUVIEW_UNKNOWN_V1 nonce=00112233445566778899aabbccddeeff chip=esp32c6 version=0.8.12 node_id=5 target_ip=192.168.1.166 target_port=5005 configured=1 device_digest=b4976b95e0186806", &parsed));
    /* Invalid digest length (too short) */
    assert(!ruview_onboarding_parse_hello_response(
        "RUVIEW_HELLO_OK_V1 nonce=00112233445566778899aabbccddeeff chip=esp32c6 version=0.8.12 node_id=5 target_ip=192.168.1.166 target_port=5005 configured=1 device_digest=1234", &parsed));
}

int main(void)
{
    test_hello();
    test_config();
    test_rejections();
    test_device_digest_vectors();
    test_device_digest_width_and_fallback();
    test_onboarding_session_lifecycle_and_retry();
    test_hello_response_formatting_and_parsing();
    puts("serial onboarding protocol tests passed");
    return 0;
}
