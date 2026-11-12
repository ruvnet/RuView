#ifndef RUVIEW_SERIAL_TEST_PLATFORM_H
#define RUVIEW_SERIAL_TEST_PLATFORM_H
#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>
typedef int esp_err_t;
typedef unsigned nvs_handle_t;
typedef int BaseType_t;
typedef struct { char version[32]; } esp_app_desc_t;
#define ESP_OK 0
#define ESP_FAIL -1
#define ESP_ERR_INVALID_ARG 0x102
#define ESP_ERR_NO_MEM 0x101
#define ESP_ERR_NVS_NOT_FOUND 0x1102
#define NVS_READONLY 0
#define NVS_READWRITE 1
#define pdPASS 1
#define pdMS_TO_TICKS(value) (value)
#define ESP_LOGI(...) ((void)0)
#define ESP_LOGW(...) ((void)0)
const esp_app_desc_t *esp_app_get_description(void);
esp_err_t ruview_onboarding_console_prepare(void);
void ruview_onboarding_device_digest(char output[17]);
int64_t esp_timer_get_time(void);
const char *esp_err_to_name(esp_err_t error);
void esp_restart(void);
void vTaskDelay(uint32_t ticks);
BaseType_t xTaskCreate(void (*task)(void *), const char *name, unsigned stack, void *arg, unsigned priority, void *handle);
esp_err_t nvs_open(const char *name, int mode, nvs_handle_t *handle);
void nvs_close(nvs_handle_t handle);
esp_err_t nvs_set_str(nvs_handle_t handle, const char *key, const char *value);
esp_err_t nvs_set_u16(nvs_handle_t handle, const char *key, uint16_t value);
esp_err_t nvs_set_u8(nvs_handle_t handle, const char *key, uint8_t value);
esp_err_t nvs_get_str(nvs_handle_t handle, const char *key, char *value, size_t *length);
esp_err_t nvs_get_u16(nvs_handle_t handle, const char *key, uint16_t *value);
esp_err_t nvs_get_u8(nvs_handle_t handle, const char *key, uint8_t *value);
esp_err_t nvs_commit(nvs_handle_t handle);
#endif
