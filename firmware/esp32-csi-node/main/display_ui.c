/**
 * @file display_ui.c
 * @brief ADR-045: LVGL 4-view swipeable UI — Dashboard | Vitals | Presence | System.
 *
 * Dark theme (#0a0a0f background) with cyan (#00d4ff) accent.
 * Glowing line effects via layered semi-transparent chart series.
 */

#include "display_ui.h"
#include "sdkconfig.h"

#if CONFIG_DISPLAY_ENABLE

#include <stdio.h>
#include <string.h>
#include "esp_log.h"
#include "esp_system.h"
#include "esp_timer.h"
#include "esp_heap_caps.h"
#include "edge_processing.h"

static const char *TAG = "disp_ui";

/* ---- Theme colors ---- */
#define COLOR_BG        lv_color_make(0x0A, 0x0A, 0x0F)
#define COLOR_CYAN      lv_color_make(0x00, 0xD4, 0xFF)
#define COLOR_AMBER     lv_color_make(0xFF, 0xB0, 0x00)
#define COLOR_GREEN     lv_color_make(0x00, 0xFF, 0x80)
#define COLOR_RED       lv_color_make(0xFF, 0x40, 0x40)
#define COLOR_DIM       lv_color_make(0x30, 0x30, 0x40)
#define COLOR_TEXT       lv_color_make(0xCC, 0xCC, 0xDD)
#define COLOR_TEXT_DIM   lv_color_make(0x66, 0x66, 0x77)

/* ---- Chart data points ---- */
#define CHART_POINTS    60

/* ---- View handles ---- */
static lv_obj_t *s_tileview = NULL;

/* Dashboard */
static lv_obj_t *s_dash_chart      = NULL;
static lv_chart_series_t *s_csi_series = NULL;
static lv_obj_t *s_dash_persons    = NULL;
static lv_obj_t *s_dash_rssi       = NULL;
static lv_obj_t *s_dash_motion     = NULL;

/* Vitals */
static lv_obj_t *s_vital_chart     = NULL;
static lv_chart_series_t *s_breath_series = NULL;
static lv_chart_series_t *s_hr_series     = NULL;
static lv_obj_t *s_vital_bpm_br    = NULL;
static lv_obj_t *s_vital_bpm_hr    = NULL;

/* Presence */
#define GRID_COLS  4
#define GRID_ROWS  4
static lv_obj_t *s_grid_cells[GRID_COLS * GRID_ROWS];
static lv_obj_t *s_presence_label = NULL;

/* System */
static lv_obj_t *s_sys_cpu         = NULL;
static lv_obj_t *s_sys_heap        = NULL;
static lv_obj_t *s_sys_psram       = NULL;
static lv_obj_t *s_sys_rssi        = NULL;
static lv_obj_t *s_sys_uptime      = NULL;
static lv_obj_t *s_sys_fps         = NULL;
static lv_obj_t *s_sys_node        = NULL;

/* ---- Style helpers ---- */
static lv_style_t s_style_bg;
static lv_style_t s_style_label;
static lv_style_t s_style_label_big;
static bool s_styles_inited = false;

static void init_styles(void)
{
    if (s_styles_inited) return;
    s_styles_inited = true;

    lv_style_init(&s_style_bg);
    lv_style_set_bg_color(&s_style_bg, COLOR_BG);
    lv_style_set_bg_opa(&s_style_bg, LV_OPA_COVER);
    lv_style_set_border_width(&s_style_bg, 0);
    lv_style_set_pad_all(&s_style_bg, 4);

    lv_style_init(&s_style_label);
    lv_style_set_text_color(&s_style_label, COLOR_TEXT);
    lv_style_set_text_font(&s_style_label, &lv_font_montserrat_14);

    lv_style_init(&s_style_label_big);
    lv_style_set_text_color(&s_style_label_big, COLOR_CYAN);
    lv_style_set_text_font(&s_style_label_big, &lv_font_montserrat_14);
}

static lv_obj_t *make_label(lv_obj_t *parent, const char *text, const lv_style_t *style)
{
    lv_obj_t *lbl = lv_label_create(parent);
    lv_label_set_text(lbl, text);
    if (style) lv_obj_add_style(lbl, (lv_style_t *)style, 0);
    return lbl;
}

static lv_obj_t *make_tile(lv_obj_t *tv, uint8_t col, uint8_t row)
{
    lv_obj_t *tile = lv_tileview_add_tile(tv, col, row, LV_DIR_HOR);
    lv_obj_add_style(tile, &s_style_bg, 0);
    return tile;
}

/* ---- View 0: Dashboard ---- */
static void create_dashboard(lv_obj_t *tile)
{
    make_label(tile, "CSI Dashboard", &s_style_label);

    /* CSI amplitude chart */
    s_dash_chart = lv_chart_create(tile);
    lv_obj_set_size(s_dash_chart, 400, 130);
    lv_obj_align(s_dash_chart, LV_ALIGN_TOP_LEFT, 0, 24);
    lv_chart_set_type(s_dash_chart, LV_CHART_TYPE_LINE);
    lv_chart_set_point_count(s_dash_chart, CHART_POINTS);
    lv_chart_set_range(s_dash_chart, LV_CHART_AXIS_PRIMARY_Y, 0, 100);
    lv_obj_set_style_bg_color(s_dash_chart, COLOR_BG, 0);
    lv_obj_set_style_border_color(s_dash_chart, COLOR_DIM, 0);
    lv_obj_set_style_line_width(s_dash_chart, 0, LV_PART_TICKS);

    s_csi_series = lv_chart_add_series(s_dash_chart, COLOR_CYAN, LV_CHART_AXIS_PRIMARY_Y);

    /* Stats panel on the right */
    lv_obj_t *panel = lv_obj_create(tile);
    lv_obj_set_size(panel, 120, 130);
    lv_obj_align(panel, LV_ALIGN_TOP_RIGHT, 0, 24);
    lv_obj_set_style_bg_color(panel, lv_color_make(0x12, 0x12, 0x1A), 0);
    lv_obj_set_style_border_width(panel, 1, 0);
    lv_obj_set_style_border_color(panel, COLOR_DIM, 0);
    lv_obj_set_style_pad_all(panel, 8, 0);
    lv_obj_set_flex_flow(panel, LV_FLEX_FLOW_COLUMN);
    lv_obj_set_flex_align(panel, LV_FLEX_ALIGN_SPACE_EVENLY, LV_FLEX_ALIGN_START, LV_FLEX_ALIGN_START);

    make_label(panel, "Persons", &s_style_label);
    s_dash_persons = make_label(panel, "0", &s_style_label_big);

    s_dash_rssi = make_label(panel, "RSSI: --", &s_style_label);
    s_dash_motion = make_label(panel, "Motion: 0.0", &s_style_label);
}

/* ---- View 1: Vitals ---- */
static void create_vitals(lv_obj_t *tile)
{
    make_label(tile, "Vital Signs", &s_style_label);

    s_vital_chart = lv_chart_create(tile);
    lv_obj_set_size(s_vital_chart, 480, 150);
    lv_obj_align(s_vital_chart, LV_ALIGN_TOP_LEFT, 0, 24);
    lv_chart_set_type(s_vital_chart, LV_CHART_TYPE_LINE);
    lv_chart_set_point_count(s_vital_chart, CHART_POINTS);
    lv_chart_set_range(s_vital_chart, LV_CHART_AXIS_PRIMARY_Y, 0, 120);
    lv_obj_set_style_bg_color(s_vital_chart, COLOR_BG, 0);
    lv_obj_set_style_border_color(s_vital_chart, COLOR_DIM, 0);
    lv_obj_set_style_line_width(s_vital_chart, 0, LV_PART_TICKS);

    /* Breathing series (cyan) */
    s_breath_series = lv_chart_add_series(s_vital_chart, COLOR_CYAN, LV_CHART_AXIS_PRIMARY_Y);
    /* Heart rate series (amber) */
    s_hr_series = lv_chart_add_series(s_vital_chart, COLOR_AMBER, LV_CHART_AXIS_PRIMARY_Y);

    /* BPM readouts */
    s_vital_bpm_br = make_label(tile, "Breathing: -- BPM", &s_style_label);
    lv_obj_align(s_vital_bpm_br, LV_ALIGN_BOTTOM_LEFT, 4, -8);
    lv_obj_set_style_text_color(s_vital_bpm_br, COLOR_CYAN, 0);

    s_vital_bpm_hr = make_label(tile, "Heart Rate: -- BPM", &s_style_label);
    lv_obj_align(s_vital_bpm_hr, LV_ALIGN_BOTTOM_RIGHT, -4, -8);
    lv_obj_set_style_text_color(s_vital_bpm_hr, COLOR_AMBER, 0);
}

/* ---- View 2: Presence Grid ---- */
static void create_presence(lv_obj_t *tile)
{
    make_label(tile, "Occupancy Map", &s_style_label);

    int cell_w = 50;
    int cell_h = 45;
    int x_off  = (368 - GRID_COLS * (cell_w + 4)) / 2;
    int y_off  = 30;

    for (int r = 0; r < GRID_ROWS; r++) {
        for (int c = 0; c < GRID_COLS; c++) {
            lv_obj_t *cell = lv_obj_create(tile);
            lv_obj_set_size(cell, cell_w, cell_h);
            lv_obj_set_pos(cell, x_off + c * (cell_w + 4), y_off + r * (cell_h + 4));
            lv_obj_set_style_bg_color(cell, COLOR_DIM, 0);
            lv_obj_set_style_bg_opa(cell, LV_OPA_COVER, 0);
            lv_obj_set_style_border_color(cell, COLOR_DIM, 0);
            lv_obj_set_style_border_width(cell, 1, 0);
            lv_obj_set_style_radius(cell, 4, 0);
            s_grid_cells[r * GRID_COLS + c] = cell;
        }
    }

    s_presence_label = make_label(tile, "Persons: 0", &s_style_label);
    lv_obj_align(s_presence_label, LV_ALIGN_BOTTOM_MID, 0, -8);
}

/* ---- View 3: System ---- */
static void create_system(lv_obj_t *tile)
{
    make_label(tile, "System Info", &s_style_label);

    lv_obj_t *panel = lv_obj_create(tile);
    lv_obj_set_size(panel, 500, 180);
    lv_obj_align(panel, LV_ALIGN_TOP_LEFT, 0, 24);
    lv_obj_set_style_bg_color(panel, lv_color_make(0x12, 0x12, 0x1A), 0);
    lv_obj_set_style_border_width(panel, 1, 0);
    lv_obj_set_style_border_color(panel, COLOR_DIM, 0);
    lv_obj_set_style_pad_all(panel, 10, 0);
    lv_obj_set_flex_flow(panel, LV_FLEX_FLOW_COLUMN);
    lv_obj_set_flex_align(panel, LV_FLEX_ALIGN_SPACE_EVENLY, LV_FLEX_ALIGN_START, LV_FLEX_ALIGN_START);

    s_sys_node   = make_label(panel, "Node: --",        &s_style_label);
    s_sys_cpu    = make_label(panel, "CPU: --%",         &s_style_label);
    s_sys_heap   = make_label(panel, "Heap: -- KB free", &s_style_label);
    s_sys_psram  = make_label(panel, "PSRAM: -- KB free",&s_style_label);
    s_sys_rssi   = make_label(panel, "WiFi RSSI: --",   &s_style_label);
    s_sys_uptime = make_label(panel, "Uptime: --",      &s_style_label);
    s_sys_fps    = make_label(panel, "FPS: --",          &s_style_label);
}

/* ---- Public API ---- */

void display_ui_create(lv_obj_t *parent)
{
    init_styles();

    s_tileview = lv_tileview_create(parent);
    lv_obj_add_style(s_tileview, &s_style_bg, 0);
    lv_obj_set_style_bg_color(s_tileview, COLOR_BG, 0);

    lv_obj_t *t0 = make_tile(s_tileview, 0, 0);
    lv_obj_t *t1 = make_tile(s_tileview, 1, 0);
    lv_obj_t *t2 = make_tile(s_tileview, 2, 0);
    lv_obj_t *t3 = make_tile(s_tileview, 3, 0);

    create_dashboard(t0);
    create_vitals(t1);
    create_presence(t2);
    create_system(t3);

    ESP_LOGI(TAG, "UI created: 4 views (Dashboard|Vitals|Presence|System)");
}

/* ---- FPS tracking ---- */
static uint32_t s_frame_count = 0;
static uint32_t s_last_fps_time = 0;
static uint32_t s_current_fps = 0;

void display_ui_update(void)
{
    /* FPS counter */
    s_frame_count++;
    uint32_t now_ms = (uint32_t)(esp_timer_get_time() / 1000);
    if (now_ms - s_last_fps_time >= 1000) {
        s_current_fps = s_frame_count;
        s_frame_count = 0;
        s_last_fps_time = now_ms;
    }

    /* Read edge data (thread-safe) */
    edge_vitals_pkt_t vitals;
    bool has_vitals = edge_get_vitals(&vitals);

    edge_person_vitals_t persons[EDGE_MAX_PERSONS];
    uint8_t n_active = 0;
    edge_get_multi_person(persons, &n_active);

    /* ---- Dashboard update ---- */
    if (s_dash_chart && has_vitals) {
        /* Push motion energy as amplitude proxy (scaled 0-100) */
        int val = (int)(vitals.motion_energy * 10.0f);
        if (val > 100) val = 100;
        if (val < 0) val = 0;
        lv_chart_set_next_value(s_dash_chart, s_csi_series, val);
    }

    if (s_dash_persons) {
        char buf[8];
        snprintf(buf, sizeof(buf), "%u", has_vitals ? vitals.n_persons : 0);
        lv_label_set_text(s_dash_persons, buf);
    }

    if (s_dash_rssi && has_vitals) {
        char buf[16];
        snprintf(buf, sizeof(buf), "RSSI: %d", vitals.rssi);
        lv_label_set_text(s_dash_rssi, buf);
    }

    if (s_dash_motion && has_vitals) {
        char buf[24];
        snprintf(buf, sizeof(buf), "Motion: %.1f", (double)vitals.motion_energy);
        lv_label_set_text(s_dash_motion, buf);
    }

    /* ---- Vitals update ---- */
    if (s_vital_chart && has_vitals) {
        int br = (int)(vitals.breathing_rate / 100);  /* Fixed-point to int BPM */
        int hr = (int)(vitals.heartrate / 10000);
        if (br > 120) br = 120;
        if (hr > 120) hr = 120;
        lv_chart_set_next_value(s_vital_chart, s_breath_series, br);
        lv_chart_set_next_value(s_vital_chart, s_hr_series, hr);

        char buf[32];
        snprintf(buf, sizeof(buf), "Breathing: %d BPM", br);
        lv_label_set_text(s_vital_bpm_br, buf);

        snprintf(buf, sizeof(buf), "Heart Rate: %d BPM", hr);
        lv_label_set_text(s_vital_bpm_hr, buf);
    }

    /* ---- Presence grid update ---- */
    if (has_vitals) {
        /* Simple visualization: color cells based on motion energy distribution */
        float energy = vitals.motion_energy;
        uint8_t active_cells = (uint8_t)(energy * 2);  /* Scale for visibility */
        if (active_cells > GRID_COLS * GRID_ROWS) active_cells = GRID_COLS * GRID_ROWS;

        for (int i = 0; i < GRID_COLS * GRID_ROWS; i++) {
            if (i < active_cells) {
                /* Color gradient: green → amber → red based on intensity */
                if (energy > 5.0f) {
                    lv_obj_set_style_bg_color(s_grid_cells[i], COLOR_RED, 0);
                } else if (energy > 2.0f) {
                    lv_obj_set_style_bg_color(s_grid_cells[i], COLOR_AMBER, 0);
                } else {
                    lv_obj_set_style_bg_color(s_grid_cells[i], COLOR_GREEN, 0);
                }
            } else {
                lv_obj_set_style_bg_color(s_grid_cells[i], COLOR_DIM, 0);
            }
        }

        char buf[20];
        snprintf(buf, sizeof(buf), "Persons: %u", vitals.n_persons);
        lv_label_set_text(s_presence_label, buf);
    }

    /* ---- System info update ---- */
    {
        char buf[48];

#ifdef CONFIG_CSI_NODE_ID
        snprintf(buf, sizeof(buf), "Node: %d", CONFIG_CSI_NODE_ID);
#else
        snprintf(buf, sizeof(buf), "Node: --");
#endif
        lv_label_set_text(s_sys_node, buf);

        snprintf(buf, sizeof(buf), "Heap: %lu KB free",
                 (unsigned long)(esp_get_free_heap_size() / 1024));
        lv_label_set_text(s_sys_heap, buf);

#if CONFIG_SPIRAM
        snprintf(buf, sizeof(buf), "PSRAM: %lu KB free",
                 (unsigned long)(heap_caps_get_free_size(MALLOC_CAP_SPIRAM) / 1024));
#else
        snprintf(buf, sizeof(buf), "PSRAM: N/A");
#endif
        lv_label_set_text(s_sys_psram, buf);

        if (has_vitals) {
            snprintf(buf, sizeof(buf), "WiFi RSSI: %d dBm", vitals.rssi);
            lv_label_set_text(s_sys_rssi, buf);
        }

        uint32_t uptime_s = (uint32_t)(esp_timer_get_time() / 1000000);
        uint32_t h = uptime_s / 3600;
        uint32_t m = (uptime_s % 3600) / 60;
        uint32_t s = uptime_s % 60;
        snprintf(buf, sizeof(buf), "Uptime: %luh %02lum %02lus",
                 (unsigned long)h, (unsigned long)m, (unsigned long)s);
        lv_label_set_text(s_sys_uptime, buf);

        snprintf(buf, sizeof(buf), "FPS: %lu", (unsigned long)s_current_fps);
        lv_label_set_text(s_sys_fps, buf);
    }
}

#endif /* CONFIG_DISPLAY_ENABLE */
