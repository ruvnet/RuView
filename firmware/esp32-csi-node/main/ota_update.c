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
#include "config_api.h"

#include <string.h>
#include "esp_log.h"
#include "esp_ota_ops.h"
#include "esp_http_server.h"
#include "esp_app_desc.h"
#include "nvs_flash.h"
#include "nvs.h"
#include "esp_system.h"
#include "esp_timer.h"

static const char *TAG = "ota_update";

/** OTA HTTP server port. */
#define OTA_PORT 8032

/** NVS namespace and key for the OTA pre-shared key. */
#define OTA_NVS_NAMESPACE "security"
#define OTA_NVS_KEY       "ota_psk"

/** Maximum PSK length (hex-encoded SHA-256). */
#define OTA_PSK_MAX_LEN   65

/** Cached PSK loaded from NVS at init time. Empty = auth disabled. */
static char s_ota_psk[OTA_PSK_MAX_LEN] = {0};

/**
 * ADR-050: Verify the Authorization header contains the correct PSK.
 * Returns true only when a PSK is provisioned AND the Bearer token
 * matches it. An unprovisioned node refuses all OTA requests
 * (fail-closed, see RuView#596 audit). The OTA server still starts so
 * the operator can `provision.py --ota-psk <hex>` over USB-CDC without
 * a reflash, but the upload endpoint will reject every request until
 * the PSK is set.
 */
static bool ota_check_auth(httpd_req_t *req)
{
    if (s_ota_psk[0] == '\0') {
        /* No PSK provisioned — fail closed. Previously this returned
         * true ("permissive for dev"), which let any host on the WiFi
         * push attacker-controlled firmware to a freshly-flashed node.
         * Plain HTTP transport + no Secure Boot V2 + no signed-image
         * verification meant a single LAN call could brick or back-
         * door a node. Reject until provisioned. */
        ESP_LOGW(TAG, "OTA rejected: no PSK in NVS (run provision.py --ota-psk <hex>)");
        return false;
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

bool ota_auth_check(httpd_req_t *req)
{
    return ota_check_auth(req);
}

/**
 * GET /ota/status — return firmware version and partition info.
 */

/* ------------------------------------------------- rollback lifecycle --- */
/*
 * With CONFIG_BOOTLOADER_APP_ROLLBACK_ENABLE a freshly-OTA'd image boots as
 * ESP_OTA_IMG_PENDING_VERIFY. If it reboots without calling
 * esp_ota_mark_app_valid_cancel_rollback() the bootloader reverts to the
 * previous partition and marks the new one ESP_OTA_IMG_INVALID.
 *
 * WHEN to confirm is the whole design. Confirming in app_main() would make
 * rollback decorative -- any image that starts would qualify. The failure that
 * actually costs someone a ladder and a USB cable is a node that cannot be
 * REACHED, and unreachable means no WiFi. So the criterion is: obtained an IP,
 * and then stayed up for a soak period. The soak also catches an image that
 * associates and then crashes, which confirming on IP alone would miss.
 *
 * Deliberately conservative: a reboot for any other reason inside the soak
 * window rolls back a good image. That errs toward a node that works over a
 * node that is new, which is the right way to be wrong.
 */

#define ROLLBACK_SOAK_US (60 * 1000000ULL)
#define OTA_STATE_NS     "ota_state"

static bool s_pending_verify = false;
static esp_timer_handle_t s_soak_timer = NULL;
/* Sized for the worst case the compiler can prove: label, a 32-char
 * version, the state phrase and the longest reset-reason string. */
static char s_rollback_reason[160] = {0};

static const char *reset_reason_str(esp_reset_reason_t r)
{
    switch (r) {
        case ESP_RST_POWERON:  return "power-on";
        case ESP_RST_SW:       return "software";
        case ESP_RST_PANIC:    return "panic";
        case ESP_RST_INT_WDT:  return "interrupt-watchdog";
        case ESP_RST_TASK_WDT: return "task-watchdog";
        case ESP_RST_WDT:      return "watchdog";
        case ESP_RST_BROWNOUT: return "brownout";
        case ESP_RST_DEEPSLEEP:return "deep-sleep";
        case ESP_RST_EXT:      return "external";
        default:               return "unknown";
    }
}

static void rollback_store_reason(const char *reason)
{
    nvs_handle_t h;
    if (nvs_open(OTA_STATE_NS, NVS_READWRITE, &h) != ESP_OK) return;
    if (reason && reason[0]) nvs_set_str(h, "rb_reason", reason);
    else                     nvs_erase_key(h, "rb_reason");
    nvs_commit(h);
    nvs_close(h);
}

void ota_rollback_boot_check(void)
{
    const esp_partition_t *running = esp_ota_get_running_partition();
    esp_ota_img_states_t st;

    if (running && esp_ota_get_state_partition(running, &st) == ESP_OK &&
        st == ESP_OTA_IMG_PENDING_VERIFY) {
        s_pending_verify = true;
        ESP_LOGW(TAG, "new image on trial (%s): must reach the network and "
                      "survive %llu s or the bootloader reverts",
                 running->label, ROLLBACK_SOAK_US / 1000000ULL);
    }

    /* Did the bootloader already revert something? The failed image is the
     * OTHER slot, left marked INVALID or ABORTED. Reporting this is the point:
     * a node that silently reappears on its old firmware looks identical to
     * one whose update never arrived. */
    const esp_partition_t *other = esp_ota_get_next_update_partition(NULL);
    if (other && esp_ota_get_state_partition(other, &st) == ESP_OK &&
        (st == ESP_OTA_IMG_INVALID || st == ESP_OTA_IMG_ABORTED)) {
        esp_app_desc_t bad;
        const char *ver = (esp_ota_get_partition_description(other, &bad) == ESP_OK)
                          ? bad.version : "unknown";
        snprintf(s_rollback_reason, sizeof(s_rollback_reason),
                 "%.16s image %.32s %s; recovered via %s",
                 other->label, ver,
                 st == ESP_OTA_IMG_INVALID ? "failed verification" : "was aborted",
                 reset_reason_str(esp_reset_reason()));
        ESP_LOGE(TAG, "FIRMWARE ROLLBACK: %s", s_rollback_reason);
        rollback_store_reason(s_rollback_reason);
    } else {
        /* Nothing pending: surface any reason banked by an earlier boot so the
         * server still learns about it if it polled late. */
        nvs_handle_t h;
        if (nvs_open(OTA_STATE_NS, NVS_READONLY, &h) == ESP_OK) {
            size_t len = sizeof(s_rollback_reason);
            if (nvs_get_str(h, "rb_reason", s_rollback_reason, &len) != ESP_OK) {
                s_rollback_reason[0] = '\0';
            }
            nvs_close(h);
        }
    }
}

static void rollback_confirm(void *arg)
{
    (void)arg;
    if (!s_pending_verify) return;
    if (esp_ota_mark_app_valid_cancel_rollback() == ESP_OK) {
        s_pending_verify = false;
        ESP_LOGI(TAG, "new image CONFIRMED: networked and stable, rollback cancelled");
        /* The previous failure, if any, is now history. */
        s_rollback_reason[0] = '\0';
        rollback_store_reason(NULL);
    } else {
        ESP_LOGE(TAG, "could not mark the image valid; it will roll back on reboot");
    }
}

void ota_rollback_notify_connected(void)
{
    if (!s_pending_verify || s_soak_timer) return;
    const esp_timer_create_args_t a = {
        .callback = rollback_confirm,
        .name = "ota_soak",
    };
    if (esp_timer_create(&a, &s_soak_timer) == ESP_OK) {
        esp_timer_start_once(s_soak_timer, ROLLBACK_SOAK_US);
        ESP_LOGI(TAG, "image on trial reached the network; confirming in %llu s",
                 ROLLBACK_SOAK_US / 1000000ULL);
    } else {
        /* Without a soak we cannot confirm, and an unconfirmed image reverts.
         * Confirm now: a working node on new firmware beats a pointless
         * rollback caused by our own resource failure. */
        ESP_LOGW(TAG, "no timer for the soak; confirming immediately");
        rollback_confirm(NULL);
    }
}

static esp_err_t ota_status_handler(httpd_req_t *req)
{
    const esp_app_desc_t *app = esp_app_get_description();
    const esp_partition_t *running = esp_ota_get_running_partition();
    const esp_partition_t *update = esp_ota_get_next_update_partition(NULL);

    char response[1024];
    int len = snprintf(response, sizeof(response),
        "{\"version\":\"%s\",\"date\":\"%s\",\"time\":\"%s\","
        "\"running_partition\":\"%s\",\"next_partition\":\"%s\","
        "\"max_size\":%lu,\"pending_verify\":%s,"
        "\"last_rollback\":%s%s%s}",
        app->version, app->date, app->time,
        running ? running->label : "unknown",
        update ? update->label : "none",
        (unsigned long)(update ? update->size : 0),
        s_pending_verify ? "true" : "false",
        s_rollback_reason[0] ? "\"" : "null",
        s_rollback_reason[0] ? s_rollback_reason : "",
        s_rollback_reason[0] ? "\"" : "");

    httpd_resp_set_type(req, "application/json");
    httpd_resp_send(req, response, len);
    return ESP_OK;
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

    const esp_partition_t *update_partition = esp_ota_get_next_update_partition(NULL);
    if (update_partition == NULL) {
        httpd_resp_send_err(req, HTTPD_500_INTERNAL_SERVER_ERROR,
                            "No OTA partition available");
        return ESP_FAIL;
    }

    if (req->content_len <= 0 || (size_t)req->content_len > update_partition->size) {
        ESP_LOGW(TAG, "OTA rejected: content_length=%d exceeds partition '%s' size=%lu",
                 req->content_len, update_partition->label,
                 (unsigned long)update_partition->size);
        httpd_resp_send_err(req, HTTPD_400_BAD_REQUEST,
                            "Invalid firmware size for OTA partition");
        return ESP_FAIL;
    }

    esp_ota_handle_t ota_handle;
    esp_err_t err = esp_ota_begin(update_partition, OTA_WITH_SEQUENTIAL_WRITES, &ota_handle);
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
    config.max_uri_handlers = 14;  /* WASM endpoints (ADR-040) + /config. */
    /* 12 KB, not the 4 KB default: the upload handler runs esp_ota_end() ->
     * esp_image_verify() on THIS task's stack, which overflows at the end of
     * an upload -- the transfer completes, validation panics, and the node
     * reboots into the old image. Reported upstream as PR #1594. */
    config.stack_size = 12288;
    /* Increase receive timeout for large uploads. */
    config.recv_wait_timeout = 30;

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

    /* Remote configuration shares this server so it inherits the OTA PSK,
     * the port, and the same fail-closed auth path (config_api.c). */
    if (config_api_register(server) != ESP_OK) {
        ESP_LOGW(TAG, "remote config endpoints unavailable");
    }

    ESP_LOGI(TAG, "OTA HTTP server started on port %d", OTA_PORT);
    ESP_LOGI(TAG, "  GET  /ota/status — firmware version info");
    ESP_LOGI(TAG, "  POST /ota        — upload new firmware binary");

    if (out_handle) *out_handle = server;
    return ESP_OK;
}

/**
 * Load the OTA PSK from NVS into the module-local s_ota_psk cache and log
 * the resulting posture. Called by both ota_update_init() and
 * ota_update_init_ex() so the per-boot diagnostic prints no matter which
 * entry point main.c uses — historically only ota_update_init() loaded the
 * PSK, which left ota_update_init_ex() with an empty s_ota_psk and an
 * invisible fail-closed posture (RuView#596 follow-up).
 */
static void ota_load_psk_from_nvs(void)
{
    nvs_handle_t nvs;
    if (nvs_open(OTA_NVS_NAMESPACE, NVS_READONLY, &nvs) == ESP_OK) {
        size_t len = sizeof(s_ota_psk);
        if (nvs_get_str(nvs, OTA_NVS_KEY, s_ota_psk, &len) == ESP_OK) {
            ESP_LOGI(TAG, "OTA PSK loaded from NVS (%d chars) — authentication enabled", (int)len - 1);
        } else {
            ESP_LOGW(TAG, "No OTA PSK in NVS — OTA upload endpoint will REJECT all requests until "
                          "provisioned (provision.py --ota-psk <hex>). Fail-closed per RuView#596.");
        }
        nvs_close(nvs);
    } else {
        ESP_LOGW(TAG, "NVS namespace '%s' not found — OTA upload endpoint will REJECT all "
                      "requests until provisioned. Fail-closed per RuView#596.", OTA_NVS_NAMESPACE);
    }
}

esp_err_t ota_update_init(void)
{
    /* ADR-050: Load OTA PSK from NVS if provisioned. */
    ota_load_psk_from_nvs();
    return ota_start_server(NULL);
}

esp_err_t ota_update_init_ex(void **out_server)
{
    /* ADR-050: Load OTA PSK from NVS if provisioned. main.c uses this
     * variant (not ota_update_init), so without this call s_ota_psk
     * stayed empty forever and the fail-closed posture was invisible
     * in serial logs. */
    ota_load_psk_from_nvs();
    return ota_start_server((httpd_handle_t *)out_server);
}
