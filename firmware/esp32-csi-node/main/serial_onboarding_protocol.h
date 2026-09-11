#ifndef SERIAL_ONBOARDING_PROTOCOL_H
#define SERIAL_ONBOARDING_PROTOCOL_H

#include <stddef.h>
#include <stdbool.h>
#include <stdint.h>

#define RUVIEW_ONBOARDING_NONCE_HEX_LEN 32
#define RUVIEW_ONBOARDING_SSID_MAX_BYTES 32
#define RUVIEW_ONBOARDING_PASSWORD_MAX_BYTES 63

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

int ruview_onboarding_parse_hello(const char *line,
                                  char nonce[RUVIEW_ONBOARDING_NONCE_HEX_LEN + 1]);

ruview_onboarding_result_t ruview_onboarding_parse_config(
    const char *line,
    ruview_onboarding_config_t *config);

const char *ruview_onboarding_result_name(ruview_onboarding_result_t result);

#endif
