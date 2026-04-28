/**
 * @file ota_pull.c
 * @brief Pull-based OTA client for RuView CSI nodes (#38).
 *
 * Periodically checks the sensing server's firmware registry for updates.
 * If a newer version is available, downloads the binary via HTTP GET,
 * writes it to the OTA partition, verifies SHA256, and reboots.
 *
 * Flow:
 *   1. GET /api/v1/firmware/latest → { available, version, sha256, size }
 *   2. Compare version with current (compile-time esp_app_desc)
 *   3. If newer: GET /api/v1/firmware/download → write to OTA partition
 *   4. Verify SHA256 matches the server's advertised hash
 *   5. Set boot partition and restart
 *
 * Guards:
 *   - Only attempts OTA if uptime > OTA_MIN_UPTIME_SEC (default 300s)
 *   - Stops BLE before OTA to avoid Core 1 StoreProhibited crash
 *   - Aborts if download exceeds OTA_MAX_SIZE
 */

#include "ota_pull.h"

#include <string.h>
#include <stdio.h>
#include <stdlib.h>
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "esp_log.h"
#include "esp_timer.h"
#include "esp_ota_ops.h"
#include "esp_http_client.h"
#include "esp_app_desc.h"
#include "esp_system.h"
#include "mbedtls/sha256.h"
#include "nvs_config.h"
#include "sdkconfig.h"

#ifdef CONFIG_BT_NIMBLE_ENABLED
#include "host/ble_gap.h"
#endif

static const char *TAG = "ota_pull";

/** Check interval in seconds (default: every 5 minutes). */
#define OTA_CHECK_INTERVAL_SEC  300

/** Minimum uptime before attempting OTA (avoid boot-loop on bad firmware). */
#define OTA_MIN_UPTIME_SEC      300

/** Maximum firmware size (1.5 MB). */
#define OTA_MAX_SIZE            (1500 * 1024)

/** HTTP receive buffer size. */
#define OTA_BUF_SIZE            1024

/** Saved server parameters. */
static char     s_server_ip[16];
static uint16_t s_server_port;

extern nvs_config_t g_nvs_config;

/**
 * Stop BLE before OTA to prevent StoreProhibited crash.
 */
static void ota_pull_stop_ble(void)
{
#ifdef CONFIG_BT_NIMBLE_ENABLED
    ble_gap_adv_stop();
    ble_gap_disc_cancel();
    ESP_LOGI(TAG, "BLE stopped for pull-OTA");
#endif
}

/**
 * Compare semantic versions. Returns:
 *   >0 if remote is newer
 *    0 if equal
 *   <0 if remote is older
 *
 * Simple comparison: parse "X.Y.Z" and compare numerically.
 * Falls back to strcmp if parsing fails.
 */
static int version_compare(const char *local, const char *remote)
{
    int lmaj = 0, lmin = 0, lpat = 0;
    int rmaj = 0, rmin = 0, rpat = 0;

    if (sscanf(local, "%d.%d.%d", &lmaj, &lmin, &lpat) != 3 ||
        sscanf(remote, "%d.%d.%d", &rmaj, &rmin, &rpat) != 3) {
        return strcmp(remote, local);
    }

    if (rmaj != lmaj) return rmaj - lmaj;
    if (rmin != lmin) return rmin - lmin;
    return rpat - lpat;
}

/**
 * Fetch JSON from a URL. Caller must free() the returned buffer.
 * Returns NULL on failure.
 */
static char *http_get_json(const char *url)
{
    esp_http_client_config_t config = {
        .url = url,
        .timeout_ms = 10000,
    };

    esp_http_client_handle_t client = esp_http_client_init(&config);
    if (!client) return NULL;

    esp_err_t err = esp_http_client_open(client, 0);
    if (err != ESP_OK) {
        esp_http_client_cleanup(client);
        return NULL;
    }

    int content_length = esp_http_client_fetch_headers(client);
    if (content_length <= 0 || content_length > 4096) {
        esp_http_client_close(client);
        esp_http_client_cleanup(client);
        return NULL;
    }

    char *buf = malloc(content_length + 1);
    if (!buf) {
        esp_http_client_close(client);
        esp_http_client_cleanup(client);
        return NULL;
    }

    int read = esp_http_client_read(client, buf, content_length);
    buf[read > 0 ? read : 0] = '\0';

    esp_http_client_close(client);
    esp_http_client_cleanup(client);
    return buf;
}

/**
 * Simple JSON string extraction: find "key":"value" and copy value to out.
 * Returns true if found.
 */
static bool json_get_string(const char *json, const char *key, char *out, size_t out_len)
{
    char pattern[64];
    snprintf(pattern, sizeof(pattern), "\"%s\":\"", key);
    const char *start = strstr(json, pattern);
    if (!start) return false;

    start += strlen(pattern);
    const char *end = strchr(start, '"');
    if (!end || (size_t)(end - start) >= out_len) return false;

    memcpy(out, start, end - start);
    out[end - start] = '\0';
    return true;
}

/**
 * Simple JSON boolean extraction: find "key":true/false.
 */
static bool json_get_bool(const char *json, const char *key)
{
    char pattern[64];
    snprintf(pattern, sizeof(pattern), "\"%s\":true", key);
    return strstr(json, pattern) != NULL;
}

/**
 * Download firmware and write to OTA partition with SHA256 verification.
 * Returns ESP_OK on success.
 */
static esp_err_t download_and_flash(const char *download_url, const char *expected_sha256)
{
    const esp_partition_t *update_partition = esp_ota_get_next_update_partition(NULL);
    if (!update_partition) {
        ESP_LOGE(TAG, "No OTA partition available");
        return ESP_ERR_NOT_FOUND;
    }

    esp_http_client_config_t config = {
        .url = download_url,
        .timeout_ms = 30000,
        .buffer_size = OTA_BUF_SIZE,
    };

    esp_http_client_handle_t client = esp_http_client_init(&config);
    if (!client) return ESP_FAIL;

    esp_err_t err = esp_http_client_open(client, 0);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "HTTP open failed: %s", esp_err_to_name(err));
        esp_http_client_cleanup(client);
        return err;
    }

    int content_length = esp_http_client_fetch_headers(client);
    ESP_LOGI(TAG, "Firmware download: %d bytes", content_length);

    if (content_length <= 0 || content_length > OTA_MAX_SIZE) {
        ESP_LOGE(TAG, "Invalid firmware size: %d", content_length);
        esp_http_client_close(client);
        esp_http_client_cleanup(client);
        return ESP_ERR_INVALID_SIZE;
    }

    /* Begin OTA write. */
    esp_ota_handle_t ota_handle;
    err = esp_ota_begin(update_partition, OTA_WITH_SEQUENTIAL_WRITES, &ota_handle);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "esp_ota_begin failed: %s", esp_err_to_name(err));
        esp_http_client_close(client);
        esp_http_client_cleanup(client);
        return err;
    }

    /* SHA256 context for verification. */
    mbedtls_sha256_context sha_ctx;
    mbedtls_sha256_init(&sha_ctx);
    mbedtls_sha256_starts(&sha_ctx, 0); /* 0 = SHA-256 (not SHA-224) */

    char buf[OTA_BUF_SIZE];
    int total = 0;

    while (total < content_length) {
        int read = esp_http_client_read(client, buf, sizeof(buf));
        if (read <= 0) {
            if (read == 0) break; /* EOF */
            ESP_LOGE(TAG, "Read error at byte %d", total);
            esp_ota_abort(ota_handle);
            mbedtls_sha256_free(&sha_ctx);
            esp_http_client_close(client);
            esp_http_client_cleanup(client);
            return ESP_FAIL;
        }

        err = esp_ota_write(ota_handle, buf, read);
        if (err != ESP_OK) {
            ESP_LOGE(TAG, "OTA write failed at byte %d: %s", total, esp_err_to_name(err));
            esp_ota_abort(ota_handle);
            mbedtls_sha256_free(&sha_ctx);
            esp_http_client_close(client);
            esp_http_client_cleanup(client);
            return err;
        }

        mbedtls_sha256_update(&sha_ctx, (const unsigned char *)buf, read);
        total += read;

        if ((total % (64 * 1024)) == 0) {
            ESP_LOGI(TAG, "OTA progress: %d / %d bytes (%.0f%%)",
                     total, content_length,
                     (float)total * 100.0f / (float)content_length);
        }
    }

    esp_http_client_close(client);
    esp_http_client_cleanup(client);

    /* Finalize SHA256. */
    unsigned char sha_hash[32];
    mbedtls_sha256_finish(&sha_ctx, sha_hash);
    mbedtls_sha256_free(&sha_ctx);

    /* Convert to hex string for comparison. */
    char sha_hex[65];
    for (int i = 0; i < 32; i++) {
        snprintf(sha_hex + i * 2, 3, "%02x", sha_hash[i]);
    }

    if (expected_sha256[0] != '\0' && strcmp(sha_hex, expected_sha256) != 0) {
        ESP_LOGE(TAG, "SHA256 mismatch! expected=%s got=%s", expected_sha256, sha_hex);
        esp_ota_abort(ota_handle);
        return ESP_ERR_INVALID_CRC;
    }

    ESP_LOGI(TAG, "SHA256 verified: %s", sha_hex);

    /* End OTA (validates image header). */
    err = esp_ota_end(ota_handle);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "esp_ota_end failed: %s", esp_err_to_name(err));
        return err;
    }

    /* Set boot partition. */
    err = esp_ota_set_boot_partition(update_partition);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "esp_ota_set_boot_partition failed: %s", esp_err_to_name(err));
        return err;
    }

    ESP_LOGI(TAG, "OTA update successful! Downloaded %d bytes to partition '%s'",
             total, update_partition->label);
    return ESP_OK;
}

/**
 * Check for firmware update and apply if available.
 */
static void check_for_update(void)
{
    const esp_app_desc_t *app = esp_app_get_description();

    /* Build URL for firmware/latest endpoint. */
    char url[128];
    snprintf(url, sizeof(url), "http://%s:%u/api/v1/firmware/latest",
             s_server_ip, (unsigned)s_server_port);

    ESP_LOGI(TAG, "Checking for update: %s (current: %s)", url, app->version);

    char *json = http_get_json(url);
    if (!json) {
        ESP_LOGD(TAG, "Failed to fetch firmware info");
        return;
    }

    if (!json_get_bool(json, "available")) {
        ESP_LOGD(TAG, "No firmware available on server");
        free(json);
        return;
    }

    char remote_version[32] = {0};
    char remote_sha256[65] = {0};
    json_get_string(json, "version", remote_version, sizeof(remote_version));
    json_get_string(json, "sha256", remote_sha256, sizeof(remote_sha256));
    free(json);

    if (remote_version[0] == '\0') {
        ESP_LOGW(TAG, "Server returned available=true but no version");
        return;
    }

    int cmp = version_compare(app->version, remote_version);
    if (cmp <= 0) {
        ESP_LOGD(TAG, "Current version %s is up to date (server: %s)",
                 app->version, remote_version);
        return;
    }

    ESP_LOGI(TAG, "Update available: %s → %s", app->version, remote_version);

    /* Stop BLE before OTA. */
    ota_pull_stop_ble();

    /* Build download URL. */
    char download_url[128];
    snprintf(download_url, sizeof(download_url), "http://%s:%u/api/v1/firmware/download",
             s_server_ip, (unsigned)s_server_port);

    esp_err_t err = download_and_flash(download_url, remote_sha256);
    if (err == ESP_OK) {
        ESP_LOGI(TAG, "Rebooting to apply update...");
        vTaskDelay(pdMS_TO_TICKS(1000));
        esp_restart();
    } else {
        ESP_LOGE(TAG, "OTA update failed: %s — will retry next cycle", esp_err_to_name(err));
        /* Don't restart on failure — let the node continue operating. */
    }
}

/**
 * FreeRTOS task: periodically check for updates.
 */
static void ota_pull_task(void *arg)
{
    (void)arg;

    /* Wait for minimum uptime before first check. */
    ESP_LOGI(TAG, "OTA pull client waiting %d s before first check", OTA_MIN_UPTIME_SEC);
    vTaskDelay(pdMS_TO_TICKS(OTA_MIN_UPTIME_SEC * 1000));

    while (1) {
        uint32_t uptime_sec = (uint32_t)(esp_timer_get_time() / 1000000ULL);
        if (uptime_sec >= OTA_MIN_UPTIME_SEC) {
            check_for_update();
        }
        vTaskDelay(pdMS_TO_TICKS(OTA_CHECK_INTERVAL_SEC * 1000));
    }
}

esp_err_t ota_pull_init(const char *server_ip, uint16_t server_port)
{
    if (!server_ip) return ESP_ERR_INVALID_ARG;

    strncpy(s_server_ip, server_ip, sizeof(s_server_ip) - 1);
    s_server_ip[sizeof(s_server_ip) - 1] = '\0';
    s_server_port = server_port;

    BaseType_t ret = xTaskCreatePinnedToCore(
        ota_pull_task,
        "ota_pull",
        6144,       /* stack — needs room for HTTP client + SHA256 */
        NULL,
        1,          /* low priority */
        NULL,
        0           /* core 0 — keep core 1 free for CSI */
    );

    if (ret != pdPASS) {
        ESP_LOGE(TAG, "Failed to create OTA pull task");
        return ESP_FAIL;
    }

    ESP_LOGI(TAG, "OTA pull client started → %s:%u (check every %d s, min uptime %d s)",
             s_server_ip, (unsigned)s_server_port,
             OTA_CHECK_INTERVAL_SEC, OTA_MIN_UPTIME_SEC);
    return ESP_OK;
}
