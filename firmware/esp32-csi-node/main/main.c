/**
 * @file main.c
 * @brief ESP32-S3 CSI Node — ADR-018 compliant firmware.
 *
 * Initializes NVS, WiFi STA mode, CSI collection, and UDP streaming.
 * CSI frames are serialized in ADR-018 binary format and sent to the
 * aggregator over UDP.
 */

#include <string.h>
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "freertos/event_groups.h"
#include "esp_system.h"
#include "esp_wifi.h"
#include "esp_event.h"
#include "esp_log.h"
#include "nvs_flash.h"
#include "sdkconfig.h"

#include "csi_collector.h"
#include "stream_sender.h"
#include "nvs_config.h"
#include "edge_processing.h"
#include "ota_update.h"
#include "power_mgmt.h"
#include "wasm_runtime.h"
#include "wasm_upload.h"

#include "esp_timer.h"

static const char *TAG = "main";

/* ADR-040: WASM timer handle (calls on_timer at configurable interval). */
static esp_timer_handle_t s_wasm_timer;

/* Runtime configuration (loaded from NVS or Kconfig defaults).
 * Global so other modules (wasm_upload.c) can access pubkey, etc. */
nvs_config_t g_nvs_config;

/* Event group bits */
#define WIFI_CONNECTED_BIT BIT0
#define WIFI_FAIL_BIT      BIT1

static EventGroupHandle_t s_wifi_event_group;
static int s_retry_num = 0;
#define MAX_RETRY 10

static void event_handler(void *arg, esp_event_base_t event_base,
                          int32_t event_id, void *event_data)
{
    if (event_base == WIFI_EVENT && event_id == WIFI_EVENT_STA_START) {
        esp_wifi_connect();
    } else if (event_base == WIFI_EVENT && event_id == WIFI_EVENT_STA_DISCONNECTED) {
        if (s_retry_num < MAX_RETRY) {
            esp_wifi_connect();
            s_retry_num++;
            ESP_LOGI(TAG, "Retrying WiFi connection (%d/%d)", s_retry_num, MAX_RETRY);
        } else {
            xEventGroupSetBits(s_wifi_event_group, WIFI_FAIL_BIT);
        }
    } else if (event_base == IP_EVENT && event_id == IP_EVENT_STA_GOT_IP) {
        ip_event_got_ip_t *event = (ip_event_got_ip_t *)event_data;
        ESP_LOGI(TAG, "Got IP: " IPSTR, IP2STR(&event->ip_info.ip));
        s_retry_num = 0;
        xEventGroupSetBits(s_wifi_event_group, WIFI_CONNECTED_BIT);
    }
}

static void wifi_init_sta(void)
{
    s_wifi_event_group = xEventGroupCreate();

    ESP_ERROR_CHECK(esp_netif_init());
    ESP_ERROR_CHECK(esp_event_loop_create_default());
    esp_netif_create_default_wifi_sta();

    wifi_init_config_t cfg = WIFI_INIT_CONFIG_DEFAULT();
    ESP_ERROR_CHECK(esp_wifi_init(&cfg));

    esp_event_handler_instance_t instance_any_id;
    esp_event_handler_instance_t instance_got_ip;
    ESP_ERROR_CHECK(esp_event_handler_instance_register(
        WIFI_EVENT, ESP_EVENT_ANY_ID, &event_handler, NULL, &instance_any_id));
    ESP_ERROR_CHECK(esp_event_handler_instance_register(
        IP_EVENT, IP_EVENT_STA_GOT_IP, &event_handler, NULL, &instance_got_ip));

    wifi_config_t wifi_config = {
        .sta = {
            .threshold.authmode = WIFI_AUTH_WPA2_PSK,
        },
    };

    /* Copy runtime SSID/password from NVS config */
    strncpy((char *)wifi_config.sta.ssid, g_nvs_config.wifi_ssid, sizeof(wifi_config.sta.ssid) - 1);
    strncpy((char *)wifi_config.sta.password, g_nvs_config.wifi_password, sizeof(wifi_config.sta.password) - 1);

    /* If password is empty, use open auth */
    if (strlen((char *)wifi_config.sta.password) == 0) {
        wifi_config.sta.threshold.authmode = WIFI_AUTH_OPEN;
    }

    ESP_ERROR_CHECK(esp_wifi_set_mode(WIFI_MODE_STA));
    ESP_ERROR_CHECK(esp_wifi_set_config(WIFI_IF_STA, &wifi_config));
    ESP_ERROR_CHECK(esp_wifi_start());

    ESP_LOGI(TAG, "WiFi STA initialized, connecting to SSID: %s", g_nvs_config.wifi_ssid);

    /* Wait for connection */
    EventBits_t bits = xEventGroupWaitBits(s_wifi_event_group,
        WIFI_CONNECTED_BIT | WIFI_FAIL_BIT,
        pdFALSE, pdFALSE, portMAX_DELAY);

    if (bits & WIFI_CONNECTED_BIT) {
        ESP_LOGI(TAG, "Connected to WiFi");
    } else if (bits & WIFI_FAIL_BIT) {
        ESP_LOGE(TAG, "Failed to connect to WiFi after %d retries", MAX_RETRY);
    }
}

void app_main(void)
{
    /* Initialize NVS */
    esp_err_t ret = nvs_flash_init();
    if (ret == ESP_ERR_NVS_NO_FREE_PAGES || ret == ESP_ERR_NVS_NEW_VERSION_FOUND) {
        ESP_ERROR_CHECK(nvs_flash_erase());
        ret = nvs_flash_init();
    }
    ESP_ERROR_CHECK(ret);

    /* Load runtime config (NVS overrides Kconfig defaults) */
    nvs_config_load(&g_nvs_config);

    ESP_LOGI(TAG, "ESP32-S3 CSI Node (ADR-018) — Node ID: %d", g_nvs_config.node_id);

    /* Initialize WiFi STA */
    wifi_init_sta();

    /* Initialize UDP sender with runtime target */
    if (stream_sender_init_with(g_nvs_config.target_ip, g_nvs_config.target_port) != 0) {
        ESP_LOGE(TAG, "Failed to initialize UDP sender");
        return;
    }

    /* Initialize CSI collection */
    csi_collector_init();

    /* ADR-039: Initialize edge processing pipeline. */
    edge_config_t edge_cfg = {
        .tier              = g_nvs_config.edge_tier,
        .presence_thresh   = g_nvs_config.presence_thresh,
        .fall_thresh       = g_nvs_config.fall_thresh,
        .vital_window      = g_nvs_config.vital_window,
        .vital_interval_ms = g_nvs_config.vital_interval_ms,
        .top_k_count       = g_nvs_config.top_k_count,
        .power_duty        = g_nvs_config.power_duty,
    };
    esp_err_t edge_ret = edge_processing_init(&edge_cfg);
    if (edge_ret != ESP_OK) {
        ESP_LOGW(TAG, "Edge processing init failed: %s (continuing without edge DSP)",
                 esp_err_to_name(edge_ret));
    }

    /* Initialize OTA update HTTP server. */
    httpd_handle_t ota_server = NULL;
    esp_err_t ota_ret = ota_update_init_ex(&ota_server);
    if (ota_ret != ESP_OK) {
        ESP_LOGW(TAG, "OTA server init failed: %s", esp_err_to_name(ota_ret));
    }

    /* ADR-040: Initialize WASM programmable sensing runtime. */
    esp_err_t wasm_ret = wasm_runtime_init();
    if (wasm_ret != ESP_OK) {
        ESP_LOGW(TAG, "WASM runtime init failed: %s", esp_err_to_name(wasm_ret));
    } else {
        /* Register WASM upload endpoints on the OTA HTTP server. */
        if (ota_server != NULL) {
            wasm_upload_register(ota_server);
        }

        /* Start periodic timer for wasm_runtime_on_timer(). */
        esp_timer_create_args_t timer_args = {
            .callback = (void (*)(void *))wasm_runtime_on_timer,
            .arg = NULL,
            .dispatch_method = ESP_TIMER_TASK,
            .name = "wasm_timer",
        };
        esp_err_t timer_ret = esp_timer_create(&timer_args, &s_wasm_timer);
        if (timer_ret == ESP_OK) {
#ifdef CONFIG_WASM_TIMER_INTERVAL_MS
            uint64_t interval_us = (uint64_t)CONFIG_WASM_TIMER_INTERVAL_MS * 1000ULL;
#else
            uint64_t interval_us = 1000000ULL;  /* Default: 1 second. */
#endif
            esp_timer_start_periodic(s_wasm_timer, interval_us);
            ESP_LOGI(TAG, "WASM on_timer() periodic: %llu ms",
                     (unsigned long long)(interval_us / 1000));
        } else {
            ESP_LOGW(TAG, "WASM timer create failed: %s", esp_err_to_name(timer_ret));
        }
    }

    /* Initialize power management. */
    power_mgmt_init(g_nvs_config.power_duty);

    ESP_LOGI(TAG, "CSI streaming active → %s:%d (edge_tier=%u, OTA=%s, WASM=%s)",
             g_nvs_config.target_ip, g_nvs_config.target_port,
             g_nvs_config.edge_tier,
             (ota_ret == ESP_OK) ? "ready" : "off",
             (wasm_ret == ESP_OK) ? "ready" : "off");

    /* Main loop — keep alive */
    while (1) {
        vTaskDelay(pdMS_TO_TICKS(10000));
    }
}
