/**
 * @file display_hal_st7789.c
 * @brief ADR-045: ST7789V2 SPI LCD + CST816 touch HAL.
 *
 * Hardware abstraction for the Waveshare ESP32-S3-Touch-LCD-1.69
 * (240x280 IPS, ST7789V2 over 4-wire SPI, CST816 capacitive touch).
 *
 * Implements the same display_hal.h contract as the SH8601 QSPI AMOLED HAL,
 * but on ESP-IDF's built-in esp_lcd ST7789 panel driver. Selected at build
 * time via CONFIG_DISPLAY_PANEL_ST7789 (see Kconfig.projbuild). Exactly one
 * display_hal_*.c is compiled into the image (CMakeLists picks by panel).
 *
 * Pin assignments (Waveshare ESP32-S3-Touch-LCD-1.69, Kconfig-overridable):
 *   LCD SPI: SCLK=6, MOSI=7, CS=5, DC=4, RST=8, BL=15
 *   Touch:   I2C SDA=11, SCL=10, RST=13, INT=14 (CST816 @ 0x15)
 *
 * Runs concurrently with the full CSI pipeline — the panel sits on SPI2_HOST
 * and a private I2C bus, neither of which the radio/UDP data plane touches.
 */

#include "display_hal.h"
#include "sdkconfig.h"

#if CONFIG_DISPLAY_ENABLE && CONFIG_DISPLAY_PANEL_ST7789

#include <string.h>
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "esp_log.h"
#include "esp_lcd_panel_io.h"
#include "esp_lcd_panel_ops.h"
#include "esp_lcd_panel_vendor.h"
#include "driver/spi_master.h"
#include "driver/gpio.h"
#include "driver/ledc.h"
#include "driver/i2c.h"
#include "esp_heap_caps.h"

static const char *TAG = "disp_hal_st7789";

/* ---- LCD SPI pins (Kconfig-overridable) ---- */
#define LCD_PIN_SCLK   CONFIG_DISPLAY_ST7789_SCLK
#define LCD_PIN_MOSI   CONFIG_DISPLAY_ST7789_MOSI
#define LCD_PIN_CS     CONFIG_DISPLAY_ST7789_CS
#define LCD_PIN_DC     CONFIG_DISPLAY_ST7789_DC
#define LCD_PIN_RST    CONFIG_DISPLAY_ST7789_RST
#define LCD_PIN_BL     CONFIG_DISPLAY_ST7789_BL

#define LCD_H_RES      CONFIG_DISPLAY_ST7789_H_RES
#define LCD_V_RES      CONFIG_DISPLAY_ST7789_V_RES
#define LCD_GAP_X      CONFIG_DISPLAY_ST7789_GAP_X
#define LCD_GAP_Y      CONFIG_DISPLAY_ST7789_GAP_Y

#define LCD_HOST       SPI2_HOST
#define LCD_PCLK_HZ    (40 * 1000 * 1000)

/* ---- Backlight PWM ---- */
#define BL_LEDC_TIMER      LEDC_TIMER_0
#define BL_LEDC_MODE       LEDC_LOW_SPEED_MODE
#define BL_LEDC_CHANNEL    LEDC_CHANNEL_0
#define BL_LEDC_DUTY_RES   LEDC_TIMER_8_BIT
#define BL_LEDC_FREQ_HZ    5000

/* ---- CST816 touch (I2C) ---- */
#define TOUCH_I2C_NUM      I2C_NUM_0
#define TOUCH_I2C_FREQ_HZ  400000
#define TOUCH_PIN_SDA      CONFIG_DISPLAY_ST7789_TOUCH_SDA
#define TOUCH_PIN_SCL      CONFIG_DISPLAY_ST7789_TOUCH_SCL
#define TOUCH_PIN_RST      CONFIG_DISPLAY_ST7789_TOUCH_RST
#define TOUCH_PIN_INT      CONFIG_DISPLAY_ST7789_TOUCH_INT
#define CST816_ADDR        0x15
#define CST816_REG_CHIPID  0xA7

/* ---- State ---- */
static esp_lcd_panel_io_handle_t s_io_handle = NULL;
static esp_lcd_panel_handle_t    s_panel = NULL;
static bool s_bl_initialized = false;
static bool s_touch_initialized = false;

/* ===================== Backlight ===================== */

static void init_backlight(void)
{
    ledc_timer_config_t timer = {
        .speed_mode      = BL_LEDC_MODE,
        .timer_num       = BL_LEDC_TIMER,
        .duty_resolution = BL_LEDC_DUTY_RES,
        .freq_hz         = BL_LEDC_FREQ_HZ,
        .clk_cfg         = LEDC_AUTO_CLK,
    };
    if (ledc_timer_config(&timer) != ESP_OK) {
        ESP_LOGW(TAG, "LEDC timer config failed — backlight will be GPIO-on");
        gpio_set_direction(LCD_PIN_BL, GPIO_MODE_OUTPUT);
        gpio_set_level(LCD_PIN_BL, 1);
        return;
    }
    ledc_channel_config_t ch = {
        .gpio_num   = LCD_PIN_BL,
        .speed_mode = BL_LEDC_MODE,
        .channel    = BL_LEDC_CHANNEL,
        .timer_sel  = BL_LEDC_TIMER,
        .duty       = 0,
        .hpoint     = 0,
    };
    ledc_channel_config(&ch);
    s_bl_initialized = true;
}

/* ===================== Panel ===================== */

static esp_err_t draw_test_pattern(void)
{
    /* Clear to background once (low power — no full-white frame, which spiked
     * current and worsened the startup brownout). LVGL draws over this. */
    uint16_t *line = heap_caps_malloc(LCD_H_RES * sizeof(uint16_t), MALLOC_CAP_DMA);
    if (!line) return ESP_ERR_NO_MEM;
    for (int x = 0; x < LCD_H_RES; x++) line[x] = 0x0841;  /* near-black */
    for (int y = 0; y < LCD_V_RES; y++)
        esp_lcd_panel_draw_bitmap(s_panel, 0, y, LCD_H_RES, y + 1, line);
    free(line);
    return ESP_OK;
}

esp_err_t display_hal_init_panel(void)
{
    ESP_LOGI(TAG, "Initializing Waveshare 1.69\" LCD (ST7789V2 %dx%d)...",
             LCD_H_RES, LCD_V_RES);

    spi_bus_config_t bus_cfg = {
        .sclk_io_num     = LCD_PIN_SCLK,
        .mosi_io_num     = LCD_PIN_MOSI,
        .miso_io_num     = -1,
        .quadwp_io_num   = -1,
        .quadhd_io_num   = -1,
        .max_transfer_sz = LCD_H_RES * 80 * sizeof(uint16_t),
    };
    esp_err_t ret = spi_bus_initialize(LCD_HOST, &bus_cfg, SPI_DMA_CH_AUTO);
    if (ret != ESP_OK) {
        ESP_LOGW(TAG, "SPI bus init failed: %s", esp_err_to_name(ret));
        return ESP_ERR_NOT_FOUND;
    }

    esp_lcd_panel_io_spi_config_t io_config = {
        .dc_gpio_num       = LCD_PIN_DC,
        .cs_gpio_num       = LCD_PIN_CS,
        .pclk_hz           = LCD_PCLK_HZ,
        .lcd_cmd_bits      = 8,
        .lcd_param_bits    = 8,
        .spi_mode          = 0,
        .trans_queue_depth = 10,
    };
    ret = esp_lcd_new_panel_io_spi((esp_lcd_spi_bus_handle_t)LCD_HOST, &io_config, &s_io_handle);
    if (ret != ESP_OK) {
        ESP_LOGE(TAG, "Panel IO init failed: %s", esp_err_to_name(ret));
        spi_bus_free(LCD_HOST);
        return ESP_ERR_NOT_FOUND;
    }

    esp_lcd_panel_dev_config_t panel_config = {
        .reset_gpio_num = LCD_PIN_RST,
        .rgb_ele_order  = LCD_RGB_ELEMENT_ORDER_RGB,
        .bits_per_pixel = 16,
    };
    ret = esp_lcd_new_panel_st7789(s_io_handle, &panel_config, &s_panel);
    if (ret != ESP_OK) {
        ESP_LOGE(TAG, "ST7789 panel create failed: %s", esp_err_to_name(ret));
        esp_lcd_panel_io_del(s_io_handle);
        spi_bus_free(LCD_HOST);
        s_io_handle = NULL;
        return ESP_ERR_NOT_FOUND;
    }

    esp_lcd_panel_reset(s_panel);
    esp_lcd_panel_init(s_panel);
    /* IPS ST7789 panels show inverted colour without this. */
    esp_lcd_panel_invert_color(s_panel, true);
    /* 240x280 visible window sits at row LCD_GAP_Y of the 240x320 controller RAM. */
    esp_lcd_panel_set_gap(s_panel, LCD_GAP_X, LCD_GAP_Y);
    esp_lcd_panel_disp_on_off(s_panel, true);

    init_backlight();
    display_hal_set_brightness(CONFIG_DISPLAY_BRIGHTNESS);

    draw_test_pattern();

    ESP_LOGI(TAG, "ST7789 panel init OK (%dx%d, gap %d,%d)",
             LCD_H_RES, LCD_V_RES, LCD_GAP_X, LCD_GAP_Y);
    return ESP_OK;
}

void display_hal_draw(int x_start, int y_start, int x_end, int y_end,
                      const void *color_data)
{
    if (!s_panel) return;
    /* esp_lcd takes an exclusive end coord, which is exactly what
     * display_task's flush callback already passes (area->x2 + 1). */
    esp_lcd_panel_draw_bitmap(s_panel, x_start, y_start, x_end, y_end, color_data);
}

/* ===================== Touch (CST816) ===================== */

static esp_err_t touch_i2c_init(void)
{
    i2c_config_t cfg = {
        .mode             = I2C_MODE_MASTER,
        .sda_io_num       = TOUCH_PIN_SDA,
        .scl_io_num       = TOUCH_PIN_SCL,
        .sda_pullup_en    = GPIO_PULLUP_ENABLE,
        .scl_pullup_en    = GPIO_PULLUP_ENABLE,
        .master.clk_speed = TOUCH_I2C_FREQ_HZ,
    };
    esp_err_t ret = i2c_param_config(TOUCH_I2C_NUM, &cfg);
    if (ret != ESP_OK) return ret;
    return i2c_driver_install(TOUCH_I2C_NUM, I2C_MODE_MASTER, 0, 0, 0);
}

static esp_err_t touch_read_reg(uint8_t reg, uint8_t *data, size_t len)
{
    i2c_cmd_handle_t cmd = i2c_cmd_link_create();
    i2c_master_start(cmd);
    i2c_master_write_byte(cmd, (CST816_ADDR << 1) | I2C_MASTER_WRITE, true);
    i2c_master_write_byte(cmd, reg, true);
    i2c_master_start(cmd);
    i2c_master_write_byte(cmd, (CST816_ADDR << 1) | I2C_MASTER_READ, true);
    i2c_master_read(cmd, data, len, I2C_MASTER_LAST_NACK);
    i2c_master_stop(cmd);
    esp_err_t ret = i2c_master_cmd_begin(TOUCH_I2C_NUM, cmd, pdMS_TO_TICKS(100));
    i2c_cmd_link_delete(cmd);
    return ret;
}

esp_err_t display_hal_init_touch(void)
{
    ESP_LOGI(TAG, "Probing CST816 touch controller...");

    /* Hardware reset pulse (CST816 needs RST toggled to boot). */
    gpio_set_direction(TOUCH_PIN_RST, GPIO_MODE_OUTPUT);
    gpio_set_level(TOUCH_PIN_RST, 0);
    vTaskDelay(pdMS_TO_TICKS(10));
    gpio_set_level(TOUCH_PIN_RST, 1);
    vTaskDelay(pdMS_TO_TICKS(60));

    gpio_config_t int_cfg = {
        .pin_bit_mask = (1ULL << TOUCH_PIN_INT),
        .mode         = GPIO_MODE_INPUT,
        .pull_up_en   = GPIO_PULLUP_ENABLE,
        .intr_type    = GPIO_INTR_DISABLE,
    };
    gpio_config(&int_cfg);

    esp_err_t ret = touch_i2c_init();
    if (ret != ESP_OK) {
        ESP_LOGW(TAG, "Touch I2C init failed: %s", esp_err_to_name(ret));
        return ESP_ERR_NOT_FOUND;
    }

    uint8_t chip_id = 0;
    ret = touch_read_reg(CST816_REG_CHIPID, &chip_id, 1);
    if (ret != ESP_OK || chip_id == 0x00 || chip_id == 0xFF) {
        ESP_LOGW(TAG, "CST816 not found (ret=%s, id=0x%02X)", esp_err_to_name(ret), chip_id);
        return ESP_ERR_NOT_FOUND;
    }

    s_touch_initialized = true;
    ESP_LOGI(TAG, "CST816 touch init OK (chip_id=0x%02X)", chip_id);
    return ESP_OK;
}

bool display_hal_touch_read(uint16_t *x, uint16_t *y)
{
    if (!s_touch_initialized) return false;

    /* Read gesture(0x01), finger_num(0x02), X hi/lo(0x03/04), Y hi/lo(0x05/06). */
    uint8_t buf[6] = {0};
    if (touch_read_reg(0x01, buf, sizeof(buf)) != ESP_OK) return false;

    uint8_t fingers = buf[1] & 0x0F;
    if (fingers == 0) return false;

    *x = ((uint16_t)(buf[2] & 0x0F) << 8) | buf[3];
    *y = ((uint16_t)(buf[4] & 0x0F) << 8) | buf[5];
    return true;
}

/* ===================== Brightness ===================== */

void display_hal_set_brightness(uint8_t percent)
{
    if (percent > 100) percent = 100;
    if (!s_bl_initialized) {
        /* LEDC unavailable — drive backlight GPIO directly. */
        gpio_set_level(LCD_PIN_BL, percent > 0 ? 1 : 0);
        return;
    }
    uint32_t duty = (uint32_t)percent * 255 / 100;   /* 8-bit resolution */
    ledc_set_duty(BL_LEDC_MODE, BL_LEDC_CHANNEL, duty);
    ledc_update_duty(BL_LEDC_MODE, BL_LEDC_CHANNEL);
}

void display_hal_refresh(void)
{
    /* Backlight-only self-heal: re-assert the LEDC duty (no SPI). The earlier
     * version also poked esp_lcd_panel_disp_on_off over SPI every 2s, which
     * could contend with the in-flight LVGL flush and hang the display task.
     * On a stable supply the panel never powers off, so backlight is enough. */
    display_hal_set_brightness(CONFIG_DISPLAY_BRIGHTNESS);
}

#endif /* CONFIG_DISPLAY_ENABLE && CONFIG_DISPLAY_PANEL_ST7789 */
