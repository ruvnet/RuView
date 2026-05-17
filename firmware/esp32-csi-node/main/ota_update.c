/**
 * @file ota_update.c
 * @brief HTTP OTA firmware update for ESP32-S3 CSI Node.
 *
 * Uses ESP-IDF's native OTA API with rollback support.
 * The HTTP server runs on port 8032 and accepts:
 *   POST /ota — firmware binary payload (application/octet-stream)
 *   GET /ota/status — current firmware version and partition info
 */

#include "ota_update.h"

#include <string.h>
#include "esp_log.h"
#include "esp_ota_ops.h"
#include "esp_http_server.h"
#include "esp_app_desc.h"
#include "nvs_flash.h"
#include "nvs.h"
#include "nvs_config.h"  /* NVS_CFG_IP_MAX */

static const char *TAG = "ota_update";

/** OTA HTTP server port. */
#define OTA_PORT 8032

/** Maximum firmware size (900 KB — matches CI binary size gate). */
#define OTA_MAX_SIZE (900 * 1024)

/** NVS namespace and key for the OTA pre-shared key. */
#define OTA_NVS_NAMESPACE "security"
#define OTA_NVS_KEY       "ota_psk"

/** Maximum PSK length (hex-encoded SHA-256). */
#define OTA_PSK_MAX_LEN   65

/** Cached PSK loaded from NVS at init time. Empty = auth disabled. */
static char s_ota_psk[OTA_PSK_MAX_LEN] = {0};

/**
 * ADR-050: Verify the Authorization header contains the correct PSK.
 * Returns true if auth is disabled (no PSK provisioned) or if the
 * Bearer token matches the stored PSK.
 */
static bool ota_check_auth(httpd_req_t *req)
{
    if (s_ota_psk[0] == '\0') {
        /* No PSK provisioned — auth disabled (permissive for dev). */
        return true;
    }

    char auth_header[128] = {0};
    if (httpd_req_get_hdr_value_str(req, "Authorization", auth_header,
                                     sizeof(auth_header)) != ESP_OK) {
        return false;
    }

    /* Expect "Bearer <psk>" */
    const char *prefix = "Bearer ";
    if (strncmp(auth_header, prefix, strlen(prefix)) != 0) {
        return false;
    }

    const char *token = auth_header + strlen(prefix);
    /* Constant-time comparison to prevent timing attacks. */
    size_t psk_len = strlen(s_ota_psk);
    size_t tok_len = strlen(token);
    if (psk_len != tok_len) return false;
    volatile uint8_t result = 0;
    for (size_t i = 0; i < psk_len; i++) {
        result |= (uint8_t)(s_ota_psk[i] ^ token[i]);
    }
    return result == 0;
}

/**
 * GET /ota/status — return firmware version and partition info.
 */
static esp_err_t ota_status_handler(httpd_req_t *req)
{
    const esp_app_desc_t *app = esp_app_get_description();
    const esp_partition_t *running = esp_ota_get_running_partition();
    const esp_partition_t *update = esp_ota_get_next_update_partition(NULL);

    char response[512];
    int len = snprintf(response, sizeof(response),
        "{\"version\":\"%s\",\"date\":\"%s\",\"time\":\"%s\","
        "\"running_partition\":\"%s\",\"next_partition\":\"%s\","
        "\"max_size\":%d}",
        app->version, app->date, app->time,
        running ? running->label : "unknown",
        update ? update->label : "none",
        OTA_MAX_SIZE);

    httpd_resp_set_type(req, "application/json");
    httpd_resp_send(req, response, len);
    return ESP_OK;
}

/**
 * POST /ota/recalibrate — clear cached gain-lock NVS keys and reboot.
 *
 * ADR-109: lets the operator force a full gain-lock re-calibration from
 * the server without a USB connection. Erases csi_cfg/gl_agc, gl_fft, and
 * gl_ap_mac (ADR-111), then calls esp_restart(). Next boot finds no NVS
 * cache and runs the 300-packet calibration as if it were a fresh device.
 */
static esp_err_t ota_recalibrate_handler(httpd_req_t *req)
{
    if (!ota_check_auth(req)) {
        ESP_LOGW(TAG, "/ota/recalibrate rejected: authentication failed");
        httpd_resp_send_err(req, HTTPD_403_FORBIDDEN,
                            "Authentication required. Use: Authorization: Bearer <psk>");
        return ESP_FAIL;
    }

    nvs_handle_t h;
    esp_err_t err = nvs_open("csi_cfg", NVS_READWRITE, &h);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "/ota/recalibrate: nvs_open(csi_cfg) failed: %s",
                 esp_err_to_name(err));
        httpd_resp_send_err(req, HTTPD_500_INTERNAL_SERVER_ERROR,
                            "NVS open failed");
        return ESP_FAIL;
    }

    /* Erase all three keys defensively — ignore individual ESP_ERR_NVS_NOT_FOUND
     * (key already absent on a never-calibrated device). */
    (void)nvs_erase_key(h, "gl_agc");
    (void)nvs_erase_key(h, "gl_fft");
    (void)nvs_erase_key(h, "gl_ap_mac");
    err = nvs_commit(h);
    nvs_close(h);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "/ota/recalibrate: nvs_commit failed: %s",
                 esp_err_to_name(err));
        httpd_resp_send_err(req, HTTPD_500_INTERNAL_SERVER_ERROR,
                            "NVS commit failed");
        return ESP_FAIL;
    }

    ESP_LOGI(TAG, "/ota/recalibrate: gain-lock NVS cleared; rebooting in 1s");

    const char *resp =
        "{\"status\":\"ok\",\"message\":\"gain-lock NVS cleared; rebooting\"}";
    httpd_resp_set_type(req, "application/json");
    httpd_resp_send(req, resp, strlen(resp));

    vTaskDelay(pdMS_TO_TICKS(1000));
    esp_restart();
    return ESP_OK; /* unreachable */
}

/**
 * POST /ota/set-target — write csi_cfg/target_ip + target_port to NVS, reboot.
 *
 * ADR-115: lets the operator point sensors at a new aggregator (Mac IP
 * change, network move) without USB. Body is plain text "IP:PORT" with
 * trailing newline tolerated, e.g. "192.168.0.103:5005". IP validated
 * by inet_pton-like check (4 dot-separated octets 0–255); port 1–65535.
 *
 * Persists into the same `csi_cfg` namespace that `nvs_config.c` reads
 * at boot — next reboot picks up the new target.
 */
static bool parse_ip_port(const char *s, char *ip_out, size_t ip_cap, uint16_t *port_out)
{
    /* Tolerate trailing whitespace/CR/LF. */
    size_t n = strlen(s);
    while (n > 0 && (s[n - 1] == '\n' || s[n - 1] == '\r' || s[n - 1] == ' ' || s[n - 1] == '\t')) {
        n--;
    }
    const char *colon = NULL;
    for (size_t i = 0; i < n; i++) {
        if (s[i] == ':') { colon = &s[i]; break; }
    }
    if (!colon) return false;
    size_t ip_len = (size_t)(colon - s);
    if (ip_len == 0 || ip_len >= ip_cap) return false;
    memcpy(ip_out, s, ip_len);
    ip_out[ip_len] = '\0';
    /* Validate 4 octets 0–255. */
    int oct_count = 0, val = -1;
    for (size_t i = 0; i <= ip_len; i++) {
        char c = ip_out[i];
        if (c == '.' || c == '\0') {
            if (val < 0 || val > 255) return false;
            oct_count++;
            val = -1;
        } else if (c >= '0' && c <= '9') {
            val = (val < 0 ? 0 : val) * 10 + (c - '0');
        } else {
            return false;
        }
    }
    if (oct_count != 4) return false;
    /* Parse port. */
    long port = 0;
    const char *p = colon + 1;
    size_t plen = n - ip_len - 1;
    if (plen == 0 || plen > 5) return false;
    for (size_t i = 0; i < plen; i++) {
        if (p[i] < '0' || p[i] > '9') return false;
        port = port * 10 + (p[i] - '0');
    }
    if (port < 1 || port > 65535) return false;
    *port_out = (uint16_t)port;
    return true;
}

static esp_err_t ota_set_target_handler(httpd_req_t *req)
{
    if (!ota_check_auth(req)) {
        ESP_LOGW(TAG, "/ota/set-target rejected: authentication failed");
        httpd_resp_send_err(req, HTTPD_403_FORBIDDEN,
                            "Authentication required. Use: Authorization: Bearer <psk>");
        return ESP_FAIL;
    }

    /* Body is short: "IPv4:port" + optional CRLF. 32 bytes is plenty. */
    char body[40] = {0};
    int total = 0;
    while (total < (int)sizeof(body) - 1) {
        int r = httpd_req_recv(req, body + total, sizeof(body) - 1 - total);
        if (r <= 0) {
            if (r == HTTPD_SOCK_ERR_TIMEOUT) continue;
            break;
        }
        total += r;
    }
    body[total < 0 ? 0 : total] = '\0';

    char ip[NVS_CFG_IP_MAX] = {0};
    uint16_t port = 0;
    if (!parse_ip_port(body, ip, sizeof(ip), &port)) {
        ESP_LOGW(TAG, "/ota/set-target rejected: invalid body '%s'", body);
        httpd_resp_send_err(req, HTTPD_400_BAD_REQUEST,
                            "Body must be 'IPv4:PORT', e.g. '192.168.0.103:5005'");
        return ESP_FAIL;
    }

    nvs_handle_t h;
    esp_err_t err = nvs_open("csi_cfg", NVS_READWRITE, &h);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "/ota/set-target: nvs_open(csi_cfg) failed: %s",
                 esp_err_to_name(err));
        httpd_resp_send_err(req, HTTPD_500_INTERNAL_SERVER_ERROR, "NVS open failed");
        return ESP_FAIL;
    }
    err = nvs_set_str(h, "target_ip", ip);
    if (err == ESP_OK) err = nvs_set_u16(h, "target_port", port);
    if (err == ESP_OK) err = nvs_commit(h);
    nvs_close(h);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "/ota/set-target: NVS write failed: %s", esp_err_to_name(err));
        httpd_resp_send_err(req, HTTPD_500_INTERNAL_SERVER_ERROR, "NVS write failed");
        return ESP_FAIL;
    }

    ESP_LOGI(TAG, "/ota/set-target: csi_cfg/target_ip=%s target_port=%u; rebooting in 1s",
             ip, (unsigned)port);

    char resp[120];
    int rlen = snprintf(resp, sizeof(resp),
        "{\"status\":\"ok\",\"target_ip\":\"%s\",\"target_port\":%u,\"message\":\"rebooting\"}",
        ip, (unsigned)port);
    httpd_resp_set_type(req, "application/json");
    httpd_resp_send(req, resp, rlen);

    vTaskDelay(pdMS_TO_TICKS(1000));
    esp_restart();
    return ESP_OK; /* unreachable */
}

/**
 * POST /ota — receive and flash firmware binary.
 */
static esp_err_t ota_upload_handler(httpd_req_t *req)
{
    /* ADR-050: Authenticate before accepting firmware upload. */
    if (!ota_check_auth(req)) {
        ESP_LOGW(TAG, "OTA upload rejected: authentication failed");
        httpd_resp_send_err(req, HTTPD_403_FORBIDDEN,
                            "Authentication required. Use: Authorization: Bearer <psk>");
        return ESP_FAIL;
    }

    ESP_LOGI(TAG, "OTA update started, content_length=%d", req->content_len);

    if (req->content_len <= 0 || req->content_len > OTA_MAX_SIZE) {
        httpd_resp_send_err(req, HTTPD_400_BAD_REQUEST,
                            "Invalid firmware size (must be 1B - 900KB)");
        return ESP_FAIL;
    }

    const esp_partition_t *update_partition = esp_ota_get_next_update_partition(NULL);
    if (update_partition == NULL) {
        httpd_resp_send_err(req, HTTPD_500_INTERNAL_SERVER_ERROR,
                            "No OTA partition available");
        return ESP_FAIL;
    }

    esp_ota_handle_t ota_handle;
    /* Issue #556: use OTA_SIZE_UNKNOWN (full partition erase) instead of
     * OTA_WITH_SEQUENTIAL_WRITES. When the new image is smaller than the
     * one previously written to the target slot, sequential writes leave
     * the tail of the old code in place. The image header SHA covers
     * only the declared image span, but residual code at stale offsets
     * can still be reached via IRAM jump tables / .literal pools on some
     * v5.2 ABIs and crash the new app on first boot, which then looks
     * like "OTA didn't take". Full erase up-front avoids this entirely
     * at the cost of one extra ~1.5 s erase before write starts. */
    esp_err_t err = esp_ota_begin(update_partition, OTA_SIZE_UNKNOWN, &ota_handle);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "esp_ota_begin failed: %s", esp_err_to_name(err));
        httpd_resp_send_err(req, HTTPD_500_INTERNAL_SERVER_ERROR,
                            "OTA begin failed");
        return ESP_FAIL;
    }

    /* Read firmware in chunks. */
    char buf[1024];
    int received = 0;
    int total = 0;

    while (total < req->content_len) {
        received = httpd_req_recv(req, buf, sizeof(buf));
        if (received <= 0) {
            if (received == HTTPD_SOCK_ERR_TIMEOUT) {
                continue;  /* Retry on timeout. */
            }
            ESP_LOGE(TAG, "OTA receive error at byte %d", total);
            esp_ota_abort(ota_handle);
            httpd_resp_send_err(req, HTTPD_500_INTERNAL_SERVER_ERROR,
                                "Receive error");
            return ESP_FAIL;
        }

        err = esp_ota_write(ota_handle, buf, received);
        if (err != ESP_OK) {
            ESP_LOGE(TAG, "esp_ota_write failed at byte %d: %s",
                     total, esp_err_to_name(err));
            esp_ota_abort(ota_handle);
            httpd_resp_send_err(req, HTTPD_500_INTERNAL_SERVER_ERROR,
                                "OTA write failed");
            return ESP_FAIL;
        }

        total += received;
        if ((total % (64 * 1024)) == 0) {
            ESP_LOGI(TAG, "OTA progress: %d / %d bytes (%.0f%%)",
                     total, req->content_len,
                     (float)total * 100.0f / (float)req->content_len);
        }
    }

    err = esp_ota_end(ota_handle);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "esp_ota_end failed: %s", esp_err_to_name(err));
        httpd_resp_send_err(req, HTTPD_500_INTERNAL_SERVER_ERROR,
                            "OTA validation failed");
        return ESP_FAIL;
    }

    err = esp_ota_set_boot_partition(update_partition);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "esp_ota_set_boot_partition failed: %s", esp_err_to_name(err));
        httpd_resp_send_err(req, HTTPD_500_INTERNAL_SERVER_ERROR,
                            "Set boot partition failed");
        return ESP_FAIL;
    }

    ESP_LOGI(TAG, "OTA update successful! Rebooting to partition '%s'...",
             update_partition->label);

    const char *resp = "{\"status\":\"ok\",\"message\":\"OTA update successful. Rebooting...\"}";
    httpd_resp_set_type(req, "application/json");
    httpd_resp_send(req, resp, strlen(resp));

    /* Delay briefly to let the response flush, then reboot. */
    vTaskDelay(pdMS_TO_TICKS(1000));
    esp_restart();

    return ESP_OK;  /* Never reached. */
}

/** Internal: start the HTTP server and register OTA endpoints. */
static esp_err_t ota_start_server(httpd_handle_t *out_handle)
{
    httpd_config_t config = HTTPD_DEFAULT_CONFIG();
    config.server_port = OTA_PORT;
    config.max_uri_handlers = 12;  /* Extra slots for WASM endpoints (ADR-040). */
    /* Increase receive timeout for large uploads. */
    config.recv_wait_timeout = 30;
    /* Issue #556: httpd default stack is 4096 B, which overflows during
     * esp_ota_end()'s image-verify (SHA256 streaming + mmap segment walk
     * eats ~3 KB on top of the request handler frame). Empirically observed
     * "***ERROR*** A stack overflow in task httpd has been detected"
     * immediately after esp_image: segment dumps when OTA reaches verify.
     * 8 KB gives a clean margin without hurting the typical idle case. */
    config.stack_size = 8192;

    httpd_handle_t server = NULL;
    esp_err_t err = httpd_start(&server, &config);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "Failed to start OTA HTTP server on port %d: %s",
                 OTA_PORT, esp_err_to_name(err));
        if (out_handle) *out_handle = NULL;
        return err;
    }

    httpd_uri_t status_uri = {
        .uri      = "/ota/status",
        .method   = HTTP_GET,
        .handler  = ota_status_handler,
        .user_ctx = NULL,
    };
    httpd_register_uri_handler(server, &status_uri);

    httpd_uri_t upload_uri = {
        .uri      = "/ota",
        .method   = HTTP_POST,
        .handler  = ota_upload_handler,
        .user_ctx = NULL,
    };
    httpd_register_uri_handler(server, &upload_uri);

    /* ADR-109: REST trigger for full gain-lock re-calibration. */
    httpd_uri_t recalibrate_uri = {
        .uri      = "/ota/recalibrate",
        .method   = HTTP_POST,
        .handler  = ota_recalibrate_handler,
        .user_ctx = NULL,
    };
    httpd_register_uri_handler(server, &recalibrate_uri);

    /* ADR-115: REST endpoint to change CSI aggregator target without USB. */
    httpd_uri_t set_target_uri = {
        .uri      = "/ota/set-target",
        .method   = HTTP_POST,
        .handler  = ota_set_target_handler,
        .user_ctx = NULL,
    };
    httpd_register_uri_handler(server, &set_target_uri);

    ESP_LOGI(TAG, "OTA HTTP server started on port %d", OTA_PORT);
    ESP_LOGI(TAG, "  GET  /ota/status       — firmware version info");
    ESP_LOGI(TAG, "  POST /ota              — upload new firmware binary");
    ESP_LOGI(TAG, "  POST /ota/recalibrate  — clear gain-lock NVS + reboot");
    ESP_LOGI(TAG, "  POST /ota/set-target   — set CSI target IP:port in NVS + reboot");

    if (out_handle) *out_handle = server;
    return ESP_OK;
}

esp_err_t ota_update_init(void)
{
    /* ADR-050: Load OTA PSK from NVS if provisioned. */
    nvs_handle_t nvs;
    if (nvs_open(OTA_NVS_NAMESPACE, NVS_READONLY, &nvs) == ESP_OK) {
        size_t len = sizeof(s_ota_psk);
        if (nvs_get_str(nvs, OTA_NVS_KEY, s_ota_psk, &len) == ESP_OK) {
            ESP_LOGI(TAG, "OTA PSK loaded from NVS (%d chars) — authentication enabled", (int)len - 1);
        } else {
            ESP_LOGW(TAG, "No OTA PSK in NVS — OTA authentication DISABLED (provision with nvs_set)");
        }
        nvs_close(nvs);
    } else {
        ESP_LOGW(TAG, "NVS namespace '%s' not found — OTA authentication DISABLED", OTA_NVS_NAMESPACE);
    }

    return ota_start_server(NULL);
}

esp_err_t ota_update_init_ex(void **out_server)
{
    return ota_start_server((httpd_handle_t *)out_server);
}
