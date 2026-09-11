#include "serial_onboarding.h"

#include "serial_onboarding_protocol.h"

#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

#include "esp_app_desc.h"
#include "esp_efuse.h"
#include "esp_log.h"
#include "esp_mac.h"
#include "esp_system.h"
#include "esp_timer.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "nvs.h"
#include "psa/crypto.h"

static const char *TAG = "serial_onboard";
static nvs_config_t s_current_config;
static char s_issued_nonce[RUVIEW_ONBOARDING_NONCE_HEX_LEN + 1];
static int64_t s_nonce_issued_at_us;

#define ONBOARDING_LINE_MAX 384
#define ONBOARDING_NONCE_TTL_US (60LL * 1000LL * 1000LL)

static const char *chip_name(void)
{
#if defined(CONFIG_IDF_TARGET_ESP32C6)
    return "esp32c6";
#elif defined(CONFIG_IDF_TARGET_ESP32S3)
    return "esp32s3";
#else
    return "esp32";
#endif
}

static void device_digest(char output[17])
{
    uint8_t base_mac[6] = {0};
    uint8_t digest[32] = {0};
    static const uint8_t domain[] = "ruview-device-v1";
    uint8_t digest_input[sizeof(domain) - 1 + sizeof(base_mac)];
    size_t digest_length = 0;
    memcpy(digest_input, domain, sizeof(domain) - 1);
    const esp_err_t mac_result = esp_efuse_mac_get_default(base_mac);
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

static void emit_hello(const char *nonce)
{
    const esp_app_desc_t *description = esp_app_get_description();
    char digest[17] = {0};
    device_digest(digest);
    const bool configured = s_current_config.wifi_ssid[0] != '\0' &&
                            s_current_config.target_ip[0] != '\0';
    printf("RUVIEW_HELLO_OK_V1 nonce=%s chip=%s version=%s node_id=%u "
           "target_ip=%s target_port=%u configured=%u device_digest=%s\n",
           nonce, chip_name(), description->version,
           (unsigned)s_current_config.node_id, s_current_config.target_ip,
           (unsigned)s_current_config.target_port, configured ? 1U : 0U, digest);
    fflush(stdout);
}

static esp_err_t commit_config(const ruview_onboarding_config_t *config)
{
    nvs_handle_t handle;
    esp_err_t result = nvs_open("csi_cfg", NVS_READWRITE, &handle);
    if (result != ESP_OK) return result;
    if (!config->preserve_wifi) {
        result = nvs_set_str(handle, "ssid", config->ssid);
        if (result == ESP_OK) result = nvs_set_str(handle, "password", config->password);
    }
    if (result == ESP_OK) result = nvs_set_str(handle, "target_ip", config->target_ip);
    if (result == ESP_OK) result = nvs_set_u16(handle, "target_port", config->target_port);
    if (result == ESP_OK) result = nvs_set_u8(handle, "node_id", config->node_id);
    if (result == ESP_OK) result = nvs_commit(handle);
    nvs_close(handle);
    return result;
}

static void process_line(char *line)
{
    char nonce[RUVIEW_ONBOARDING_NONCE_HEX_LEN + 1] = {0};
    if (ruview_onboarding_parse_hello(line, nonce)) {
        memcpy(s_issued_nonce, nonce, sizeof(s_issued_nonce));
        s_nonce_issued_at_us = esp_timer_get_time();
        emit_hello(nonce);
        return;
    }
    if (strncmp(line, "RUVIEW_CONFIG_V1 ", 17) != 0) return;

    ruview_onboarding_config_t request;
    const ruview_onboarding_result_t parsed = ruview_onboarding_parse_config(line, &request);
    if (parsed != RUVIEW_ONBOARDING_OK) {
        printf("RUVIEW_CONFIG_ERR_V1 reason=%s\n", ruview_onboarding_result_name(parsed));
        fflush(stdout);
        return;
    }
    const int64_t age_us = esp_timer_get_time() - s_nonce_issued_at_us;
    if (s_issued_nonce[0] == '\0' || strcmp(request.nonce, s_issued_nonce) != 0 ||
        age_us < 0 || age_us > ONBOARDING_NONCE_TTL_US) {
        printf("RUVIEW_CONFIG_ERR_V1 nonce=%s reason=claim_expired\n", request.nonce);
        fflush(stdout);
        return;
    }
    s_issued_nonce[0] = '\0';
    const esp_err_t result = commit_config(&request);
    if (result != ESP_OK) {
        printf("RUVIEW_CONFIG_ERR_V1 nonce=%s reason=nvs_commit code=%s\n",
               request.nonce, esp_err_to_name(result));
        fflush(stdout);
        return;
    }

    printf("RUVIEW_CONFIG_OK_V1 nonce=%s node_id=%u rebooting=1\n",
           request.nonce, (unsigned)request.node_id);
    fflush(stdout);
    vTaskDelay(pdMS_TO_TICKS(400));
    esp_restart();
}

static void consume_bytes(const uint8_t *input, ssize_t count, char *line,
                          size_t *length, bool *overflow)
{
    for (ssize_t index = 0; index < count; index++) {
        const uint8_t byte = input[index];
        if (byte == '\r') continue;
        if (byte == '\n') {
            if (!*overflow && *length > 0) {
                line[*length] = '\0';
                process_line(line);
            }
            *length = 0;
            *overflow = false;
            continue;
        }
        if (byte < 0x20 || byte == 0x7f) continue;
        if (*length + 1 < ONBOARDING_LINE_MAX) line[(*length)++] = (char)byte;
        else *overflow = true;
    }
}

static void serial_onboarding_task(void *argument)
{
    (void)argument;
    const int original_flags = fcntl(STDIN_FILENO, F_GETFL, 0);
    if (original_flags >= 0) (void)fcntl(STDIN_FILENO, F_SETFL, original_flags | O_NONBLOCK);
    /* ESP-IDF secondary consoles mirror output but deliberately do not forward
     * input to STDIN. Native S3/C6 USB Serial/JTAG therefore needs a bounded
     * direct VFS reader while UART-console boards continue to use STDIN. */
    const int usb_fd = open("/dev/secondary", O_RDONLY | O_NONBLOCK);
    char primary_line[ONBOARDING_LINE_MAX];
    char usb_line[ONBOARDING_LINE_MAX];
    size_t primary_length = 0;
    size_t usb_length = 0;
    bool primary_overflow = false;
    bool usb_overflow = false;
    for (;;) {
        uint8_t input[64];
        const ssize_t count = read(STDIN_FILENO, input, sizeof(input));
        if (count > 0) {
            consume_bytes(input, count, primary_line, &primary_length, &primary_overflow);
        }
        else if (count < 0 && errno != EAGAIN && errno != EWOULDBLOCK) {
            ESP_LOGW(TAG, "serial input unavailable: errno=%d", errno);
            vTaskDelay(pdMS_TO_TICKS(500));
        }
        if (usb_fd >= 0) {
            const ssize_t usb_count = read(usb_fd, input, sizeof(input));
            if (usb_count > 0) {
                consume_bytes(input, usb_count, usb_line, &usb_length, &usb_overflow);
            }
        }
        vTaskDelay(pdMS_TO_TICKS(20));
    }
}

esp_err_t serial_onboarding_start(const nvs_config_t *config)
{
    if (config == NULL) return ESP_ERR_INVALID_ARG;
    memcpy(&s_current_config, config, sizeof(s_current_config));
    const BaseType_t created = xTaskCreate(serial_onboarding_task, "serial_onboard",
                                           4096, NULL, 4, NULL);
    if (created != pdPASS) return ESP_ERR_NO_MEM;
    ESP_LOGI(TAG, "bounded serial onboarding ready");
    return ESP_OK;
}
