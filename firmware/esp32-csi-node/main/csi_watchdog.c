/**
 * @file csi_watchdog.c
 * @brief CSI capture watchdog (RuView#1941).
 *
 * MEASURED 2026-09-15 (ESP32-C6, 0.8.12): CSI callbacks stopped for 473 s
 * while Wi-Fi, ping, ESP-NOW, the console and RUVIEW_HELLO all stayed
 * healthy. The uplink watchdog in main.c cannot see this: adaptive-controller
 * and mesh packets keep sendto() succeeding, and its limit is 20 minutes.
 *
 * This task watches capture itself. Activity is the OLDER of "last raw CSI
 * callback" and "last CSI frame accepted by sendto", so either the radio or
 * the send path stopping counts. The escalation decision is the pure,
 * host-tested csi_stall_policy; this file only performs the actions.
 *
 * Disabled while power duty-cycling is active, because modem sleep
 * legitimately starves the callback.
 */
#include "csi_watchdog.h"

#include "sdkconfig.h"
#include "esp_log.h"
#include "esp_netif.h"
#include "esp_system.h"
#include "esp_timer.h"
#include "esp_wifi.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"

#include "csi_collector.h"
#include "csi_stall_policy.h"
#include "node_log.h"
#include "nvs_config.h"

static const char *TAG = "csi_wd";

#ifndef CONFIG_CSI_STALL_TIMEOUT_S
#define CONFIG_CSI_STALL_TIMEOUT_S 15
#endif
#ifndef CONFIG_CSI_STALL_INJECT_MODE
#define CONFIG_CSI_STALL_INJECT_MODE 0
#endif
#ifndef CONFIG_CSI_STALL_INJECT_AFTER_S
#define CONFIG_CSI_STALL_INJECT_AFTER_S 60
#endif

#define CSI_WD_PERIOD_MS   1000
#define CSI_WD_REPORT_S    60

static volatile uint32_t s_stalls = 0;
static volatile uint32_t s_recoveries = 0;

extern nvs_config_t g_nvs_config;

#ifdef CONFIG_CSI_STALL_WATCHDOG
static uint32_t now_ms(void)
{
    return (uint32_t)(esp_timer_get_time() / 1000);
}

static bool sta_link_up(void)
{
    wifi_ap_record_t ap;
    if (esp_wifi_sta_get_ap_info(&ap) != ESP_OK) {
        return false;
    }
    esp_netif_t *sta = esp_netif_get_handle_from_ifkey("WIFI_STA_DEF");
    esp_netif_ip_info_t ip;
    return sta != NULL && esp_netif_get_ip_info(sta, &ip) == ESP_OK && ip.ip.addr != 0;
}

/* Older of the two liveness stamps; 0 until both have been seen once. With a
 * source-MAC filter only the radio stamp counts: a filtered transmitter that
 * is switched off must not walk a healthy node to a reboot. */
static uint32_t capture_activity_ms(void)
{
    uint32_t cb = csi_collector_last_callback_ms();
    if (csi_collector_mac_filter_active()) {
        return cb;
    }
    uint32_t tx = csi_collector_last_csi_send_ms();
    if (cb == 0 || tx == 0) {
        return 0;
    }
    return ((int32_t)(cb - tx) < 0) ? cb : tx;
}

static void csi_watchdog_task(void *arg)
{
    (void)arg;
    csi_stall_state_t st;
    csi_stall_init(&st, (uint32_t)CONFIG_CSI_STALL_TIMEOUT_S * 1000u, now_ms());
    uint32_t stall_began_ms = 0;
    uint32_t last_report_ms = now_ms();
#if CONFIG_CSI_STALL_INJECT_MODE != 0
    bool injected = false;
#endif

    for (;;) {
        vTaskDelay(pdMS_TO_TICKS(CSI_WD_PERIOD_MS));
        uint32_t now = now_ms();

#if CONFIG_CSI_STALL_INJECT_MODE != 0
        if (!injected && now >= (uint32_t)CONFIG_CSI_STALL_INJECT_AFTER_S * 1000u) {
            injected = true;
            csi_collector_inject_stall();
        }
#endif

        uint32_t cb = csi_collector_last_callback_ms();
        uint32_t tx = csi_collector_last_csi_send_ms();
        uint32_t activity = capture_activity_ms();
        bool link = sta_link_up();

        if ((uint32_t)(now - last_report_ms) >= CSI_WD_REPORT_S * 1000u) {
            last_report_ms = now;
            ESP_LOGI(TAG, "capture: cb_age=%lums tx_age=%lums link=%d stage=%u "
                          "stalls=%lu recovered=%lu",
                     (unsigned long)(cb ? now - cb : 0),
                     (unsigned long)(tx ? now - tx : 0), (int)link,
                     (unsigned)st.stage, (unsigned long)s_stalls,
                     (unsigned long)s_recoveries);
        }

        switch (csi_stall_step(&st, now, activity, link)) {
        case CSI_STALL_NONE:
            break;

        case CSI_STALL_REARM: {
            s_stalls++;
            stall_began_ms = activity;
            ESP_LOGE(TAG, "CSI capture stalled: cb_age=%lums tx_age=%lums with link up "
                          "-- stage 1: re-arming capture path (stall #%lu)",
                     (unsigned long)(now - cb), (unsigned long)(now - tx),
                     (unsigned long)s_stalls);
            node_log_event(NODE_LOG_EV_CSI_STALL, 1, (int32_t)(now - activity));
            esp_err_t err = csi_collector_rearm();
            if (err != ESP_OK) {
                ESP_LOGW(TAG, "re-arm reported %s", esp_err_to_name(err));
            }
            break;
        }

        case CSI_STALL_REASSOC:
            ESP_LOGE(TAG, "CSI still stalled %lus after re-arm -- stage 2: "
                          "forcing Wi-Fi reassociation",
                     (unsigned long)CONFIG_CSI_STALL_TIMEOUT_S);
            node_log_event(NODE_LOG_EV_CSI_STALL, 2, (int32_t)(now - stall_began_ms));
            /* main.c's disconnect handler owns the reconnect. */
            esp_wifi_disconnect();
            break;

        case CSI_STALL_RESTART:
            ESP_LOGE(TAG, "CSI still stalled after reassociation (%lums total) "
                          "-- stage 3: restarting",
                     (unsigned long)(now - stall_began_ms));
            node_log_event(NODE_LOG_EV_CSI_STALL, 3, (int32_t)(now - stall_began_ms));
            vTaskDelay(pdMS_TO_TICKS(200));   /* let the log line drain */
            esp_restart();
            break;

        case CSI_STALL_RECOVERED:
            s_recoveries++;
            ESP_LOGW(TAG, "CSI capture recovered: outage %lums (stall #%lu, "
                          "recovered without reboot %lu)",
                     (unsigned long)(activity - stall_began_ms),
                     (unsigned long)s_stalls, (unsigned long)s_recoveries);
            node_log_event(NODE_LOG_EV_CSI_STALL, 0, (int32_t)(activity - stall_began_ms));
            break;
        }
    }
}
#endif /* CONFIG_CSI_STALL_WATCHDOG */

esp_err_t csi_watchdog_start(void)
{
#ifndef CONFIG_CSI_STALL_WATCHDOG
    ESP_LOGI(TAG, "CSI capture watchdog disabled (CONFIG_CSI_STALL_WATCHDOG=n)");
    return ESP_OK;
#else
    if (g_nvs_config.power_duty < 100) {
        ESP_LOGI(TAG, "CSI capture watchdog off: power duty %u%% sleeps the modem",
                 (unsigned)g_nvs_config.power_duty);
        return ESP_OK;
    }
    BaseType_t ok = xTaskCreate(csi_watchdog_task, "csi_wd", 3072, NULL, 3, NULL);
    if (ok != pdPASS) {
        ESP_LOGE(TAG, "failed to create CSI watchdog task");
        return ESP_ERR_NO_MEM;
    }
    ESP_LOGI(TAG, "CSI capture watchdog on: timeout=%ds per stage (re-arm, reassoc, restart)%s",
             CONFIG_CSI_STALL_TIMEOUT_S,
             CONFIG_CSI_STALL_INJECT_MODE ? " [TEST STALL INJECTION BUILD]" : "");
    return ESP_OK;
#endif
}

uint32_t csi_watchdog_stall_count(void)
{
    return s_stalls;
}

uint32_t csi_watchdog_recovery_count(void)
{
    return s_recoveries;
}
