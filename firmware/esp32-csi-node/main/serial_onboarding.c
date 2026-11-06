#include "serial_onboarding.h"

#include "serial_onboarding_protocol.h"
#include "onboarding_identity.h"
#include "onboarding_console.h"

#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdarg.h>
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

#define ONBOARDING_LINE_MAX 384
#define ONBOARDING_NONCE_TTL_US (60LL * 1000LL * 1000LL)

typedef struct {
    bool native_usb;
    char issued_nonce[RUVIEW_ONBOARDING_NONCE_HEX_LEN + 1];
    int64_t nonce_issued_at_us;
    char line[ONBOARDING_LINE_MAX];
    size_t length;
    bool overflow;
} onboarding_endpoint_t;

static bool reply(onboarding_endpoint_t *endpoint, const char *format, ...)
{
    char output[512];
    va_list arguments;
    va_start(arguments, format);
    const int count = vsnprintf(output, sizeof(output), format, arguments);
    va_end(arguments);
    if (count < 0 || (size_t)count >= sizeof(output)) return false;
    if (endpoint->native_usb) return ruview_onboarding_usb_write(output, (size_t)count);
    const bool written = fwrite(output, 1, (size_t)count, stdout) == (size_t)count;
    return fflush(stdout) == 0 && written;
}

static const char *chip_name(void)
{
#if defined(CONFIG_IDF_TARGET_ESP32C5)
    return "esp32c5";
#elif defined(CONFIG_IDF_TARGET_ESP32C6)
    return "esp32c6";
#elif defined(CONFIG_IDF_TARGET_ESP32S3)
    return "esp32s3";
#else
    return "esp32";
#endif
}

static bool emit_hello(onboarding_endpoint_t *endpoint, const char *nonce)
{
    const esp_app_desc_t *description = esp_app_get_description();
    char digest[17] = {0};
    ruview_onboarding_device_digest(digest);
    const bool configured = s_current_config.wifi_ssid[0] != '\0' &&
                            s_current_config.target_ip[0] != '\0';
    return reply(endpoint, "RUVIEW_HELLO_OK_V1 nonce=%s chip=%s version=%s node_id=%u "
           "target_ip=%s target_port=%u configured=%u device_digest=%s\n",
           nonce, chip_name(), description->version,
           (unsigned)s_current_config.node_id, s_current_config.target_ip,
           (unsigned)s_current_config.target_port, configured ? 1U : 0U, digest);
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

static void process_line(onboarding_endpoint_t *endpoint, char *line)
{
    char nonce[RUVIEW_ONBOARDING_NONCE_HEX_LEN + 1] = {0};
    if (ruview_onboarding_parse_hello(line, nonce)) {
        endpoint->issued_nonce[0] = '\0';
        if (emit_hello(endpoint, nonce)) {
            memcpy(endpoint->issued_nonce, nonce, sizeof(endpoint->issued_nonce));
            endpoint->nonce_issued_at_us = esp_timer_get_time();
        }
        return;
    }
    if (strncmp(line, "RUVIEW_CONFIG_V1 ", 17) != 0) return;

    ruview_onboarding_config_t request;
    const ruview_onboarding_result_t parsed = ruview_onboarding_parse_config(line, &request);
    if (parsed != RUVIEW_ONBOARDING_OK) {
        (void)reply(endpoint, "RUVIEW_CONFIG_ERR_V1 reason=%s\n", ruview_onboarding_result_name(parsed));
        return;
    }
    const int64_t age_us = esp_timer_get_time() - endpoint->nonce_issued_at_us;
    if (endpoint->issued_nonce[0] == '\0' || strcmp(request.nonce, endpoint->issued_nonce) != 0 ||
        age_us < 0 || age_us > ONBOARDING_NONCE_TTL_US) {
        (void)reply(endpoint, "RUVIEW_CONFIG_ERR_V1 nonce=%s reason=claim_expired\n", request.nonce);
        return;
    }
    endpoint->issued_nonce[0] = '\0';
    const esp_err_t result = commit_config(&request);
    if (result != ESP_OK) {
        (void)reply(endpoint, "RUVIEW_CONFIG_ERR_V1 nonce=%s reason=nvs_commit code=%s\n",
               request.nonce, esp_err_to_name(result));
        return;
    }

    (void)reply(endpoint, "RUVIEW_CONFIG_OK_V1 nonce=%s node_id=%u rebooting=1\n",
           request.nonce, (unsigned)request.node_id);
    vTaskDelay(pdMS_TO_TICKS(400));
    esp_restart();
}

static void consume_bytes(onboarding_endpoint_t *endpoint, const uint8_t *input, ssize_t count)
{
    char *line = endpoint->line;
    size_t *length = &endpoint->length;
    bool *overflow = &endpoint->overflow;
    for (ssize_t index = 0; index < count; index++) {
        const uint8_t byte = input[index];
        if (byte == '\r') continue;
        if (byte == '\n') {
            if (!*overflow && *length > 0) {
                line[*length] = '\0';
                process_line(endpoint, line);
            }
            memset(line, 0, ONBOARDING_LINE_MAX);
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
    onboarding_endpoint_t primary = {0};
    bool primary_error_logged = false;
#if RUVIEW_ONBOARDING_HAS_NATIVE_USB
    onboarding_endpoint_t native_usb = {.native_usb = true};
#endif
    for (;;) {
        uint8_t input[64];
        const ssize_t count = read(STDIN_FILENO, input, sizeof(input));
        if (count > 0) {
            primary_error_logged = false;
            consume_bytes(&primary, input, count);
        }
        else if (count < 0 && errno != EAGAIN && errno != EWOULDBLOCK) {
            if (!primary_error_logged) {
                ESP_LOGW(TAG, "serial input unavailable: errno=%d", errno);
                primary_error_logged = true;
            }
#if !RUVIEW_ONBOARDING_HAS_NATIVE_USB
            vTaskDelay(pdMS_TO_TICKS(500));
#endif
        }
#if RUVIEW_ONBOARDING_HAS_NATIVE_USB
        const int usb_count = ruview_onboarding_usb_read(input, sizeof(input));
        if (usb_count > 0) consume_bytes(&native_usb, input, usb_count);
#endif
        vTaskDelay(pdMS_TO_TICKS(20));
    }
}

esp_err_t serial_onboarding_start(const nvs_config_t *config)
{
    if (config == NULL) return ESP_ERR_INVALID_ARG;
    const esp_err_t console_result = ruview_onboarding_console_prepare();
    if (console_result != ESP_OK) {
#if RUVIEW_ONBOARDING_HAS_NATIVE_USB
        /* A failed optional endpoint must not disable the existing UART. */
        ESP_LOGW(TAG, "native USB onboarding unavailable: %s; UART remains active", esp_err_to_name(console_result));
#else
        return console_result;
#endif
    }
    memcpy(&s_current_config, config, sizeof(s_current_config));
    const BaseType_t created = xTaskCreate(serial_onboarding_task, "serial_onboard",
                                           4096, NULL, 4, NULL);
    if (created != pdPASS) return ESP_ERR_NO_MEM;
    ESP_LOGI(TAG, "bounded serial onboarding ready");
    return ESP_OK;
}
