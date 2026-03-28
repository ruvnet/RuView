/**
 * @file display_task.c
 * @brief ADR-045: FreeRTOS display task — LVGL pump on Core 0, priority 1.
 *
 * Gracefully skips if RM67162 panel or SPIRAM is absent.
 * Reads from edge_get_vitals() / edge_get_multi_person() (thread-safe).
 */

#include "display_task.h"
#include "sdkconfig.h"

#if CONFIG_DISPLAY_ENABLE

#include <string.h>
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "esp_log.h"
#include "esp_heap_caps.h"
#include "lvgl.h"

#include "display_hal.h"
#include "display_ui.h"

#define DISP_H_RES  368
#define DISP_V_RES  448

static const char *TAG = "disp_task";

/* ---- Config ---- */
#ifdef CONFIG_DISPLAY_FPS_LIMIT
#define DISP_FPS_LIMIT      CONFIG_DISPLAY_FPS_LIMIT
#else
#define DISP_FPS_LIMIT      30
#endif

#define DISP_TASK_STACK      (8 * 1024)
#define DISP_TASK_PRIORITY   1
#define DISP_TASK_CORE       0

#define DISP_BUF_LINES       40

/* ---- LVGL flush callback — calls display_hal_draw directly ---- */
static void lvgl_flush_cb(lv_disp_drv_t *drv, const lv_area_t *area, lv_color_t *color_p)
{
    display_hal_draw(area->x1, area->y1, area->x2 + 1, area->y2 + 1, color_p);
    lv_disp_flush_ready(drv);
}

/* ---- LVGL touch input callback ---- */
static void lvgl_touch_cb(lv_indev_drv_t *drv, lv_indev_data_t *data)
{
    uint16_t x, y;
    if (display_hal_touch_read(&x, &y)) {
        data->point.x = x;
        data->point.y = y;
        data->state = LV_INDEV_STATE_PRESSED;
    } else {
        data->state = LV_INDEV_STATE_RELEASED;
    }
}

/* ---- Display task ---- */
static void display_task(void *arg)
{
    const TickType_t frame_period = pdMS_TO_TICKS(1000 / DISP_FPS_LIMIT);

    ESP_LOGI(TAG, "Display task running on Core %d, %d fps limit",
             xPortGetCoreID(), DISP_FPS_LIMIT);

    display_ui_create(lv_scr_act());

    TickType_t last_wake = xTaskGetTickCount();
    while (1) {
        display_ui_update();
        lv_timer_handler();
        vTaskDelayUntil(&last_wake, frame_period);
    }
}

/* ---- Public API ---- */

esp_err_t display_task_start(void)
{
    ESP_LOGI(TAG, "Initializing display subsystem...");

    bool use_psram = false;
#if CONFIG_SPIRAM
    size_t psram_free = heap_caps_get_free_size(MALLOC_CAP_SPIRAM);
    if (psram_free >= 64 * 1024) {
        use_psram = true;
        ESP_LOGI(TAG, "PSRAM available: %u KB — using PSRAM buffers", (unsigned)(psram_free / 1024));
    } else {
        ESP_LOGW(TAG, "PSRAM too small (%u bytes) — falling back to internal DMA memory", (unsigned)psram_free);
    }
#else
    ESP_LOGW(TAG, "SPIRAM not enabled — using internal DMA memory (smaller buffers)");
#endif

    /* Probe display hardware */
    esp_err_t ret = display_hal_init_panel();
    if (ret != ESP_OK) {
        ESP_LOGW(TAG, "Display not available — running headless");
        return ESP_OK;
    }

    /* Init touch (optional) */
    esp_err_t touch_ret = display_hal_init_touch();

    /* Initialize LVGL */
    lv_init();

    /* Double-buffered draw buffers — prefer PSRAM, fall back to internal DMA */
    size_t buf_lines = use_psram ? DISP_BUF_LINES : 10;  /* Smaller buffers without PSRAM */
    size_t buf_size = DISP_H_RES * buf_lines * sizeof(lv_color_t);
    uint32_t alloc_caps = use_psram ? MALLOC_CAP_SPIRAM : (MALLOC_CAP_DMA | MALLOC_CAP_INTERNAL);
    lv_color_t *buf1 = heap_caps_malloc(buf_size, alloc_caps);
    lv_color_t *buf2 = heap_caps_malloc(buf_size, alloc_caps);
    if (!buf1 || !buf2) {
        ESP_LOGE(TAG, "Failed to allocate LVGL buffers (%u bytes, caps=0x%lx)",
                 (unsigned)buf_size, (unsigned long)alloc_caps);
        if (buf1) {
            free(buf1);
            buf1 = NULL;
        }
        if (buf2) {
            free(buf2);
            buf2 = NULL;
        }
        return ESP_OK;
    }
    ESP_LOGI(TAG, "LVGL buffers: 2x %u bytes (%u lines, %s)",
             (unsigned)buf_size, (unsigned)buf_lines, use_psram ? "PSRAM" : "internal DMA");

    static lv_disp_draw_buf_t draw_buf;
    lv_disp_draw_buf_init(&draw_buf, buf1, buf2, DISP_H_RES * buf_lines);

    static lv_disp_drv_t disp_drv;
    lv_disp_drv_init(&disp_drv);
    disp_drv.hor_res  = DISP_H_RES;
    disp_drv.ver_res  = DISP_V_RES;
    disp_drv.flush_cb = lvgl_flush_cb;
    disp_drv.draw_buf = &draw_buf;
    lv_disp_drv_register(&disp_drv);

    if (touch_ret == ESP_OK) {
        static lv_indev_drv_t indev_drv;
        lv_indev_drv_init(&indev_drv);
        indev_drv.type    = LV_INDEV_TYPE_POINTER;
        indev_drv.read_cb = lvgl_touch_cb;
        lv_indev_drv_register(&indev_drv);
        ESP_LOGI(TAG, "Touch input registered");
    }

    BaseType_t xret = xTaskCreatePinnedToCore(
        display_task, "display", DISP_TASK_STACK,
        NULL, DISP_TASK_PRIORITY, NULL, DISP_TASK_CORE);

    if (xret != pdPASS) {
        ESP_LOGE(TAG, "Failed to create display task");
        return ESP_OK;
    }

    ESP_LOGI(TAG, "Display task started (Core %d, priority %d, %d fps)",
             DISP_TASK_CORE, DISP_TASK_PRIORITY, DISP_FPS_LIMIT);
    return ESP_OK;
}

#else /* !CONFIG_DISPLAY_ENABLE */

esp_err_t display_task_start(void)
{
    return ESP_OK;
}

#endif /* CONFIG_DISPLAY_ENABLE */
