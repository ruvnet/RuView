/**
 * @file rv_radio_ops_esp32.c
 * @brief ADR-081 Layer 1 — ESP32 binding for rv_radio_ops_t.
 *
 * Wraps the existing csi_collector + esp_wifi_* surface so the adaptive
 * controller, mesh plane, and feature-extraction layers can address the
 * radio through a single chipset-agnostic vtable.
 *
 * This is intentionally thin. The heavy lifting still lives in
 * csi_collector.c (CSI callback, channel hopping, NDP injection); this file
 * is the contract that lets a second chipset (Nexmon Broadcom, custom
 * silicon) drop in without touching the layers above.
 */

#include "rv_radio_ops.h"
#include "csi_collector.h"

#include <string.h>
#include "esp_err.h"
#include "esp_log.h"
#include "esp_wifi.h"

static const char *TAG = "rv_radio_esp32";

/* ---- Active ops registry ---- */

static const rv_radio_ops_t *s_active_ops = NULL;

void rv_radio_ops_register(const rv_radio_ops_t *ops)
{
    s_active_ops = ops;
}

const rv_radio_ops_t *rv_radio_ops_get(void)
{
    return s_active_ops;
}

/* ---- ESP32 binding state ---- */

static uint8_t  s_current_channel = 1;
static uint8_t  s_current_bw      = 20;
static uint8_t  s_current_profile = RV_PROFILE_PASSIVE_LOW_RATE;
static uint8_t  s_current_mode    = RV_RADIO_MODE_PASSIVE_RX;
static bool     s_csi_enabled     = true;

/* ---- Vtable implementations ---- */

static int esp32_init(void)
{
    /* csi_collector_init() is called from app_main() before the controller
     * starts; nothing to do here for the ESP32 binding. We just confirm a
     * valid current channel was captured by csi_collector_init(). */
    ESP_LOGI(TAG, "ESP32 radio ops: init (current ch=%u bw=%u)",
             (unsigned)s_current_channel, (unsigned)s_current_bw);
    return ESP_OK;
}

static int esp32_set_channel(uint8_t ch, uint8_t bw)
{
    wifi_second_chan_t second = WIFI_SECOND_CHAN_NONE;
    if (bw == 40) {
        /* HT40+: secondary channel above primary. The controller never asks
         * for HT40 today (sensing prefers HT20), but the mapping is here so
         * a future profile can. */
        second = WIFI_SECOND_CHAN_ABOVE;
    } else if (bw != 20) {
        ESP_LOGW(TAG, "set_channel: unsupported bw=%u, treating as 20 MHz",
                 (unsigned)bw);
        bw = 20;
    }

    esp_err_t err = esp_wifi_set_channel(ch, second);
    if (err != ESP_OK) {
        ESP_LOGW(TAG, "set_channel(%u, bw=%u) failed: %s",
                 (unsigned)ch, (unsigned)bw, esp_err_to_name(err));
        return (int)err;
    }
    s_current_channel = ch;
    s_current_bw      = bw;
    return ESP_OK;
}

static int esp32_set_mode(uint8_t mode)
{
    /* Persist the mode for the health snapshot; actual TX behavior is
     * triggered by the controller calling csi_inject_ndp_frame() directly
     * once the controller PR lands. For now this is bookkeeping plus a
     * passive/active probe gate. */
    switch (mode) {
    case RV_RADIO_MODE_DISABLED:
    case RV_RADIO_MODE_PASSIVE_RX:
    case RV_RADIO_MODE_ACTIVE_PROBE:
    case RV_RADIO_MODE_CALIBRATION:
        s_current_mode = mode;
        return ESP_OK;
    default:
        ESP_LOGW(TAG, "set_mode: unknown mode %u", (unsigned)mode);
        return ESP_ERR_INVALID_ARG;
    }
}

static int esp32_set_csi_enabled(bool en)
{
    esp_err_t err = esp_wifi_set_csi(en);
    if (err != ESP_OK) {
        ESP_LOGW(TAG, "set_csi(%d) failed: %s", (int)en, esp_err_to_name(err));
        return (int)err;
    }
    s_csi_enabled = en;
    return ESP_OK;
}

static int esp32_set_capture_profile(uint8_t profile_id)
{
    if (profile_id >= RV_PROFILE_COUNT) {
        ESP_LOGW(TAG, "set_capture_profile: invalid id %u", (unsigned)profile_id);
        return ESP_ERR_INVALID_ARG;
    }

    /* Profiles are advisory at this layer — the controller uses them to
     * decide cadence/window/threshold for the layers above. The radio
     * binding records the active profile for health reporting and may
     * adjust the underlying TX/RX mode in future bindings. */
    s_current_profile = profile_id;

    /* For ACTIVE_PROBE and CALIBRATION, switch the radio mode to match. */
    if (profile_id == RV_PROFILE_ACTIVE_PROBE) {
        esp32_set_mode(RV_RADIO_MODE_ACTIVE_PROBE);
    } else if (profile_id == RV_PROFILE_CALIBRATION) {
        esp32_set_mode(RV_RADIO_MODE_CALIBRATION);
    } else {
        esp32_set_mode(RV_RADIO_MODE_PASSIVE_RX);
    }
    return ESP_OK;
}

static int esp32_get_health(rv_radio_health_t *out)
{
    if (out == NULL) {
        return ESP_ERR_INVALID_ARG;
    }
    memset(out, 0, sizeof(*out));

    out->pkt_yield_per_sec = csi_collector_get_pkt_yield_per_sec();
    out->send_fail_count   = csi_collector_get_send_fail_count();
    out->current_channel   = s_current_channel;
    out->current_bw_mhz    = s_current_bw;
    out->current_profile   = s_current_profile;

    wifi_ap_record_t ap = {0};
    if (esp_wifi_sta_get_ap_info(&ap) == ESP_OK) {
        out->rssi_median_dbm = ap.rssi;
    }
    return ESP_OK;
}

/* ---- The vtable instance ---- */

static const rv_radio_ops_t s_esp32_ops = {
    .init                 = esp32_init,
    .set_channel          = esp32_set_channel,
    .set_mode             = esp32_set_mode,
    .set_csi_enabled      = esp32_set_csi_enabled,
    .set_capture_profile  = esp32_set_capture_profile,
    .get_health           = esp32_get_health,
};

void rv_radio_ops_esp32_register(void)
{
    if (s_active_ops == &s_esp32_ops) {
        return;  /* idempotent */
    }
    rv_radio_ops_register(&s_esp32_ops);
    ESP_LOGI(TAG, "ESP32 radio ops registered as active binding");
}
