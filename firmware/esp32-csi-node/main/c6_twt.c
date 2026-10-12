/**
 * @file c6_twt.c
 * @brief ESP32-C6 TWT setup implementation — ADR-110 Phase 3.
 *
 * Implementation note: ESP-IDF v5.4's iTWT API on C6 is
 *
 *     esp_err_t esp_wifi_sta_itwt_setup(wifi_itwt_setup_config_t *cfg);
 *     esp_err_t esp_wifi_sta_itwt_teardown(uint8_t flow_id);
 *
 * The setup is asynchronous — the actual accept/reject arrives later as
 * a WIFI_EVENT_ITWT_SETUP event. The default handler in this module
 * logs the outcome; the helper itself returns as soon as the request
 * is queued.
 */

#include "sdkconfig.h"
#include "soc/soc_caps.h"

#if (defined(CONFIG_IDF_TARGET_ESP32C6) || defined(CONFIG_IDF_TARGET_ESP32C5)) && SOC_WIFI_HE_SUPPORT

#include "c6_twt.h"
#include "esp_log.h"
#include "esp_wifi.h"
#include "esp_wifi_he.h"      /* esp_wifi_sta_itwt_setup / _teardown */
#include "esp_wifi_he_types.h"
#include "esp_wifi_types.h"
#include "esp_event.h"
#include <string.h>

static const char *TAG = "c6_twt";

static bool      s_active     = false;
static uint8_t   s_flow_id    = 0;
static uint32_t  s_wake_int   = 0;
static uint32_t  s_wake_dura  = 0;

#ifndef CONFIG_C6_TWT_WAKE_INTERVAL_US
#define CONFIG_C6_TWT_WAKE_INTERVAL_US  10000  /* 100 fps default cadence */
#endif

#ifndef CONFIG_C6_TWT_MIN_WAKE_DURA_US
#define CONFIG_C6_TWT_MIN_WAKE_DURA_US  512    /* enough to capture 1 CSI frame */
#endif

/* WIFI_EVENT_ITWT_SETUP handler — logs accept/reject. */
static void on_itwt_event(void *arg, esp_event_base_t base,
                          int32_t event_id, void *event_data)
{
    (void)arg;
    (void)base;
    (void)event_data;
    switch (event_id) {
    case WIFI_EVENT_ITWT_SETUP:
        ESP_LOGI(TAG, "iTWT setup event received from AP (flow_id captured)");
        s_active = true;
        break;
    case WIFI_EVENT_ITWT_TEARDOWN:
        ESP_LOGI(TAG, "iTWT teardown event received");
        s_active = false;
        break;
    case WIFI_EVENT_ITWT_SUSPEND:
        ESP_LOGI(TAG, "iTWT suspended by AP");
        break;
    default:
        break;
    }
}

static bool s_handler_installed = false;

static void install_event_handler_once(void)
{
    if (s_handler_installed) return;
    esp_err_t e = esp_event_handler_instance_register(
        WIFI_EVENT, ESP_EVENT_ANY_ID, on_itwt_event, NULL, NULL);
    if (e == ESP_OK) {
        s_handler_installed = true;
    } else {
        ESP_LOGW(TAG, "Could not install iTWT event handler: %s",
                 esp_err_to_name(e));
    }
}

esp_err_t c6_twt_setup(uint32_t wake_interval_us, uint32_t min_wake_dura_us)
{
    install_event_handler_once();

    s_wake_int  = wake_interval_us;
    s_wake_dura = min_wake_dura_us < 256 ? 256 : min_wake_dura_us;

    wifi_itwt_setup_config_t cfg = {0};
    cfg.setup_cmd       = TWT_REQUEST;
    cfg.flow_id         = s_flow_id;
    cfg.twt_id          = 0;
    cfg.flow_type       = 1;            /* unannounced */
    cfg.min_wake_dura   = (uint8_t)((s_wake_dura + 255) / 256);  /* 256 µs units */
    cfg.wake_duration_unit = 0;          /* 0 = 256 µs, 1 = 1024 µs */
    cfg.wake_invl_expn  = 10;            /* mantissa * 2^10 ≈ 1024 µs base */
    /* mantissa = wake_interval_us / 1024, clamped to uint16 */
    uint32_t mant = wake_interval_us >> 10;
    if (mant == 0) mant = 1;
    if (mant > 0xFFFF) mant = 0xFFFF;
    cfg.wake_invl_mant  = (uint16_t)mant;
    cfg.trigger         = 0;             /* non-triggered: STA wakes on its own */

    esp_err_t ret = esp_wifi_sta_itwt_setup(&cfg);
    if (ret == ESP_OK) {
        ESP_LOGI(TAG, "iTWT setup queued: wake_interval=%lu µs (mant=%u expn=10), "
                      "min_wake_dura=%u (%lu µs)",
                 (unsigned long)wake_interval_us, (unsigned)mant,
                 cfg.min_wake_dura, (unsigned long)s_wake_dura);
        return ESP_OK;
    }
    /* Treat AP-rejection / not-supported / wrong-AP-mode as graceful — log
     * and continue. ESP_ERR_INVALID_ARG is included here because empirically
     * (live capture on ruv.net 2026-05-22) the ESP-IDF v5.4 driver returns
     * INVALID_ARG when the associated AP advertises TWT Responder=0 — the
     * call validates against the AP's HE capability bitmap, not just the
     * struct fields. */
    if (ret == ESP_ERR_NOT_SUPPORTED || ret == ESP_ERR_WIFI_NOT_CONNECT ||
        ret == ESP_ERR_INVALID_STATE  || ret == ESP_ERR_INVALID_ARG) {
        ESP_LOGW(TAG, "iTWT not available (%s) - AP likely not 11ax/iTWT capable,"
                      " falling back to opportunistic CSI",
                 esp_err_to_name(ret));
        return ESP_OK;
    }
    ESP_LOGE(TAG, "iTWT setup failed: %s", esp_err_to_name(ret));
    return ret;
}

esp_err_t c6_twt_setup_default(void)
{
    return c6_twt_setup(CONFIG_C6_TWT_WAKE_INTERVAL_US,
                        CONFIG_C6_TWT_MIN_WAKE_DURA_US);
}

void c6_twt_teardown(void)
{
    if (!s_active) return;
    /* IDF v5.4 signature: esp_err_t esp_wifi_sta_itwt_teardown(int flow_id) */
    esp_err_t ret = esp_wifi_sta_itwt_teardown((int)s_flow_id);
    if (ret == ESP_OK) {
        ESP_LOGI(TAG, "iTWT teardown sent (flow_id=%u)", s_flow_id);
    } else {
        ESP_LOGW(TAG, "iTWT teardown failed: %s", esp_err_to_name(ret));
    }
    s_active = false;
}

bool c6_twt_is_active(void)
{
    return s_active;
}

#endif  /* CONFIG_IDF_TARGET_ESP32C6 && SOC_WIFI_HE_SUPPORT */
