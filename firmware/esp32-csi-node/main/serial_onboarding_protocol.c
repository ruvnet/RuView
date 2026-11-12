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
        const char *octet_start = cursor;
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
        if (digits > 1 && *octet_start == '0') return false;
        octets++;
        if (*cursor == '.') {
            cursor++;
            if (*cursor == '\0') return false;
        }
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

ruview_onboarding_result_t ruview_onboarding_parse_radar_config(
    const char *line, ruview_onboarding_radar_config_t *request)
{
    if (!line || !request || strlen(line) >= 384) return RUVIEW_ONBOARDING_ERR_FORMAT;
    char copy[384];
    memcpy(copy, line, strlen(line) + 1);
    char *tokens[8] = {0};
    size_t count = 0;
    char *save = NULL;
    for (char *token = strtok_r(copy, " ", &save); token; token = strtok_r(NULL, " ", &save)) {
        if (count == 8) return RUVIEW_ONBOARDING_ERR_FORMAT;
        tokens[count++] = token;
    }
    if (count != 8 || strcmp(tokens[0], "RUVIEW_RADAR_CONFIG_V1") != 0 || strlen(tokens[2]) != 16)
        return RUVIEW_ONBOARDING_ERR_FORMAT;
    for (size_t i = 0; i < 16; i++) {
        if (!((tokens[2][i] >= '0' && tokens[2][i] <= '9') ||
              (tokens[2][i] >= 'a' && tokens[2][i] <= 'f'))) return RUVIEW_ONBOARDING_ERR_FORMAT;
    }
    char generic[384];
    const int length = snprintf(generic, sizeof(generic), "RUVIEW_CONFIG_V1 %s %s %s %s %s %s",
        tokens[1], tokens[3], tokens[4], tokens[5], tokens[6], tokens[7]);
    if (length < 0 || (size_t)length >= sizeof(generic)) return RUVIEW_ONBOARDING_ERR_FORMAT;
    memset(request, 0, sizeof(*request));
    const ruview_onboarding_result_t result = ruview_onboarding_parse_config(generic, &request->config);
    if (result != RUVIEW_ONBOARDING_OK) return result;
    if (request->config.target_port < 1024) return RUVIEW_ONBOARDING_ERR_TARGET_PORT;
    memcpy(request->device_id, tokens[2], sizeof(request->device_id));
    return RUVIEW_ONBOARDING_OK;
}

bool ruview_onboarding_radar_claim_valid(const ruview_onboarding_radar_config_t *request,
    const char *device_id, uint8_t saved_node_id, uint16_t raw_port,
    const char *issued_nonce, int64_t issued_at_us, int64_t now_us)
{
    return request && device_id && issued_nonce && valid_nonce(issued_nonce)
        && strcmp(device_id, "0000000000000000") != 0
        && issued_at_us >= 0 && now_us >= issued_at_us && now_us - issued_at_us <= 60000000LL
        && strcmp(request->device_id, device_id) == 0
        && strcmp(request->config.nonce, issued_nonce) == 0
        && request->config.node_id == saved_node_id && raw_port >= 1024
        && request->config.target_port >= 1024 && request->config.target_port != raw_port;
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
