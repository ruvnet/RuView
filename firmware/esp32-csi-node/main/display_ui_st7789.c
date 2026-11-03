/**
 * @file display_ui_st7789.c
 * @brief ADR-045: compact LVGL UI for the 240x280 ST7789 panel.
 *
 * The 4-view AMOLED UI (display_ui.c) is laid out for 368x448 and overflows a
 * 1.69" LCD. This variant is a single legible screen sized for 240x280: node
 * identity, a big ACTIVITY bar driven by the CSI motion/presence metrics (so
 * movement is visible across a room), and a live CSI packet-rate footer.
 * Selected at build time via CONFIG_DISPLAY_PANEL_ST7789 (CMakeLists compiles
 * this instead of display_ui.c). Same display_ui.h contract.
 */

#include "display_ui.h"
#include "csi_collector.h"   /* node id + live CSI packet rate */
#include "sdkconfig.h"

#if CONFIG_DISPLAY_ENABLE && CONFIG_DISPLAY_PANEL_ST7789

#include <stdio.h>
#include "esp_log.h"
#include "esp_timer.h"
#include "esp_system.h"
#include "edge_processing.h"

static const char *TAG = "disp_ui_st7789";

#define COLOR_BG     lv_color_make(0x0A, 0x0A, 0x0F)
#define COLOR_CYAN   lv_color_make(0x00, 0xD4, 0xFF)
#define COLOR_TEXT   lv_color_make(0xCC, 0xCC, 0xDD)
#define COLOR_DIM    lv_color_make(0x66, 0x66, 0x77)
#define COLOR_GREEN  lv_color_make(0x00, 0xFF, 0x80)
#define COLOR_TRACK  lv_color_make(0x1C, 0x1C, 0x26)

/* Activity = max(motion_energy, presence_score); ~0 idle, climbs past 6 on
 * movement near the link. Scale x10 to a 0-100 bar (clamped). */
#define ACT_SCALE   10.0f

static lv_obj_t *s_node = NULL;
static lv_obj_t *s_act_val = NULL;   /* big activity number */
static lv_obj_t *s_bar = NULL;       /* activity bar */
static lv_obj_t *s_foot = NULL;      /* CSI rate + RSSI */
static lv_obj_t *s_foot2 = NULL;     /* uptime + heap */
static lv_obj_t *s_hb = NULL;        /* heartbeat dot */

static lv_obj_t *label(lv_obj_t *p, const lv_font_t *font, lv_color_t color,
                       lv_align_t align, int x, int y, const char *text)
{
    lv_obj_t *l = lv_label_create(p);
    lv_label_set_text(l, text);
    lv_obj_set_style_text_font(l, font, 0);
    lv_obj_set_style_text_color(l, color, 0);
    lv_obj_align(l, align, x, y);
    return l;
}

void display_ui_create(lv_obj_t *parent)
{
    lv_obj_set_style_bg_color(parent, COLOR_BG, 0);
    lv_obj_set_style_bg_opa(parent, LV_OPA_COVER, 0);

    label(parent, &lv_font_montserrat_20, COLOR_DIM, LV_ALIGN_TOP_MID, 0, 6, "RuView CSI");
    s_node = label(parent, &lv_font_montserrat_36, COLOR_CYAN, LV_ALIGN_TOP_MID, 0, 30, "NODE -");
    s_hb   = label(parent, &lv_font_montserrat_20, COLOR_CYAN, LV_ALIGN_TOP_RIGHT, -10, 8, "*");

    label(parent, &lv_font_montserrat_20, COLOR_TEXT, LV_ALIGN_TOP_MID, 0, 92, "ACTIVITY");
    s_act_val = label(parent, &lv_font_montserrat_36, COLOR_GREEN, LV_ALIGN_TOP_MID, 0, 116, "0");

    s_bar = lv_bar_create(parent);
    lv_obj_set_size(s_bar, 200, 24);
    lv_obj_align(s_bar, LV_ALIGN_TOP_MID, 0, 172);
    lv_bar_set_range(s_bar, 0, 100);
    lv_bar_set_value(s_bar, 0, LV_ANIM_OFF);
    lv_obj_set_style_bg_color(s_bar, COLOR_TRACK, LV_PART_MAIN);
    lv_obj_set_style_bg_color(s_bar, COLOR_GREEN, LV_PART_INDICATOR);
    lv_obj_set_style_radius(s_bar, 4, LV_PART_MAIN);

    s_foot  = label(parent, &lv_font_montserrat_14, COLOR_TEXT, LV_ALIGN_BOTTOM_MID, 0, -26, "CSI --/s   RSSI --");
    s_foot2 = label(parent, &lv_font_montserrat_14, COLOR_DIM,  LV_ALIGN_BOTTOM_MID, 0, -6,  "up 0h00m   heap -- KB");

    ESP_LOGI(TAG, "Compact ST7789 UI created (240x280, activity bar)");
}

void display_ui_update(void)
{
    char buf[48];

    /* Heartbeat — blink ~2 Hz so the UI is visibly alive regardless of data. */
    static uint32_t s_hb_last = 0;
    static bool s_hb_on = false;
    uint32_t now_ms = (uint32_t)(esp_timer_get_time() / 1000);
    if (now_ms - s_hb_last >= 500) {
        s_hb_last = now_ms;
        s_hb_on = !s_hb_on;
        lv_label_set_text(s_hb, s_hb_on ? "*" : " ");
    }

    snprintf(buf, sizeof(buf), "NODE %u", (unsigned)csi_collector_get_node_id());
    lv_label_set_text(s_node, buf);

    uint16_t pps = csi_collector_get_pkt_yield_per_sec();

    edge_vitals_pkt_t v;
    int rssi = 0;
    float activity = 0.0f;
    if (edge_get_vitals(&v)) {
        rssi = v.rssi;
        activity = v.motion_energy > v.presence_score ? v.motion_energy : v.presence_score;
    }

    int act100 = (int)(activity * ACT_SCALE);
    if (act100 > 100) act100 = 100;
    if (act100 < 0) act100 = 0;
    lv_bar_set_value(s_bar, act100, LV_ANIM_OFF);

    snprintf(buf, sizeof(buf), "%d", act100);
    lv_label_set_text(s_act_val, buf);
    lv_obj_set_style_text_color(s_act_val, act100 > 10 ? COLOR_GREEN : COLOR_DIM, 0);

    snprintf(buf, sizeof(buf), "CSI %u/s   RSSI %d", (unsigned)pps, rssi);
    lv_label_set_text(s_foot, buf);

    uint32_t up = (uint32_t)(esp_timer_get_time() / 1000000);
    snprintf(buf, sizeof(buf), "up %luh%02lum   heap %luK",
             (unsigned long)(up / 3600), (unsigned long)((up % 3600) / 60),
             (unsigned long)(esp_get_free_heap_size() / 1024));
    lv_label_set_text(s_foot2, buf);

    /* NOTE: no per-loop ESP_LOG here. On the USB-Serial-JTAG console, logging
     * with no host attached (e.g. running off a wall charger) blocks once the
     * TX buffer fills — which would hang the display task. Keep this loop
     * log-free so the panel keeps refreshing headless. */
}

#endif /* CONFIG_DISPLAY_ENABLE && CONFIG_DISPLAY_PANEL_ST7789 */
