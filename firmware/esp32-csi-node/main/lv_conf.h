/**
 * @file lv_conf.h
 * @brief LVGL compile-time configuration for ESP32-S3 AMOLED display (ADR-045).
 *
 * Tuned for RM67162 536x240 QSPI AMOLED with 8MB PSRAM.
 * Color depth: RGB565 (16-bit) for QSPI bandwidth.
 * Double-buffered in SPIRAM, 30fps target.
 */

#ifndef LV_CONF_H
#define LV_CONF_H

#include <stdint.h>
#include "sdkconfig.h"

#if CONFIG_DISPLAY_ENABLE

/* ---- Core ---- */
#define LV_COLOR_DEPTH          16
#define LV_COLOR_16_SWAP        1   /* Byte-swap for SPI/QSPI displays */
#define LV_MEM_CUSTOM           1   /* Use ESP-IDF heap instead of LVGL's internal allocator */
#define LV_MEM_CUSTOM_INCLUDE   <stdlib.h>
#define LV_MEM_CUSTOM_ALLOC     malloc
#define LV_MEM_CUSTOM_FREE      free
#define LV_MEM_CUSTOM_REALLOC   realloc

/* ---- Display ---- */
#define LV_HOR_RES_MAX          368
#define LV_VER_RES_MAX          448
#define LV_DPI_DEF              200

/* ---- Tick (provided by esp_timer in display_task.c) ---- */
#define LV_TICK_CUSTOM           1
#define LV_TICK_CUSTOM_INCLUDE   "esp_timer.h"
#define LV_TICK_CUSTOM_SYS_TIME_EXPR ((uint32_t)(esp_timer_get_time() / 1000))

/* ---- Drawing ---- */
#define LV_DRAW_COMPLEX         1
#define LV_SHADOW_CACHE_SIZE    0
#define LV_CIRCLE_CACHE_SIZE    4
#define LV_IMG_CACHE_DEF_SIZE   0

/* ---- Fonts ---- */
#define LV_FONT_MONTSERRAT_14   1
#define LV_FONT_MONTSERRAT_20   1
#define LV_FONT_DEFAULT         &lv_font_montserrat_14

/* ---- Widgets ---- */
#define LV_USE_ARC              1
#define LV_USE_BAR              1
#define LV_USE_BTN              0
#define LV_USE_BTNMATRIX        0
#define LV_USE_CANVAS           0
#define LV_USE_CHECKBOX         0
#define LV_USE_DROPDOWN         0
#define LV_USE_IMG              0
#define LV_USE_LABEL            1
#define LV_USE_LINE             1
#define LV_USE_ROLLER           0
#define LV_USE_SLIDER           0
#define LV_USE_SWITCH           0
#define LV_USE_TEXTAREA         0
#define LV_USE_TABLE            0

/* ---- Extra widgets ---- */
#define LV_USE_CHART            1
#define LV_CHART_AXIS_TICK_LABEL_MAX_LEN 32
#define LV_USE_METER            0
#define LV_USE_SPINBOX          0
#define LV_USE_SPAN             0
#define LV_USE_TILEVIEW         1   /* Used for swipeable page navigation */
#define LV_USE_TABVIEW          0
#define LV_USE_WIN              0

/* ---- Themes ---- */
#define LV_USE_THEME_DEFAULT    1
#define LV_THEME_DEFAULT_DARK   1

/* ---- Logging ---- */
#define LV_USE_LOG              0
#define LV_USE_ASSERT_NULL      1
#define LV_USE_ASSERT_MALLOC    1

/* ---- GPU / render ---- */
#define LV_USE_GPU_ESP32_S3     0   /* No parallel LCD interface — we use QSPI */

/* ---- Animation ---- */
#define LV_USE_ANIM             1
#define LV_ANIM_DEF_TIME        200

/* ---- Misc ---- */
#define LV_USE_GROUP            1   /* For touch/input device routing */
#define LV_USE_PERF_MONITOR     0
#define LV_USE_MEM_MONITOR      0
#define LV_SPRINTF_CUSTOM       0

#endif /* CONFIG_DISPLAY_ENABLE */

#endif /* LV_CONF_H */
