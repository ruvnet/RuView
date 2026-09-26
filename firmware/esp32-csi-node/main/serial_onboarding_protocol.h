#ifndef SERIAL_ONBOARDING_PROTOCOL_H
#define SERIAL_ONBOARDING_PROTOCOL_H

#include <stddef.h>
#include <stdbool.h>
#include <stdint.h>

#define RUVIEW_ONBOARDING_NONCE_HEX_LEN 32
#define RUVIEW_ONBOARDING_SSID_MAX_BYTES 32
#define RUVIEW_ONBOARDING_PASSWORD_MAX_BYTES 63
#define RUVIEW_ONBOARDING_DIGEST_HEX_LEN 16
#define RUVIEW_ONBOARDING_BASE_MAC_LEN 6
#define RUVIEW_ONBOARDING_DIGEST_DOMAIN "ruview-device-v1"
#define RUVIEW_ONBOARDING_DIGEST_FALLBACK "0000000000000000"
#define RUVIEW_ONBOARDING_NONCE_TTL_US (60LL * 1000LL * 1000LL)

typedef struct {
    char nonce[RUVIEW_ONBOARDING_NONCE_HEX_LEN + 1];
    uint8_t node_id;
    char target_ip[16];
    uint16_t target_port;
    bool preserve_wifi;
    char ssid[RUVIEW_ONBOARDING_SSID_MAX_BYTES + 1];
    char password[RUVIEW_ONBOARDING_PASSWORD_MAX_BYTES + 1];
} ruview_onboarding_config_t;

typedef enum {
    RUVIEW_ONBOARDING_OK = 0,
    RUVIEW_ONBOARDING_ERR_FORMAT,
    RUVIEW_ONBOARDING_ERR_NONCE,
    RUVIEW_ONBOARDING_ERR_NODE_ID,
    RUVIEW_ONBOARDING_ERR_TARGET_IP,
    RUVIEW_ONBOARDING_ERR_TARGET_PORT,
    RUVIEW_ONBOARDING_ERR_SSID,
    RUVIEW_ONBOARDING_ERR_PASSWORD,
} ruview_onboarding_result_t;

typedef struct {
    char issued_nonce[RUVIEW_ONBOARDING_NONCE_HEX_LEN + 1];
    int64_t issued_at_us;
} ruview_onboarding_session_t;

typedef struct {
    char nonce[RUVIEW_ONBOARDING_NONCE_HEX_LEN + 1];
    char chip[16];
    char version[32];
    uint8_t node_id;
    char target_ip[16];
    uint16_t target_port;
    bool configured;
    char device_digest[RUVIEW_ONBOARDING_DIGEST_HEX_LEN + 1];
} ruview_onboarding_hello_response_t;

/**
 * Compute the 16-hex device digest from a 6-byte base MAC address.
 *
 * Formula: SHA-256("ruview-device-v1" || mac)[0..8] formatted as 16 lowercase hex chars + NUL.
 *
 * Seam & Validation Contracts:
 *   - mac must be non-NULL.
 *   - mac_len must equal RUVIEW_ONBOARDING_BASE_MAC_LEN (6 bytes).
 *     Rejects 8-byte EUI-64 or truncated/overflow widths.
 *   - mac must not be all zeros (00:00:00:00:00:00).
 *   - output must point to a buffer of at least 17 bytes (16 hex chars + NUL).
 *
 * Returns:
 *   true on success.
 *   false on any validation failure, safely populating output with
 *         RUVIEW_ONBOARDING_DIGEST_FALLBACK ("0000000000000000").
 */
bool ruview_onboarding_device_digest(const uint8_t *mac, size_t mac_len,
                                    char output[RUVIEW_ONBOARDING_DIGEST_HEX_LEN + 1]);

void ruview_onboarding_session_init(ruview_onboarding_session_t *session);

void ruview_onboarding_session_record_hello(
    ruview_onboarding_session_t *session,
    const char *nonce,
    int64_t current_time_us);

bool ruview_onboarding_session_validate_claim(
    ruview_onboarding_session_t *session,
    const char *nonce,
    int64_t current_time_us,
    int64_t ttl_us,
    const char **rejection_reason);

int ruview_onboarding_format_hello_response(
    char *buffer, size_t buffer_capacity,
    const char *nonce, const char *chip, const char *version,
    uint8_t node_id, const char *target_ip, uint16_t target_port,
    bool configured, const char *device_digest);

bool ruview_onboarding_parse_hello_response(
    const char *line,
    ruview_onboarding_hello_response_t *response);

int ruview_onboarding_parse_hello(const char *line,
                                  char nonce[RUVIEW_ONBOARDING_NONCE_HEX_LEN + 1]);

ruview_onboarding_result_t ruview_onboarding_parse_config(
    const char *line,
    ruview_onboarding_config_t *config);

const char *ruview_onboarding_result_name(ruview_onboarding_result_t result);

#endif
