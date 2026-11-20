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

int main(void)
{
    test_hello();
    test_config();
    test_rejections();
    puts("serial onboarding protocol tests passed");
    return 0;
}
