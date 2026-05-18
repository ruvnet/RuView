/**
 * @file led_indicator.c
 * @brief Configurable RGB LED Status Indicator for ESP32 CSI Node
 */

#include "led_indicator.h"
#include "sdkconfig.h"
#include "nvs_config.h"
#include "esp_log.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"

#ifdef CONFIG_RGB_LED_ENABLED
#include "led_strip.h"

static const char *TAG = "led_indicator";
extern nvs_config_t g_nvs_config;

static led_strip_handle_t s_led_strip = NULL;
static led_indicator_state_t s_current_state = LED_STATE_BOOTING;
static TaskHandle_t s_led_task = NULL;

static void led_task(void *arg)
{
    uint8_t pulse = 0;
    int8_t dir = 5;
    bool toggle = false;
    
    while (1) {
        if (!s_led_strip) {
            vTaskDelay(pdMS_TO_TICKS(100));
            continue;
        }

        switch (s_current_state) {
            case LED_STATE_BOOTING:
                /* Solid White */
                led_strip_set_pixel(s_led_strip, 0, 50, 50, 50);
                led_strip_refresh(s_led_strip);
                vTaskDelay(pdMS_TO_TICKS(100));
                break;
                
            case LED_STATE_WIFI_CONNECTING:
                /* Fast Blinking Blue */
                toggle = !toggle;
                if (toggle) {
                    led_strip_set_pixel(s_led_strip, 0, 0, 0, 100);
                } else {
                    led_strip_clear(s_led_strip);
                }
                led_strip_refresh(s_led_strip);
                vTaskDelay(pdMS_TO_TICKS(200));
                break;
                
            case LED_STATE_WIFI_ERROR:
                /* Solid Red */
                led_strip_set_pixel(s_led_strip, 0, 100, 0, 0);
                led_strip_refresh(s_led_strip);
                vTaskDelay(pdMS_TO_TICKS(100));
                break;
                
            case LED_STATE_CONNECTED:
                /* Slow Pulsing Green */
                pulse += dir;
                if (pulse >= 100 || pulse <= 0) {
                    dir = -dir;
                }
                led_strip_set_pixel(s_led_strip, 0, 0, pulse, 0);
                led_strip_refresh(s_led_strip);
                vTaskDelay(pdMS_TO_TICKS(50));
                break;
                
            case LED_STATE_MOCK_MODE:
                /* Blinking Yellow */
                toggle = !toggle;
                if (toggle) {
                    led_strip_set_pixel(s_led_strip, 0, 100, 100, 0);
                } else {
                    led_strip_clear(s_led_strip);
                }
                led_strip_refresh(s_led_strip);
                vTaskDelay(pdMS_TO_TICKS(500));
                break;
                
            case LED_STATE_MMWAVE_ERROR:
                /* Slow Blinking Yellow */
                toggle = !toggle;
                if (toggle) {
                    led_strip_set_pixel(s_led_strip, 0, 100, 100, 0);
                } else {
                    led_strip_clear(s_led_strip);
                }
                led_strip_refresh(s_led_strip);
                vTaskDelay(pdMS_TO_TICKS(1000));
                break;

            case LED_STATE_SWARM_ERROR:
                /* Slow Blinking Magenta */
                toggle = !toggle;
                if (toggle) {
                    led_strip_set_pixel(s_led_strip, 0, 100, 0, 100);
                } else {
                    led_strip_clear(s_led_strip);
                }
                led_strip_refresh(s_led_strip);
                vTaskDelay(pdMS_TO_TICKS(1000));
                break;

            case LED_STATE_SWARM_ACTIVE:
                /* Quick Blip Magenta */
                led_strip_set_pixel(s_led_strip, 0, 100, 0, 100);
                led_strip_refresh(s_led_strip);
                vTaskDelay(pdMS_TO_TICKS(100));
                led_strip_clear(s_led_strip);
                led_strip_refresh(s_led_strip);
                s_current_state = LED_STATE_CONNECTED;
                break;
        }
    }
}

void led_indicator_init(void)
{
    led_strip_config_t strip_config = {
        .strip_gpio_num = CONFIG_RGB_LED_GPIO,
        .max_leds = 1,
        .led_model = LED_MODEL_WS2812,
        .color_component_format = LED_STRIP_COLOR_COMPONENT_FMT_GRB,
        .flags.invert_out = false,
    };
    led_strip_rmt_config_t rmt_config = {
        .resolution_hz = 10 * 1000 * 1000, /* 10MHz */
        .flags.with_dma = false,
    };

    if (led_strip_new_rmt_device(&strip_config, &rmt_config, &s_led_strip) == ESP_OK) {
        led_strip_clear(s_led_strip);
        
        if (!g_nvs_config.status_led) {
            ESP_LOGI(TAG, "Status LED disabled by NVS configuration. Cleared and stopped.");
            return;
        }

        xTaskCreate(led_task, "led_indicator_task", 2048, NULL, 5, &s_led_task);
        ESP_LOGI(TAG, "RGB LED Indicator initialized on GPIO %d", CONFIG_RGB_LED_GPIO);
    } else {
        ESP_LOGE(TAG, "Failed to initialize RGB LED on GPIO %d", CONFIG_RGB_LED_GPIO);
    }
}

void led_indicator_set_state(led_indicator_state_t state)
{
    if (!g_nvs_config.status_led || !s_led_strip) {
        return;
    }
    s_current_state = state;
}

#else

/* Stubs when disabled via Kconfig */
void led_indicator_init(void) {}
void led_indicator_set_state(led_indicator_state_t state) {}

#endif
