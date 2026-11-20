#include "c6_antenna_select.h"

#include "sdkconfig.h"

#if defined(CONFIG_IDF_TARGET_ESP32C6) && defined(CONFIG_C6_XIAO_ANTENNA_SELECT)
#include "driver/gpio.h"
#include "esp_log.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"

#define XIAO_RF_SWITCH_POWER_GPIO GPIO_NUM_3
#define XIAO_RF_SWITCH_SELECT_GPIO GPIO_NUM_14

static const char *TAG = "c6_antenna";
#endif

esp_err_t c6_xiao_antenna_apply(void)
{
#if defined(CONFIG_IDF_TARGET_ESP32C6) && defined(CONFIG_C6_XIAO_ANTENNA_SELECT)
    gpio_config_t config = {
        .pin_bit_mask = (1ULL << XIAO_RF_SWITCH_POWER_GPIO) |
                        (1ULL << XIAO_RF_SWITCH_SELECT_GPIO),
        .mode = GPIO_MODE_OUTPUT,
        .pull_up_en = GPIO_PULLUP_DISABLE,
        .pull_down_en = GPIO_PULLDOWN_DISABLE,
        .intr_type = GPIO_INTR_DISABLE,
    };
    esp_err_t err = gpio_config(&config);
    if (err != ESP_OK) {
        return err;
    }

    err = gpio_set_level(XIAO_RF_SWITCH_POWER_GPIO, 0);
    if (err != ESP_OK) {
        return err;
    }
    vTaskDelay(pdMS_TO_TICKS(100));

#if defined(CONFIG_C6_XIAO_ANTENNA_EXTERNAL)
    const c6_xiao_antenna_t antenna = C6_XIAO_ANTENNA_EXTERNAL;
    const char *name = "external U.FL";
#else
    const c6_xiao_antenna_t antenna = C6_XIAO_ANTENNA_INTERNAL;
    const char *name = "internal ceramic";
#endif

    err = gpio_set_level(XIAO_RF_SWITCH_SELECT_GPIO,
                         c6_xiao_antenna_gpio14_level(antenna));
    if (err == ESP_OK) {
        ESP_LOGI(TAG, "XIAO ESP32-C6 antenna path: %s", name);
    }
    return err;
#else
    return ESP_OK;
#endif
}
