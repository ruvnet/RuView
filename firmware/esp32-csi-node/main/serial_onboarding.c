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

static const char *TAG = "serial_onboard";
static nvs_config_t s_current_config;
static ruview_onboarding_session_t s_session;

#define ONBOARDING_LINE_MAX 384
#define ONBOARDING_NONCE_TTL_US RUVIEW_ONBOARDING_NONCE_TTL_US

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

static void device_digest(char output[RUVIEW_ONBOARDING_DIGEST_HEX_LEN + 1])
{
    uint8_t base_mac[RUVIEW_ONBOARDING_BASE_MAC_LEN] = {0};
    const esp_err_t mac_result = esp_base_mac_addr_get(base_mac);
    if (mac_result != ESP_OK ||
        !ruview_onboarding_device_digest(base_mac, sizeof(base_mac), output)) {
        memcpy(output, RUVIEW_ONBOARDING_DIGEST_FALLBACK,
               RUVIEW_ONBOARDING_DIGEST_HEX_LEN + 1);
    }
}

static void emit_hello(const char *nonce)
{
    const esp_app_desc_t *description = esp_app_get_description();
    char digest[RUVIEW_ONBOARDING_DIGEST_HEX_LEN + 1] = {0};
    device_digest(digest);
    const bool configured = s_current_config.wifi_ssid[0] != '\0' &&
                            s_current_config.target_ip[0] != '\0';
    char line[ONBOARDING_LINE_MAX];
    const int len = ruview_onboarding_format_hello_response(
        line, sizeof(line), nonce, chip_name(), description->version,
        s_current_config.node_id, s_current_config.target_ip,
        s_current_config.target_port, configured, digest);
    if (len > 0) {
        printf("%s\n", line);
        fflush(stdout);
    }
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
        ruview_onboarding_session_record_hello(&s_session, nonce, esp_timer_get_time());
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
    const char *rejection_reason = NULL;
    if (!ruview_onboarding_session_validate_claim(
            &s_session, request.nonce, esp_timer_get_time(),
            ONBOARDING_NONCE_TTL_US, &rejection_reason)) {
        printf("RUVIEW_CONFIG_ERR_V1 nonce=%s reason=%s\n",
               request.nonce, rejection_reason ? rejection_reason : "claim_expired");
        fflush(stdout);
        return;
    }
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
