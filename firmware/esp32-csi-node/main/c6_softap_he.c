/**
 * @file c6_softap_he.c
 * @brief ESP32-C6 soft-AP with HE/TWT — ADR-110 B1/B2 cheap-unblock.
 *
 * Pairs with c6_softap_he.h. Builds only when both targets are set:
 *
 *   CONFIG_IDF_TARGET_ESP32C6    (selected by `idf.py set-target esp32c6`)
 *   CONFIG_C6_SOFTAP_HE_ENABLE   (Kconfig, default n)
 *
 * The IDF v5.4 soft-AP path advertises HE automatically on chips with
 * SOC_WIFI_HE_SUPPORT; the operator-side concern here is making sure
 * the beacon also advertises `TWT Responder=1` so a STA-side
 * `esp_wifi_sta_itwt_setup()` call doesn't bounce with `INVALID_ARG`
 * the same way it did against `ruv.net` (the bench's 11n-only AP).
 *
 * TWT Responder advertisement in IDF v5.4 is gated by
 * `wifi_he_ap_config_t.twt_responder = 1`. When the IDF header doesn't
 * expose that struct (older v5.3), the AP still comes up with HE but
 * without TWT Responder — we log a warning and continue so the build
 * stays portable.
 */

#include "sdkconfig.h"

#if (defined(CONFIG_IDF_TARGET_ESP32C6) || defined(CONFIG_IDF_TARGET_ESP32C5)) && defined(CONFIG_C6_SOFTAP_HE_ENABLE)

#include "c6_softap_he.h"
#include "esp_log.h"
#include "esp_wifi.h"
#include "esp_wifi_types.h"
#include "esp_event.h"
#include "esp_netif.h"
#include "nvs_flash.h"
#include "nvs.h"
#include <string.h>

static const char *TAG = "c6_softap";

static bool    s_started   = false;
static uint8_t s_sta_count = 0;
static uint8_t s_channel   = 0;

#ifndef CONFIG_C6_SOFTAP_HE_SSID
#define CONFIG_C6_SOFTAP_HE_SSID    "ruview-c6-twt"
#endif
#ifndef CONFIG_C6_SOFTAP_HE_PSK
#define CONFIG_C6_SOFTAP_HE_PSK     "ruviewtwt"
#endif
#ifndef CONFIG_C6_SOFTAP_HE_CHANNEL
#define CONFIG_C6_SOFTAP_HE_CHANNEL 6
#endif

static void load_nvs_override(const char *key, char *dst, size_t dst_len)
{
    nvs_handle_t h;
    if (nvs_open("ruview", NVS_READONLY, &h) != ESP_OK) return;
    size_t n = dst_len;
    esp_err_t err = nvs_get_str(h, key, dst, &n);
    if (err == ESP_OK) {
        ESP_LOGI(TAG, "nvs override: %s=\"%s\"", key, dst);
    }
    nvs_close(h);
}

static uint8_t load_nvs_u8(const char *key, uint8_t fallback)
{
    nvs_handle_t h;
    if (nvs_open("ruview", NVS_READONLY, &h) != ESP_OK) return fallback;
    uint8_t v = fallback;
    if (nvs_get_u8(h, key, &v) == ESP_OK) {
        ESP_LOGI(TAG, "nvs override: %s=%u", key, v);
    }
    nvs_close(h);
    return v;
}

static void on_wifi_event(void *arg, esp_event_base_t base,
                          int32_t event_id, void *event_data)
{
    (void)arg; (void)base; (void)event_data;
    switch (event_id) {
    case WIFI_EVENT_AP_START:
        s_started = true;
        ESP_LOGI(TAG, "AP started on channel %u", s_channel);
        break;
    case WIFI_EVENT_AP_STOP:
        s_started = false;
        ESP_LOGI(TAG, "AP stopped");
        break;
    case WIFI_EVENT_AP_STACONNECTED:
        if (s_sta_count < 255) s_sta_count++;
        ESP_LOGI(TAG, "STA connected — total=%u", s_sta_count);
        break;
    case WIFI_EVENT_AP_STADISCONNECTED:
        if (s_sta_count > 0) s_sta_count--;
        ESP_LOGI(TAG, "STA disconnected — total=%u", s_sta_count);
        break;
    default:
        break;
    }
}

esp_err_t c6_softap_he_start(uint8_t *out_channel)
{
    if (s_started) {
        if (out_channel) *out_channel = s_channel;
        return ESP_OK;
    }

    /* Resolve config: NVS overrides Kconfig defaults. */
    char ssid[33] = CONFIG_C6_SOFTAP_HE_SSID;
    char psk[64]  = CONFIG_C6_SOFTAP_HE_PSK;
    load_nvs_override("softap_ssid", ssid, sizeof(ssid));
    load_nvs_override("softap_psk",  psk,  sizeof(psk));
    s_channel = load_nvs_u8("softap_chan", CONFIG_C6_SOFTAP_HE_CHANNEL);
    if (s_channel < 1 || s_channel > 13) s_channel = CONFIG_C6_SOFTAP_HE_CHANNEL;

    /* AP+STA so the existing STA path keeps working (NVS-provisioned upstream). */
    ESP_ERROR_CHECK(esp_wifi_set_mode(WIFI_MODE_APSTA));

    wifi_config_t ap_cfg = {0};
    size_t ssid_len = strlen(ssid);
    if (ssid_len > 32) ssid_len = 32;
    memcpy(ap_cfg.ap.ssid, ssid, ssid_len);
    ap_cfg.ap.ssid_len = (uint8_t)ssid_len;
    strlcpy((char *)ap_cfg.ap.password, psk, sizeof(ap_cfg.ap.password));
    ap_cfg.ap.channel        = s_channel;
    ap_cfg.ap.max_connection = 4;
    ap_cfg.ap.authmode       = strlen(psk) >= 8 ? WIFI_AUTH_WPA2_PSK : WIFI_AUTH_OPEN;
    ap_cfg.ap.beacon_interval = 100;
    /* pmf_cfg.required = false keeps backward compatibility for STA clients
     * that don't speak PMF. */
    ap_cfg.ap.pmf_cfg.required = false;

    /* Register the event handler before bringing the AP up so we don't
     * miss WIFI_EVENT_AP_START. */
    ESP_ERROR_CHECK(esp_event_handler_instance_register(
        WIFI_EVENT, ESP_EVENT_ANY_ID, on_wifi_event, NULL, NULL));

    esp_err_t err = esp_wifi_set_config(WIFI_IF_AP, &ap_cfg);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "set_config(AP) failed: %s", esp_err_to_name(err));
        return err;
    }

    /* IDF v5.4 LIMIT (verified empirically 2026-05-23 — WITNESS-LOG-110 §A0.6):
     * the public API exposes ONLY STA-side iTWT/bTWT (esp_wifi_sta_itwt_*,
     * esp_wifi_sta_btwt_*). There is NO esp_wifi_ap_set_he_config(), NO
     * wifi_he_ap_config_t, and NO wifi_config_t.ap.he_* field. A second C6
     * associating against this soft-AP currently lands at phymode 11bgn
     * (he:0, vht:0, ht:1) — the AP doesn't advertise HE because there's no
     * way to ask it to. A future IDF release that exposes AP-side HE config
     * (or a patched WiFi blob) is required to make this AP iTWT-capable.
     *
     * Until then, this module still gives you a working WPA2 soft-AP on a
     * controlled channel for AP+STA bench experiments and ESP-NOW peer
     * discovery — just not iTWT validation. The c6_twt module on the STA
     * side will return ESP_ERR_INVALID_ARG against this AP (no TWT Responder
     * in the beacon), exactly as it does against any other 11n-only AP. */
    ESP_LOGI(TAG, "soft-AP starting: ssid=\"%s\" channel=%u auth=%s",
             ssid, s_channel,
             ap_cfg.ap.authmode == WIFI_AUTH_OPEN ? "open" : "wpa2-psk");
    ESP_LOGW(TAG, "IDF v5.4 soft-AP does NOT advertise HE — STAs will associate at 11bgn. "
                  "iTWT validation requires an external 11ax AP. See WITNESS-LOG-110 §A0.6.");

    /* Don't call esp_wifi_start() here — main.c brings the WiFi up once
     * for both AP and STA. We just configured the AP iface so it joins
     * the existing start. */

    if (out_channel) *out_channel = s_channel;
    return ESP_OK;
}

bool c6_softap_he_is_up(void)        { return s_started; }
uint8_t c6_softap_he_sta_count(void) { return s_sta_count; }

#endif  /* CONFIG_IDF_TARGET_ESP32C6 && CONFIG_C6_SOFTAP_HE_ENABLE */
