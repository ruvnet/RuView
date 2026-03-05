/**
 * @file display_ui.h
 * @brief ADR-045: LVGL 4-view swipeable UI for CSI node stats.
 *
 * Views: Dashboard | Vitals | Presence | System
 * Dark theme with cyan (#00d4ff) accent.
 */

#ifndef DISPLAY_UI_H
#define DISPLAY_UI_H

#include "lvgl.h"

#ifdef __cplusplus
extern "C" {
#endif

/** Create all LVGL views on the given tileview parent. */
void display_ui_create(lv_obj_t *parent);

/**
 * Update all views with latest data. Called every display refresh cycle.
 * Reads from edge_get_vitals() and edge_get_multi_person() internally.
 */
void display_ui_update(void);

#ifdef __cplusplus
}
#endif

#endif /* DISPLAY_UI_H */
