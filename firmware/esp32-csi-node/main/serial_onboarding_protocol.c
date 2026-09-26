#ifndef _POSIX_C_SOURCE
#define _POSIX_C_SOURCE 200809L /* strtok_r under -std=c11 -pedantic on glibc */
#endif
#include "serial_onboarding_protocol.h"

#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define HELLO_COMMAND "RUVIEW_HELLO_V1"
#define CONFIG_COMMAND "RUVIEW_CONFIG_V1"

static bool valid_nonce(const char *value)
{
    if (value == NULL || strlen(value) != RUVIEW_ONBOARDING_NONCE_HEX_LEN) return false;
    for (size_t index = 0; index < RUVIEW_ONBOARDING_NONCE_HEX_LEN; index++) {
        const char c = value[index];
        if (!((c >= '0' && c <= '9') || (c >= 'a' && c <= 'f') ||
              (c >= 'A' && c <= 'F'))) return false;
    }
    return true;
}

static bool valid_digest(const char *value)
{
    if (value == NULL || strlen(value) != RUVIEW_ONBOARDING_DIGEST_HEX_LEN) return false;
    for (size_t index = 0; index < RUVIEW_ONBOARDING_DIGEST_HEX_LEN; index++) {
        const char c = value[index];
        if (!((c >= '0' && c <= '9') || (c >= 'a' && c <= 'f') ||
              (c >= 'A' && c <= 'F'))) return false;
    }
    return true;
}

static bool parse_u32(const char *value, uint32_t maximum, uint32_t *output)
{
    if (value == NULL || value[0] == '\0' || output == NULL) return false;
    uint32_t result = 0;
    for (const char *cursor = value; *cursor != '\0'; cursor++) {
        if (*cursor < '0' || *cursor > '9') return false;
        const uint32_t digit = (uint32_t)(*cursor - '0');
        if (result > (maximum - digit) / 10U) return false;
        result = result * 10U + digit;
    }
    *output = result;
    return true;
}

static bool valid_ipv4(const char *value)
{
    if (value == NULL || value[0] == '\0' || strlen(value) > 15) return false;
    unsigned octets = 0;
    const char *cursor = value;
    while (*cursor != '\0') {
        if (octets == 4) return false;
        if (*cursor == '.') return false;
        uint32_t octet = 0;
        unsigned digits = 0;
        while (*cursor != '\0' && *cursor != '.') {
            if (*cursor < '0' || *cursor > '9' || digits == 3) return false;
            octet = octet * 10U + (uint32_t)(*cursor - '0');
            if (octet > 255U) return false;
            digits++;
            cursor++;
        }
        if (digits == 0) return false;
        octets++;
        if (*cursor == '.') cursor++;
    }
    return octets == 4;
}

static bool valid_private_ipv4(const char *value)
{
    if (!valid_ipv4(value)) return false;
    unsigned first = 0;
    unsigned second = 0;
    if (sscanf(value, "%u.%u", &first, &second) != 2) return false;
    return first == 10U || (first == 172U && second >= 16U && second <= 31U) ||
           (first == 192U && second == 168U);
}

static int base64_value(char c)
{
    if (c >= 'A' && c <= 'Z') return c - 'A';
    if (c >= 'a' && c <= 'z') return c - 'a' + 26;
    if (c >= '0' && c <= '9') return c - '0' + 52;
    if (c == '+') return 62;
    if (c == '/') return 63;
    return -1;
}

static bool decode_base64(const char *encoded, uint8_t *output,
                          size_t output_capacity, size_t *output_length)
{
    if (encoded == NULL || output == NULL || output_length == NULL) return false;
    const size_t length = strlen(encoded);
    if (length == 0 || length % 4 != 0) return false;
    size_t written = 0;
    for (size_t index = 0; index < length; index += 4) {
        int values[4];
        unsigned padding = 0;
        for (unsigned offset = 0; offset < 4; offset++) {
            const char c = encoded[index + offset];
            if (c == '=') {
                if (offset < 2 || index + 4 != length) return false;
                values[offset] = 0;
                padding++;
            } else {
                if (padding != 0) return false;
                values[offset] = base64_value(c);
                if (values[offset] < 0) return false;
            }
        }
        if (padding > 2) return false;
        const uint32_t block = ((uint32_t)values[0] << 18) |
                               ((uint32_t)values[1] << 12) |
                               ((uint32_t)values[2] << 6) |
                               (uint32_t)values[3];
        const unsigned bytes = 3U - padding;
        if (written + bytes > output_capacity) return false;
        output[written++] = (uint8_t)(block >> 16);
        if (bytes > 1) output[written++] = (uint8_t)(block >> 8);
        if (bytes > 2) output[written++] = (uint8_t)block;
    }
    *output_length = written;
    return true;
}

static bool printable_secret(const uint8_t *bytes, size_t length)
{
    if (bytes == NULL) return false;
    for (size_t index = 0; index < length; index++) {
        if (bytes[index] < 0x20 || bytes[index] == 0x7f) return false;
    }
    return true;
}

int ruview_onboarding_parse_hello(
    const char *line,
    char nonce[RUVIEW_ONBOARDING_NONCE_HEX_LEN + 1])
{
    if (line == NULL || nonce == NULL) return 0;
    const size_t prefix_length = strlen(HELLO_COMMAND);
    if (strncmp(line, HELLO_COMMAND, prefix_length) != 0 || line[prefix_length] != ' ')
        return 0;
    const char *candidate = line + prefix_length + 1;
    if (!valid_nonce(candidate)) return 0;
    memcpy(nonce, candidate, RUVIEW_ONBOARDING_NONCE_HEX_LEN + 1);
    return 1;
}

ruview_onboarding_result_t ruview_onboarding_parse_config(
    const char *line,
    ruview_onboarding_config_t *config)
{
    if (line == NULL || config == NULL || strlen(line) >= 384) return RUVIEW_ONBOARDING_ERR_FORMAT;
    char copy[384];
    memcpy(copy, line, strlen(line) + 1);

    char *tokens[7] = {0};
    size_t token_count = 0;
    char *save = NULL;
    for (char *token = strtok_r(copy, " ", &save); token != NULL;
         token = strtok_r(NULL, " ", &save)) {
        if (token_count == 7) return RUVIEW_ONBOARDING_ERR_FORMAT;
        tokens[token_count++] = token;
    }
    if (token_count != 7 || strcmp(tokens[0], CONFIG_COMMAND) != 0)
        return RUVIEW_ONBOARDING_ERR_FORMAT;
    if (!valid_nonce(tokens[1])) return RUVIEW_ONBOARDING_ERR_NONCE;

    uint32_t node_id = 0;
    if (!parse_u32(tokens[2], 255, &node_id) || node_id == 0)
        return RUVIEW_ONBOARDING_ERR_NODE_ID;
    if (!valid_private_ipv4(tokens[3])) return RUVIEW_ONBOARDING_ERR_TARGET_IP;
    uint32_t target_port = 0;
    if (!parse_u32(tokens[4], 65535, &target_port) || target_port == 0)
        return RUVIEW_ONBOARDING_ERR_TARGET_PORT;

    const bool preserve_wifi = strcmp(tokens[5], "KEEP") == 0 &&
                               strcmp(tokens[6], "KEEP") == 0;
    if ((strcmp(tokens[5], "KEEP") == 0) != (strcmp(tokens[6], "KEEP") == 0))
        return RUVIEW_ONBOARDING_ERR_FORMAT;
    uint8_t ssid[RUVIEW_ONBOARDING_SSID_MAX_BYTES];
    size_t ssid_length = 0;
    if (!preserve_wifi && (!decode_base64(tokens[5], ssid, sizeof(ssid), &ssid_length) ||
        ssid_length == 0 || !printable_secret(ssid, ssid_length)))
        return RUVIEW_ONBOARDING_ERR_SSID;
    uint8_t password[RUVIEW_ONBOARDING_PASSWORD_MAX_BYTES];
    size_t password_length = 0;
    if (!preserve_wifi && (!decode_base64(tokens[6], password, sizeof(password), &password_length) ||
        password_length < 8 || !printable_secret(password, password_length)))
        return RUVIEW_ONBOARDING_ERR_PASSWORD;

    memset(config, 0, sizeof(*config));
    memcpy(config->nonce, tokens[1], RUVIEW_ONBOARDING_NONCE_HEX_LEN + 1);
    config->node_id = (uint8_t)node_id;
    memcpy(config->target_ip, tokens[3], strlen(tokens[3]) + 1);
    config->target_port = (uint16_t)target_port;
    config->preserve_wifi = preserve_wifi;
    if (!preserve_wifi) {
        memcpy(config->ssid, ssid, ssid_length);
        config->ssid[ssid_length] = '\0';
        memcpy(config->password, password, password_length);
        config->password[password_length] = '\0';
    }
    return RUVIEW_ONBOARDING_OK;
}

const char *ruview_onboarding_result_name(ruview_onboarding_result_t result)
{
    switch (result) {
    case RUVIEW_ONBOARDING_OK: return "ok";
    case RUVIEW_ONBOARDING_ERR_FORMAT: return "format";
    case RUVIEW_ONBOARDING_ERR_NONCE: return "nonce";
    case RUVIEW_ONBOARDING_ERR_NODE_ID: return "node_id";
    case RUVIEW_ONBOARDING_ERR_TARGET_IP: return "target_ip";
    case RUVIEW_ONBOARDING_ERR_TARGET_PORT: return "target_port";
    case RUVIEW_ONBOARDING_ERR_SSID: return "ssid";
    case RUVIEW_ONBOARDING_ERR_PASSWORD: return "password";
    default: return "unknown";
    }
}

static inline uint32_t ruview_rotr32(uint32_t x, unsigned n)
{
    return (x >> n) | (x << (32 - n));
}

#define SHA256_CH(x, y, z) (((x) & (y)) ^ (~(x) & (z)))
#define SHA256_MAJ(x, y, z) (((x) & (y)) ^ ((x) & (z)) ^ ((y) & (z)))
#define SHA256_EP0(x) (ruview_rotr32(x, 2) ^ ruview_rotr32(x, 13) ^ ruview_rotr32(x, 22))
#define SHA256_EP1(x) (ruview_rotr32(x, 6) ^ ruview_rotr32(x, 11) ^ ruview_rotr32(x, 25))
#define SHA256_SIG0(x) (ruview_rotr32(x, 7) ^ ruview_rotr32(x, 18) ^ ((x) >> 3))
#define SHA256_SIG1(x) (ruview_rotr32(x, 17) ^ ruview_rotr32(x, 19) ^ ((x) >> 10))

static const uint32_t K256[64] = {
    0x428a2f98U, 0x71374491U, 0xb5c0fbcfU, 0xe9b5dba5U, 0x3956c25bU, 0x59f111f1U, 0x923f82a4U, 0xab1c5ed5U,
    0xd807aa98U, 0x12835b01U, 0x243185beU, 0x550c7dc3U, 0x72be5d74U, 0x80deb1feU, 0x9bdc06a7U, 0xc19bf174U,
    0xe49b69c1U, 0xefbe4786U, 0x0fc19dc6U, 0x240ca1ccU, 0x2de92c6fU, 0x4a7484aaU, 0x5cb0a9dcU, 0x76f988daU,
    0x983e5152U, 0xa831c66dU, 0xb00327c8U, 0xbf597fc7U, 0xc6e00bf3U, 0xd5a79147U, 0x06ca6351U, 0x14292967U,
    0x27b70a85U, 0x2e1b2138U, 0x4d2c6dfcU, 0x53380d13U, 0x650a7354U, 0x766a0abbU, 0x81c2c92eU, 0x92722c85U,
    0xa2bfe8a1U, 0xa81a664bU, 0xc24b8b70U, 0xc76c51a3U, 0xd192e819U, 0xd6990624U, 0xf40e3585U, 0x106aa070U,
    0x19a4c116U, 0x1e376c08U, 0x2748774cU, 0x34b0bcb5U, 0x391c0cb3U, 0x4ed8aa4aU, 0x5b9cca4fU, 0x682e6ff3U,
    0x748f82eeU, 0x78a5636fU, 0x84c87814U, 0x8cc70208U, 0x90befffaU, 0xa4506cebU, 0xbef9a3f7U, 0xc67178f2U
};

static void ruview_sha256_transform(uint32_t state[8], const uint8_t data[64])
{
    uint32_t w[64];
    for (int i = 0; i < 16; i++) {
        w[i] = ((uint32_t)data[i * 4] << 24) |
               ((uint32_t)data[i * 4 + 1] << 16) |
               ((uint32_t)data[i * 4 + 2] << 8) |
               ((uint32_t)data[i * 4 + 3]);
    }
    for (int i = 16; i < 64; i++) {
        w[i] = SHA256_SIG1(w[i - 2]) + w[i - 7] + SHA256_SIG0(w[i - 15]) + w[i - 16];
    }
    uint32_t a = state[0];
    uint32_t b = state[1];
    uint32_t c = state[2];
    uint32_t d = state[3];
    uint32_t e = state[4];
    uint32_t f = state[5];
    uint32_t g = state[6];
    uint32_t h = state[7];

    for (int i = 0; i < 64; i++) {
        uint32_t t1 = h + SHA256_EP1(e) + SHA256_CH(e, f, g) + K256[i] + w[i];
        uint32_t t2 = SHA256_EP0(a) + SHA256_MAJ(a, b, c);
        h = g;
        g = f;
        f = e;
        e = d + t1;
        d = c;
        c = b;
        b = a;
        a = t1 + t2;
    }
    state[0] += a;
    state[1] += b;
    state[2] += c;
    state[3] += d;
    state[4] += e;
    state[5] += f;
    state[6] += g;
    state[7] += h;
}

static void ruview_sha256(const uint8_t *data, size_t len, uint8_t digest[32])
{
    uint32_t state[8] = {
        0x6a09e667U, 0xbb67ae85U, 0x3c6ef372U, 0xa54ff53aU,
        0x510e527fU, 0x9b05688cU, 0x1f83d9abU, 0x5be0cd19U
    };
    uint8_t block[64];
    size_t offset = 0;
    while (len >= 64) {
        ruview_sha256_transform(state, data + offset);
        offset += 64;
        len -= 64;
    }
    memset(block, 0, sizeof(block));
    if (len > 0) {
        memcpy(block, data + offset, len);
    }
    block[len] = 0x80;
    uint64_t total_bits = (uint64_t)(offset + len) * 8U;
    if (len >= 56) {
        ruview_sha256_transform(state, block);
        memset(block, 0, sizeof(block));
    }
    for (int i = 0; i < 8; i++) {
        block[63 - i] = (uint8_t)(total_bits >> (i * 8));
    }
    ruview_sha256_transform(state, block);

    for (int i = 0; i < 8; i++) {
        digest[i * 4] = (uint8_t)(state[i] >> 24);
        digest[i * 4 + 1] = (uint8_t)(state[i] >> 16);
        digest[i * 4 + 2] = (uint8_t)(state[i] >> 8);
        digest[i * 4 + 3] = (uint8_t)(state[i]);
    }
}

bool ruview_onboarding_device_digest(const uint8_t *mac, size_t mac_len,
                                    char output[RUVIEW_ONBOARDING_DIGEST_HEX_LEN + 1])
{
    if (output == NULL) return false;
    memcpy(output, RUVIEW_ONBOARDING_DIGEST_FALLBACK, RUVIEW_ONBOARDING_DIGEST_HEX_LEN + 1);

    if (mac == NULL || mac_len != RUVIEW_ONBOARDING_BASE_MAC_LEN) {
        return false;
    }
    bool all_zero = true;
    for (size_t i = 0; i < RUVIEW_ONBOARDING_BASE_MAC_LEN; i++) {
        if (mac[i] != 0) {
            all_zero = false;
            break;
        }
    }
    if (all_zero) return false;

    static const uint8_t domain[] = RUVIEW_ONBOARDING_DIGEST_DOMAIN;
    const size_t domain_len = sizeof(domain) - 1; /* 16 */
    uint8_t input[sizeof(domain) - 1 + RUVIEW_ONBOARDING_BASE_MAC_LEN];
    memcpy(input, domain, domain_len);
    memcpy(input + domain_len, mac, RUVIEW_ONBOARDING_BASE_MAC_LEN);

    uint8_t hash[32];
    ruview_sha256(input, sizeof(input), hash);

    for (size_t i = 0; i < 8; i++) {
        snprintf(output + i * 2, 3, "%02x", hash[i]);
    }
    output[RUVIEW_ONBOARDING_DIGEST_HEX_LEN] = '\0';
    return true;
}

void ruview_onboarding_session_init(ruview_onboarding_session_t *session)
{
    if (session == NULL) return;
    memset(session, 0, sizeof(*session));
}

void ruview_onboarding_session_record_hello(
    ruview_onboarding_session_t *session,
    const char *nonce,
    int64_t current_time_us)
{
    if (session == NULL || nonce == NULL) return;
    memcpy(session->issued_nonce, nonce, RUVIEW_ONBOARDING_NONCE_HEX_LEN);
    session->issued_nonce[RUVIEW_ONBOARDING_NONCE_HEX_LEN] = '\0';
    session->issued_at_us = current_time_us;
}

bool ruview_onboarding_session_validate_claim(
    ruview_onboarding_session_t *session,
    const char *nonce,
    int64_t current_time_us,
    int64_t ttl_us,
    const char **rejection_reason)
{
    if (session == NULL || nonce == NULL) {
        if (rejection_reason) *rejection_reason = "claim_expired";
        return false;
    }
    const int64_t age_us = current_time_us - session->issued_at_us;
    if (session->issued_nonce[0] == '\0' || strcmp(nonce, session->issued_nonce) != 0 ||
        age_us < 0 || age_us > ttl_us) {
        if (rejection_reason) *rejection_reason = "claim_expired";
        return false;
    }
    session->issued_nonce[0] = '\0';
    return true;
}

int ruview_onboarding_format_hello_response(
    char *buffer, size_t buffer_capacity,
    const char *nonce, const char *chip, const char *version,
    uint8_t node_id, const char *target_ip, uint16_t target_port,
    bool configured, const char *device_digest)
{
    if (buffer == NULL || buffer_capacity == 0) return -1;
    int written = snprintf(
        buffer, buffer_capacity,
        "RUVIEW_HELLO_OK_V1 nonce=%s chip=%s version=%s node_id=%u "
        "target_ip=%s target_port=%u configured=%u device_digest=%s",
        nonce ? nonce : "",
        chip ? chip : "unknown",
        version ? version : "0.0.0",
        (unsigned)node_id,
        target_ip ? target_ip : "0.0.0.0",
        (unsigned)target_port,
        configured ? 1U : 0U,
        device_digest ? device_digest : RUVIEW_ONBOARDING_DIGEST_FALLBACK);
    if (written < 0 || (size_t)written >= buffer_capacity) return -1;
    return written;
}

bool ruview_onboarding_parse_hello_response(
    const char *line,
    ruview_onboarding_hello_response_t *response)
{
    if (line == NULL || response == NULL) return false;
    memset(response, 0, sizeof(*response));
    const char *prefix = "RUVIEW_HELLO_OK_V1 ";
    const size_t prefix_len = strlen(prefix);
    if (strncmp(line, prefix, prefix_len) != 0) return false;

    char copy[384];
    if (strlen(line) >= sizeof(copy)) return false;
    memcpy(copy, line + prefix_len, strlen(line + prefix_len) + 1);

    char *save = NULL;
    for (char *tok = strtok_r(copy, " \r\n", &save); tok != NULL;
         tok = strtok_r(NULL, " \r\n", &save)) {
        char *eq = strchr(tok, '=');
        if (eq == NULL) continue;
        *eq = '\0';
        const char *key = tok;
        const char *val = eq + 1;
        if (strcmp(key, "nonce") == 0) {
            strncpy(response->nonce, val, sizeof(response->nonce) - 1);
        } else if (strcmp(key, "chip") == 0) {
            strncpy(response->chip, val, sizeof(response->chip) - 1);
        } else if (strcmp(key, "version") == 0) {
            strncpy(response->version, val, sizeof(response->version) - 1);
        } else if (strcmp(key, "node_id") == 0) {
            response->node_id = (uint8_t)atoi(val);
        } else if (strcmp(key, "target_ip") == 0) {
            strncpy(response->target_ip, val, sizeof(response->target_ip) - 1);
        } else if (strcmp(key, "target_port") == 0) {
            response->target_port = (uint16_t)atoi(val);
        } else if (strcmp(key, "configured") == 0) {
            response->configured = (atoi(val) != 0);
        } else if (strcmp(key, "device_digest") == 0) {
            strncpy(response->device_digest, val, sizeof(response->device_digest) - 1);
        }
    }
    return valid_nonce(response->nonce) && valid_digest(response->device_digest);
}
