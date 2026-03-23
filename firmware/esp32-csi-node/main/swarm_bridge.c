/**
 * @file swarm_bridge.c
 * @brief ADR-066: ESP32 Swarm Bridge — Cognitum Seed coordinator client.
 *
 * Runs a FreeRTOS task on Core 0 that periodically POSTs registration,
 * heartbeat, and happiness vectors to a Cognitum Seed ingest endpoint.
 */

#include "swarm_bridge.h"

#include <string.h>
#include <stdio.h>
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "freertos/semphr.h"
#include "esp_log.h"
#include "esp_timer.h"
#include "esp_system.h"
#include "esp_app_desc.h"
#include "esp_netif.h"
#include "esp_http_client.h"

static const char *TAG = "swarm";

/* ---- Task parameters ---- */
#define SWARM_TASK_STACK   3072   /**< 3 KB stack — HTTP client uses ~2.5 KB. */
#define SWARM_TASK_PRIO    3
#define SWARM_TASK_CORE    0
#define SWARM_HTTP_TIMEOUT 3000  /**< HTTP timeout in ms (Seed responds <100ms on LAN). */

/* ---- Ingest endpoint path ---- */
#define SWARM_INGEST_PATH  "/api/v1/store/ingest"

/* ---- JSON buffer size (Seed tuple format: max ~120 bytes per vector) ---- */
#define SWARM_JSON_BUF     256

/* ---- Module state ---- */
static swarm_config_t  s_cfg;
static uint8_t         s_node_id;
static SemaphoreHandle_t s_mutex;
static TaskHandle_t    s_task_handle;

/* ---- Protected shared data ---- */
static edge_vitals_pkt_t s_vitals;
static float             s_happiness[SWARM_VECTOR_DIM];
static bool              s_vitals_valid;

/* ---- Counters ---- */
static uint32_t s_cnt_regs;
static uint32_t s_cnt_heartbeats;
static uint32_t s_cnt_ingests;
static uint32_t s_cnt_errors;

/* ---- Forward declarations ---- */
static void swarm_task(void *arg);
static esp_err_t swarm_post_json(esp_http_client_handle_t client,
                                 const char *json, int json_len);
static void swarm_get_ip_str(char *buf, size_t buf_len);

/* ------------------------------------------------------------------ */

esp_err_t swarm_bridge_init(const swarm_config_t *cfg, uint8_t node_id)
{
    if (cfg == NULL || cfg->seed_url[0] == '\0') {
        ESP_LOGW(TAG, "seed_url is empty — swarm bridge disabled");
        return ESP_ERR_INVALID_ARG;
    }

    memcpy(&s_cfg, cfg, sizeof(s_cfg));
    s_node_id = node_id;

    /* Apply defaults for zero-valued intervals. */
    if (s_cfg.heartbeat_sec == 0) {
        s_cfg.heartbeat_sec = 30;
    }
    if (s_cfg.ingest_sec == 0) {
        s_cfg.ingest_sec = 5;
    }

    s_mutex = xSemaphoreCreateMutex();
    if (s_mutex == NULL) {
        ESP_LOGE(TAG, "failed to create mutex");
        return ESP_ERR_NO_MEM;
    }

    s_vitals_valid = false;
    memset(s_happiness, 0, sizeof(s_happiness));
    s_cnt_regs = 0;
    s_cnt_heartbeats = 0;
    s_cnt_ingests = 0;
    s_cnt_errors = 0;

    BaseType_t ret = xTaskCreatePinnedToCore(
        swarm_task, "swarm", SWARM_TASK_STACK, NULL,
        SWARM_TASK_PRIO, &s_task_handle, SWARM_TASK_CORE);

    if (ret != pdPASS) {
        ESP_LOGE(TAG, "failed to create swarm task");
        vSemaphoreDelete(s_mutex);
        s_mutex = NULL;
        return ESP_FAIL;
    }

    ESP_LOGI(TAG, "bridge init OK — seed=%s zone=%s hb=%us ingest=%us",
             s_cfg.seed_url, s_cfg.zone_name,
             s_cfg.heartbeat_sec, s_cfg.ingest_sec);
    return ESP_OK;
}

void swarm_bridge_update_vitals(const edge_vitals_pkt_t *vitals)
{
    if (vitals == NULL || s_mutex == NULL) {
        return;
    }
    xSemaphoreTake(s_mutex, portMAX_DELAY);
    memcpy(&s_vitals, vitals, sizeof(s_vitals));
    s_vitals_valid = true;
    xSemaphoreGive(s_mutex);
}

void swarm_bridge_update_happiness(const float *vector, uint8_t dim)
{
    if (vector == NULL || s_mutex == NULL) {
        return;
    }
    uint8_t n = (dim < SWARM_VECTOR_DIM) ? dim : SWARM_VECTOR_DIM;

    xSemaphoreTake(s_mutex, portMAX_DELAY);
    memcpy(s_happiness, vector, n * sizeof(float));
    /* Zero-fill remaining dimensions. */
    for (uint8_t i = n; i < SWARM_VECTOR_DIM; i++) {
        s_happiness[i] = 0.0f;
    }
    xSemaphoreGive(s_mutex);
}

void swarm_bridge_get_stats(uint32_t *regs, uint32_t *heartbeats,
                            uint32_t *ingests, uint32_t *errors)
{
    if (regs)       *regs       = s_cnt_regs;
    if (heartbeats) *heartbeats = s_cnt_heartbeats;
    if (ingests)    *ingests    = s_cnt_ingests;
    if (errors)     *errors     = s_cnt_errors;
}

/* ---- HTTP POST helper ---- */

static esp_err_t swarm_post_json(esp_http_client_handle_t client,
                                 const char *json, int json_len)
{
    esp_http_client_set_post_field(client, json, json_len);

    esp_err_t err = esp_http_client_perform(client);
    if (err != ESP_OK) {
        /* Connection may have been closed by Seed between requests.
         * Close our end and let the next perform() reconnect. */
        esp_http_client_close(client);
        /* Retry once. */
        err = esp_http_client_perform(client);
        if (err != ESP_OK) {
            ESP_LOGW(TAG, "HTTP POST failed: %s", esp_err_to_name(err));
            s_cnt_errors++;
            esp_http_client_close(client);
            return err;
        }
    }

    int status = esp_http_client_get_status_code(client);
    /* Close connection after each request to avoid stale keep-alive. */
    esp_http_client_close(client);

    if (status < 200 || status >= 300) {
        ESP_LOGW(TAG, "HTTP POST status %d", status);
        s_cnt_errors++;
        return ESP_FAIL;
    }

    return ESP_OK;
}

/* ---- Get local IP address as string ---- */

static void swarm_get_ip_str(char *buf, size_t buf_len)
{
    esp_netif_t *netif = esp_netif_get_handle_from_ifkey("WIFI_STA_DEF");
    if (netif == NULL) {
        snprintf(buf, buf_len, "0.0.0.0");
        return;
    }

    esp_netif_ip_info_t ip_info;
    if (esp_netif_get_ip_info(netif, &ip_info) != ESP_OK) {
        snprintf(buf, buf_len, "0.0.0.0");
        return;
    }

    snprintf(buf, buf_len, IPSTR, IP2STR(&ip_info.ip));
}

/* ---- Swarm bridge task ---- */

static void swarm_task(void *arg)
{
    (void)arg;

    /* Build the full ingest URL once. */
    char url[128];
    snprintf(url, sizeof(url), "%s%s", s_cfg.seed_url, SWARM_INGEST_PATH);

    /* Create a reusable HTTP client. */
    esp_http_client_config_t http_cfg = {
        .url            = url,
        .method         = HTTP_METHOD_POST,
        .timeout_ms     = SWARM_HTTP_TIMEOUT,
    };
    esp_http_client_handle_t client = esp_http_client_init(&http_cfg);
    if (client == NULL) {
        ESP_LOGE(TAG, "failed to create HTTP client — task exiting");
        vTaskDelete(NULL);
        return;
    }

    esp_http_client_set_header(client, "Content-Type", "application/json");

    /* ADR-066: Set Bearer token for Seed WiFi auth (from pairing). */
    if (s_cfg.seed_token[0] != '\0') {
        char auth_hdr[80];
        snprintf(auth_hdr, sizeof(auth_hdr), "Bearer %s", s_cfg.seed_token);
        esp_http_client_set_header(client, "Authorization", auth_hdr);
        ESP_LOGI(TAG, "Bearer token configured for Seed auth");
    }

    /* Get firmware version string. */
    const esp_app_desc_t *app = esp_app_get_description();
    const char *fw_ver = app ? app->version : "unknown";

    /* Get local IP. */
    char ip_str[16];
    swarm_get_ip_str(ip_str, sizeof(ip_str));

    /* ---- Registration POST ---- */
    /* Seed ingest format: {"vectors":[[u64_id, [f32; dim]]]} */
    {
        /* ID scheme: node_id * 1000000 + type_code (0=reg, 1=hb, 2=happiness) */
        uint32_t reg_id = (uint32_t)s_node_id * 1000000U;
        char json[SWARM_JSON_BUF];
        int len = snprintf(json, sizeof(json),
            "{\"vectors\":[[%lu,[0,0,0,0,0,0,0,0]]]}",
            (unsigned long)reg_id);

        if (swarm_post_json(client, json, len) == ESP_OK) {
            s_cnt_regs++;
            ESP_LOGI(TAG, "registered node %u with seed (id=%lu)", s_node_id, (unsigned long)reg_id);
        } else {
            ESP_LOGW(TAG, "registration failed — will retry on next heartbeat");
        }
    }

    /* ---- Main loop ---- */
    TickType_t last_heartbeat = xTaskGetTickCount();
    TickType_t last_ingest    = xTaskGetTickCount();
    const TickType_t poll_interval = pdMS_TO_TICKS(1000);  /* Wake every 1 s. */

    for (;;) {
        vTaskDelay(poll_interval);

        TickType_t now = xTaskGetTickCount();

        /* Snapshot shared data under mutex. */
        float            hv[SWARM_VECTOR_DIM];
        edge_vitals_pkt_t vit;
        bool              vit_valid;

        xSemaphoreTake(s_mutex, portMAX_DELAY);
        memcpy(hv, s_happiness, sizeof(hv));
        memcpy(&vit, &s_vitals, sizeof(vit));
        vit_valid = s_vitals_valid;
        xSemaphoreGive(s_mutex);

        uint32_t uptime_s = (uint32_t)(esp_timer_get_time() / 1000000ULL);
        uint32_t free_heap = esp_get_free_heap_size();
        uint32_t ts = (uint32_t)(esp_timer_get_time() / 1000ULL);

        /* ---- Heartbeat ---- */
        if ((now - last_heartbeat) >= pdMS_TO_TICKS(s_cfg.heartbeat_sec * 1000U)) {
            last_heartbeat = now;

            bool presence = vit_valid && (vit.flags & 0x01);

            /* Heartbeat ID: node_id * 1000000 + 100000 + ts_sec */
            uint32_t hb_id = (uint32_t)s_node_id * 1000000U + 100000U + (uptime_s % 100000U);
            char json[SWARM_JSON_BUF];
            int len = snprintf(json, sizeof(json),
                "{\"vectors\":[[%lu,[%.4f,%.4f,%.4f,%.4f,%.4f,%.4f,%.4f,%.4f]]]}",
                (unsigned long)hb_id,
                hv[0], hv[1], hv[2], hv[3], hv[4], hv[5], hv[6], hv[7]);

            if (swarm_post_json(client, json, len) == ESP_OK) {
                s_cnt_heartbeats++;
            }
        }

        /* ---- Happiness ingest (only when presence detected) ---- */
        if ((now - last_ingest) >= pdMS_TO_TICKS(s_cfg.ingest_sec * 1000U)) {
            last_ingest = now;

            bool presence = vit_valid && (vit.flags & 0x01);
            if (presence) {
                /* Happiness ID: node_id * 1000000 + 200000 + ts_sec */
                uint32_t h_id = (uint32_t)s_node_id * 1000000U + 200000U + (ts / 1000U % 100000U);
                char json[SWARM_JSON_BUF];
                int len = snprintf(json, sizeof(json),
                    "{\"vectors\":[[%lu,[%.4f,%.4f,%.4f,%.4f,%.4f,%.4f,%.4f,%.4f]]]}",
                    (unsigned long)h_id,
                    hv[0], hv[1], hv[2], hv[3], hv[4], hv[5], hv[6], hv[7]);

                if (swarm_post_json(client, json, len) == ESP_OK) {
                    s_cnt_ingests++;
                }
            }
        }
    }

    /* Unreachable, but clean up for completeness. */
    esp_http_client_cleanup(client);
    vTaskDelete(NULL);
}
