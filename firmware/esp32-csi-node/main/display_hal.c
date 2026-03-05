/**
 * @file display_hal.c
 * @brief ADR-045: SH8601 QSPI AMOLED HAL for Waveshare ESP32-S3-Touch-AMOLED-1.8.
 *
 * Uses ESP-IDF esp_lcd_panel_io_spi in QSPI mode (quad_mode=true, lcd_cmd_bits=32).
 * The panel_io layer handles the 0x02/0x32 QSPI command encoding.
 *
 * Hardware: SH8601 368x448, FT3168 touch, TCA9554 I/O expander for power/reset.
 *
 * Pin assignments (Waveshare ESP32-S3-Touch-AMOLED-1.8):
 *   QSPI: CS=12, CLK=11, D0=4, D1=5, D2=6, D3=7
 *   I2C:  SDA=15, SCL=14  (shared: touch FT3168 + TCA9554 expander)
 *   Touch INT=21
 */

#include "display_hal.h"
#include "sdkconfig.h"

#if CONFIG_DISPLAY_ENABLE

#include <string.h>
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "esp_log.h"
#include "esp_lcd_panel_io.h"
#include "driver/spi_master.h"
#include "driver/gpio.h"
#include "driver/i2c.h"
#include "esp_heap_caps.h"

static const char *TAG = "disp_hal";

/* ---- QSPI Pin Definitions (Waveshare board) ---- */
#define DISP_QSPI_CS       12
#define DISP_QSPI_CLK      11
#define DISP_QSPI_D0       4
#define DISP_QSPI_D1       5
#define DISP_QSPI_D2       6
#define DISP_QSPI_D3       7

/* ---- I2C (shared: touch + TCA9554 expander) ---- */
#define I2C_SDA             15
#define I2C_SCL             14
#define TOUCH_INT_PIN       21
#define I2C_MASTER_NUM      I2C_NUM_0
#define I2C_MASTER_FREQ_HZ  400000

/* ---- TCA9554 I/O expander ---- */
#define TCA9554_ADDR        0x20
#define TCA9554_REG_OUTPUT  0x01
#define TCA9554_REG_CONFIG  0x03

/* ---- FT3168 touch controller ---- */
#define FT3168_ADDR         0x38

/* ---- Display dimensions ---- */
#define DISP_H_RES          368
#define DISP_V_RES          448

/* ---- QSPI opcodes (packed into lcd_cmd bits [31:24]) ---- */
#define LCD_OPCODE_WRITE_CMD   0x02
#define LCD_OPCODE_WRITE_COLOR 0x32

/* ---- State ---- */
static esp_lcd_panel_io_handle_t s_io_handle = NULL;
static bool s_i2c_initialized = false;
static bool s_touch_initialized = false;

/* ---- I2C helpers ---- */

static esp_err_t i2c_write_reg(uint8_t dev_addr, uint8_t reg, const uint8_t *data, size_t len)
{
    i2c_cmd_handle_t cmd = i2c_cmd_link_create();
    i2c_master_start(cmd);
    i2c_master_write_byte(cmd, (dev_addr << 1) | I2C_MASTER_WRITE, true);
    i2c_master_write_byte(cmd, reg, true);
    if (data && len > 0) {
        i2c_master_write(cmd, data, len, true);
    }
    i2c_master_stop(cmd);
    esp_err_t ret = i2c_master_cmd_begin(I2C_MASTER_NUM, cmd, pdMS_TO_TICKS(100));
    i2c_cmd_link_delete(cmd);
    return ret;
}

static esp_err_t i2c_read_reg(uint8_t dev_addr, uint8_t reg, uint8_t *data, size_t len)
{
    i2c_cmd_handle_t cmd = i2c_cmd_link_create();
    i2c_master_start(cmd);
    i2c_master_write_byte(cmd, (dev_addr << 1) | I2C_MASTER_WRITE, true);
    i2c_master_write_byte(cmd, reg, true);
    i2c_master_start(cmd);
    i2c_master_write_byte(cmd, (dev_addr << 1) | I2C_MASTER_READ, true);
    i2c_master_read(cmd, data, len, I2C_MASTER_LAST_NACK);
    i2c_master_stop(cmd);
    esp_err_t ret = i2c_master_cmd_begin(I2C_MASTER_NUM, cmd, pdMS_TO_TICKS(100));
    i2c_cmd_link_delete(cmd);
    return ret;
}

static esp_err_t init_i2c_bus(void)
{
    if (s_i2c_initialized) return ESP_OK;

    i2c_config_t i2c_cfg = {
        .mode             = I2C_MODE_MASTER,
        .sda_io_num       = I2C_SDA,
        .scl_io_num       = I2C_SCL,
        .sda_pullup_en    = GPIO_PULLUP_ENABLE,
        .scl_pullup_en    = GPIO_PULLUP_ENABLE,
        .master.clk_speed = I2C_MASTER_FREQ_HZ,
    };

    esp_err_t ret = i2c_param_config(I2C_MASTER_NUM, &i2c_cfg);
    if (ret != ESP_OK) return ret;

    ret = i2c_driver_install(I2C_MASTER_NUM, I2C_MODE_MASTER, 0, 0, 0);
    if (ret != ESP_OK) return ret;

    s_i2c_initialized = true;
    ESP_LOGI(TAG, "I2C bus init OK (SDA=%d, SCL=%d)", I2C_SDA, I2C_SCL);
    return ESP_OK;
}

/* ---- TCA9554 I/O expander: toggle pins for display power/reset ---- */

static esp_err_t tca9554_init_display_power(void)
{
    /* Set pins 0, 1, 2 as outputs */
    uint8_t cfg = 0xF8;
    esp_err_t ret = i2c_write_reg(TCA9554_ADDR, TCA9554_REG_CONFIG, &cfg, 1);
    if (ret != ESP_OK) {
        ESP_LOGW(TAG, "TCA9554 not found at 0x%02X: %s", TCA9554_ADDR, esp_err_to_name(ret));
        return ret;
    }

    /* Set pins 0,1,2 LOW (reset state) */
    uint8_t out = 0x00;
    i2c_write_reg(TCA9554_ADDR, TCA9554_REG_OUTPUT, &out, 1);
    vTaskDelay(pdMS_TO_TICKS(200));

    /* Set pins 0,1,2 HIGH (power on + release reset) */
    out = 0x07;
    i2c_write_reg(TCA9554_ADDR, TCA9554_REG_OUTPUT, &out, 1);
    vTaskDelay(pdMS_TO_TICKS(200));

    ESP_LOGI(TAG, "TCA9554 display power/reset toggled");
    return ESP_OK;
}

/* ---- Panel IO helpers: send commands via esp_lcd QSPI panel IO ---- */

static esp_err_t panel_write_cmd(uint8_t dcs_cmd, const void *data, size_t data_len)
{
    /* Pack as 32-bit lcd_cmd: [31:24]=opcode, [23:8]=dcs_cmd, [7:0]=0 */
    uint32_t lcd_cmd = ((uint32_t)LCD_OPCODE_WRITE_CMD << 24) | ((uint32_t)dcs_cmd << 8);
    return esp_lcd_panel_io_tx_param(s_io_handle, (int)lcd_cmd, data, data_len);
}

static esp_err_t panel_write_color(const void *color_data, size_t data_len)
{
    /* RAMWR (0x2C) packed as 32-bit lcd_cmd with quad opcode */
    uint32_t lcd_cmd = ((uint32_t)LCD_OPCODE_WRITE_COLOR << 24) | (0x2C << 8);
    return esp_lcd_panel_io_tx_color(s_io_handle, (int)lcd_cmd, color_data, data_len);
}

/* ---- SH8601 init sequence (from Waveshare reference) ---- */

typedef struct {
    uint8_t cmd;
    uint8_t data[4];
    uint8_t data_len;
    uint16_t delay_ms;
} sh8601_init_cmd_t;

static const sh8601_init_cmd_t sh8601_init_cmds[] = {
    {0x11, {0x00},                   0, 120},  /* Sleep Out + 120ms */
    {0x44, {0x01, 0xD1},             2, 0},    /* Partial area */
    {0x35, {0x00},                   1, 0},    /* Tearing Effect ON */
    {0x53, {0x20},                   1, 10},   /* Write CTRL Display */
    {0x2A, {0x00, 0x00, 0x01, 0x6F}, 4, 0},   /* CASET: 0-367 */
    {0x2B, {0x00, 0x00, 0x01, 0xBF}, 4, 0},   /* RASET: 0-447 */
    {0x51, {0x00},                   1, 10},   /* Brightness: 0 */
    {0x29, {0x00},                   0, 10},   /* Display ON */
    {0x51, {0xFF},                   1, 0},    /* Brightness: max */
    {0x00, {0x00},                   0xFF, 0}, /* End sentinel */
};

static esp_err_t send_init_sequence(void)
{
    for (int i = 0; sh8601_init_cmds[i].data_len != 0xFF; i++) {
        const sh8601_init_cmd_t *cmd = &sh8601_init_cmds[i];
        esp_err_t ret = panel_write_cmd(
            cmd->cmd,
            cmd->data_len > 0 ? cmd->data : NULL,
            cmd->data_len);
        if (ret != ESP_OK) {
            ESP_LOGE(TAG, "CMD 0x%02X failed: %s", cmd->cmd, esp_err_to_name(ret));
            return ret;
        }
        if (cmd->delay_ms > 0) {
            vTaskDelay(pdMS_TO_TICKS(cmd->delay_ms));
        }
    }
    return ESP_OK;
}

/* ---- Public API ---- */

esp_err_t display_hal_init_panel(void)
{
    ESP_LOGI(TAG, "Initializing Waveshare AMOLED 1.8\" (SH8601 368x448)...");

    /* Step 1: Init I2C bus */
    esp_err_t ret = init_i2c_bus();
    if (ret != ESP_OK) {
        ESP_LOGW(TAG, "I2C bus init failed");
        return ESP_ERR_NOT_FOUND;
    }

    /* Step 2: TCA9554 display power/reset (optional — only present on Waveshare board) */
    ret = tca9554_init_display_power();
    if (ret != ESP_OK) {
        ESP_LOGW(TAG, "TCA9554 not found — assuming display power is always-on (direct wiring)");
        /* Continue without TCA9554 — the display may be powered directly */
    }

    /* Step 3: Initialize SPI bus */
    spi_bus_config_t bus_cfg = {
        .sclk_io_num     = DISP_QSPI_CLK,
        .data0_io_num    = DISP_QSPI_D0,
        .data1_io_num    = DISP_QSPI_D1,
        .data2_io_num    = DISP_QSPI_D2,
        .data3_io_num    = DISP_QSPI_D3,
        .max_transfer_sz = DISP_H_RES * DISP_V_RES * 2,
    };

    ret = spi_bus_initialize(SPI2_HOST, &bus_cfg, SPI_DMA_CH_AUTO);
    if (ret != ESP_OK) {
        ESP_LOGW(TAG, "SPI bus init failed: %s", esp_err_to_name(ret));
        return ESP_ERR_NOT_FOUND;
    }

    /* Step 4: Create panel IO with QSPI mode */
    esp_lcd_panel_io_spi_config_t io_config = {
        .dc_gpio_num       = -1,       /* No DC pin in QSPI mode */
        .cs_gpio_num       = DISP_QSPI_CS,
        .pclk_hz           = 40 * 1000 * 1000,
        .lcd_cmd_bits      = 32,       /* 32-bit command: [opcode|dcs_cmd|0x00] */
        .lcd_param_bits    = 8,
        .spi_mode          = 0,
        .trans_queue_depth = 10,
        .flags = {
            .quad_mode = true,
        },
    };

    ret = esp_lcd_new_panel_io_spi((esp_lcd_spi_bus_handle_t)SPI2_HOST, &io_config, &s_io_handle);
    if (ret != ESP_OK) {
        ESP_LOGE(TAG, "Panel IO init failed: %s", esp_err_to_name(ret));
        spi_bus_free(SPI2_HOST);
        return ESP_ERR_NOT_FOUND;
    }
    ESP_LOGI(TAG, "QSPI panel IO created (40MHz, quad mode)");

    /* Step 5: Send SH8601 init sequence */
    ret = send_init_sequence();
    if (ret != ESP_OK) {
        ESP_LOGW(TAG, "SH8601 init sequence failed");
        esp_lcd_panel_io_del(s_io_handle);
        spi_bus_free(SPI2_HOST);
        s_io_handle = NULL;
        return ESP_ERR_NOT_FOUND;
    }

    /* Step 6: Draw test pattern — cyan bar at top */
    ESP_LOGI(TAG, "Drawing test pattern...");
    uint16_t *line_buf = heap_caps_malloc(DISP_H_RES * 2, MALLOC_CAP_DMA);
    if (line_buf) {
        uint8_t caset[4] = {0, 0, (DISP_H_RES - 1) >> 8, (DISP_H_RES - 1) & 0xFF};
        uint8_t raset[4] = {0, 0, (DISP_V_RES - 1) >> 8, (DISP_V_RES - 1) & 0xFF};
        panel_write_cmd(0x2A, caset, 4);
        panel_write_cmd(0x2B, raset, 4);

        for (int y = 0; y < DISP_V_RES; y++) {
            uint16_t color = (y < 30) ? 0x07FF : 0x0841;
            for (int x = 0; x < DISP_H_RES; x++) {
                line_buf[x] = color;
            }
            panel_write_color(line_buf, DISP_H_RES * 2);
        }
        free(line_buf);
        ESP_LOGI(TAG, "Test pattern drawn");
    }

    ESP_LOGI(TAG, "SH8601 panel init OK (%dx%d)", DISP_H_RES, DISP_V_RES);
    return ESP_OK;
}

void display_hal_draw(int x_start, int y_start, int x_end, int y_end,
                      const void *color_data)
{
    if (!s_io_handle) return;

    /* SH8601 requires coordinates divisible by 2 */
    x_start &= ~1;
    y_start &= ~1;
    if (x_end & 1) x_end++;
    if (y_end & 1) y_end++;
    if (x_end > DISP_H_RES) x_end = DISP_H_RES;
    if (y_end > DISP_V_RES) y_end = DISP_V_RES;

    uint8_t caset[4] = {
        (x_start >> 8) & 0xFF, x_start & 0xFF,
        ((x_end - 1) >> 8) & 0xFF, (x_end - 1) & 0xFF,
    };
    panel_write_cmd(0x2A, caset, 4);

    uint8_t raset[4] = {
        (y_start >> 8) & 0xFF, y_start & 0xFF,
        ((y_end - 1) >> 8) & 0xFF, (y_end - 1) & 0xFF,
    };
    panel_write_cmd(0x2B, raset, 4);

    size_t len = (x_end - x_start) * (y_end - y_start) * 2;
    panel_write_color(color_data, len);
}

esp_err_t display_hal_init_touch(void)
{
    ESP_LOGI(TAG, "Probing FT3168 touch controller...");

    if (!s_i2c_initialized) {
        esp_err_t ret = init_i2c_bus();
        if (ret != ESP_OK) return ESP_ERR_NOT_FOUND;
    }

    gpio_config_t int_cfg = {
        .pin_bit_mask = (1ULL << TOUCH_INT_PIN),
        .mode         = GPIO_MODE_INPUT,
        .pull_up_en   = GPIO_PULLUP_ENABLE,
        .intr_type    = GPIO_INTR_DISABLE,
    };
    gpio_config(&int_cfg);

    uint8_t chip_id = 0;
    esp_err_t ret = i2c_read_reg(FT3168_ADDR, 0xA8, &chip_id, 1);
    if (ret != ESP_OK || chip_id == 0x00 || chip_id == 0xFF) {
        ESP_LOGW(TAG, "FT3168 not found (ret=%s, id=0x%02X)", esp_err_to_name(ret), chip_id);
        return ESP_ERR_NOT_FOUND;
    }

    s_touch_initialized = true;
    ESP_LOGI(TAG, "FT3168 touch init OK (chip_id=0x%02X)", chip_id);
    return ESP_OK;
}

bool display_hal_touch_read(uint16_t *x, uint16_t *y)
{
    if (!s_touch_initialized) return false;

    uint8_t buf[7] = {0};
    esp_err_t ret = i2c_read_reg(FT3168_ADDR, 0x01, buf, 7);
    if (ret != ESP_OK) return false;

    uint8_t num_points = buf[1];
    if (num_points == 0 || num_points > 2) return false;

    *x = ((buf[2] & 0x0F) << 8) | buf[3];
    *y = ((buf[4] & 0x0F) << 8) | buf[5];
    return true;
}

void display_hal_set_brightness(uint8_t percent)
{
    if (!s_io_handle) return;
    if (percent > 100) percent = 100;
    uint8_t val = (uint8_t)((uint32_t)percent * 255 / 100);
    panel_write_cmd(0x51, &val, 1);
}

#endif /* CONFIG_DISPLAY_ENABLE */
