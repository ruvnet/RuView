/**
 * @file wasm_upload.c
 * @brief ADR-040 — HTTP endpoints for WASM module upload and management.
 *
 * Registers REST endpoints on the existing OTA HTTP server (port 8032):
 *   POST   /wasm/upload    — Upload RVF or raw .wasm (max 128 KB + RVF overhead)
 *   GET    /wasm/list       — List loaded modules with state, manifest, counters
 *   POST   /wasm/start/:id  — Start a loaded module (calls on_init)
 *   POST   /wasm/stop/:id   — Stop a running module
 *   DELETE /wasm/:id        — Unload a module and free memory
 *
 * Upload accepts two formats:
 *   1. RVF container (preferred): header + manifest + WASM + signature
 *   2. Raw .wasm binary (only when wasm_verify=0, for lab/dev use)
 *
 * Detection is by magic bytes: "RVF\x01" vs "\0asm".
 */

#include "sdkconfig.h"
#include "wasm_upload.h"

#if defined(CONFIG_WASM_ENABLE)

#include "wasm_runtime.h"
#include "rvf_parser.h"
#include "nvs_config.h"

#include <string.h>
#include <stdio.h>
#include "esp_log.h"
#include "esp_heap_caps.h"

static const char *TAG = "wasm_upload";

/* Max upload size: RVF overhead + max WASM binary. */
#define MAX_UPLOAD_SIZE (RVF_HEADER_SIZE + RVF_MANIFEST_SIZE + \
                         WASM_MAX_MODULE_SIZE + RVF_SIGNATURE_LEN + 4096)

/* ======================================================================
 * Receive full request body into PSRAM buffer
 * ====================================================================== */

static uint8_t *receive_body(httpd_req_t *req, int *out_len)
{
    if (req->content_len <= 0) {
        httpd_resp_send_err(req, HTTPD_400_BAD_REQUEST, "Empty body");
        return NULL;
    }
    if (req->content_len > MAX_UPLOAD_SIZE) {
        char msg[80];
        snprintf(msg, sizeof(msg), "Upload too large (%d > %d)",
                 req->content_len, MAX_UPLOAD_SIZE);
        httpd_resp_send_err(req, HTTPD_400_BAD_REQUEST, msg);
        return NULL;
    }

    uint8_t *buf = heap_caps_malloc(req->content_len, MALLOC_CAP_SPIRAM | MALLOC_CAP_8BIT);
    if (buf == NULL) buf = malloc(req->content_len);
    if (buf == NULL) {
        httpd_resp_send_err(req, HTTPD_500_INTERNAL_SERVER_ERROR, "Out of memory");
        return NULL;
    }

    int total = 0;
    while (total < req->content_len) {
        int received = httpd_req_recv(req, (char *)(buf + total),
                                       req->content_len - total);
        if (received <= 0) {
            if (received == HTTPD_SOCK_ERR_TIMEOUT) continue;
            free(buf);
            httpd_resp_send_err(req, HTTPD_500_INTERNAL_SERVER_ERROR, "Receive error");
            return NULL;
        }
        total += received;
    }

    *out_len = total;
    return buf;
}

/* ======================================================================
 * POST /wasm/upload — Upload RVF or raw .wasm
 * ====================================================================== */

static esp_err_t wasm_upload_handler(httpd_req_t *req)
{
    int total = 0;
    uint8_t *buf = receive_body(req, &total);
    if (buf == NULL) return ESP_FAIL;

    ESP_LOGI(TAG, "Received upload: %d bytes", total);

    uint8_t module_id = 0;
    esp_err_t err;
    const char *format = "raw";

    if (rvf_is_rvf(buf, (uint32_t)total)) {
        /* ── RVF path ── */
        format = "rvf";
        rvf_parsed_t parsed;
        err = rvf_parse(buf, (uint32_t)total, &parsed);
        if (err != ESP_OK) {
            free(buf);
            char msg[80];
            snprintf(msg, sizeof(msg), "RVF parse failed: %s", esp_err_to_name(err));
            httpd_resp_send_err(req, HTTPD_400_BAD_REQUEST, msg);
            return ESP_FAIL;
        }

        /* ADR-050: Verify signature (default-on; skip only if
         * CONFIG_WASM_SKIP_SIGNATURE is explicitly set for dev/lab). */
#ifndef CONFIG_WASM_SKIP_SIGNATURE
        {
            /* Load pubkey from NVS config (set via provision.py --wasm-pubkey). */
            extern nvs_config_t g_nvs_config;
            if (!g_nvs_config.wasm_pubkey_valid) {
                free(buf);
                httpd_resp_send_err(req, HTTPD_403_FORBIDDEN,
                                    "wasm_verify enabled but no pubkey in NVS. "
                                    "Provision with: provision.py --wasm-pubkey <hex>");
                return ESP_FAIL;
            }
            if (parsed.signature == NULL) {
                free(buf);
                httpd_resp_send_err(req, HTTPD_403_FORBIDDEN,
                                    "RVF has no signature (wasm_verify is enabled)");
                return ESP_FAIL;
            }
            err = rvf_verify_signature(&parsed, buf, g_nvs_config.wasm_pubkey);
            if (err != ESP_OK) {
                free(buf);
                httpd_resp_send_err(req, HTTPD_403_FORBIDDEN,
                                    "Signature verification failed");
                return ESP_FAIL;
            }
        }
#endif

        /* Load WASM payload into runtime. */
        err = wasm_runtime_load(parsed.wasm_data, parsed.wasm_len, &module_id);
        if (err != ESP_OK) {
            free(buf);
            char msg[80];
            snprintf(msg, sizeof(msg), "WASM load failed: %s", esp_err_to_name(err));
            httpd_resp_send_err(req, HTTPD_500_INTERNAL_SERVER_ERROR, msg);
            return ESP_FAIL;
        }

        /* Apply manifest to the slot. */
        wasm_runtime_set_manifest(module_id,
                                   parsed.manifest->module_name,
                                   parsed.manifest->capabilities,
                                   parsed.manifest->max_frame_us);

        /* Auto-start. */
        err = wasm_runtime_start(module_id);

        char response[256];
        snprintf(response, sizeof(response),
                 "{\"status\":\"ok\",\"format\":\"rvf\","
                 "\"module_id\":%u,\"name\":\"%s\","
                 "\"wasm_size\":%lu,\"caps\":\"0x%04lx\","
                 "\"budget_us\":%lu,\"started\":%s}",
                 module_id, parsed.manifest->module_name,
                 (unsigned long)parsed.wasm_len,
                 (unsigned long)parsed.manifest->capabilities,
                 (unsigned long)parsed.manifest->max_frame_us,
                 (err == ESP_OK) ? "true" : "false");

        free(buf);
        httpd_resp_set_type(req, "application/json");
        httpd_resp_send(req, response, strlen(response));
        return ESP_OK;

    } else if (rvf_is_raw_wasm(buf, (uint32_t)total)) {
        /* ── Raw WASM path (dev/lab only) ── */
#ifndef CONFIG_WASM_SKIP_SIGNATURE
        free(buf);
        httpd_resp_send_err(req, HTTPD_403_FORBIDDEN,
                            "Raw WASM upload rejected (signature verification enabled). "
                            "Use RVF container with signature, or set CONFIG_WASM_SKIP_SIGNATURE for dev.");
        return ESP_FAIL;
#else
        format = "raw";
        err = wasm_runtime_load(buf, (uint32_t)total, &module_id);
        free(buf);

        if (err != ESP_OK) {
            char msg[80];
            snprintf(msg, sizeof(msg), "Load failed: %s", esp_err_to_name(err));
            httpd_resp_send_err(req, HTTPD_500_INTERNAL_SERVER_ERROR, msg);
            return ESP_FAIL;
        }

        err = wasm_runtime_start(module_id);

        char response[128];
        snprintf(response, sizeof(response),
                 "{\"status\":\"ok\",\"format\":\"raw\","
                 "\"module_id\":%u,\"size\":%d,\"started\":%s}",
                 module_id, total, (err == ESP_OK) ? "true" : "false");
        httpd_resp_set_type(req, "application/json");
        httpd_resp_send(req, response, strlen(response));
        return ESP_OK;
#endif
    } else {
        free(buf);
        httpd_resp_send_err(req, HTTPD_400_BAD_REQUEST,
                            "Unrecognized format (expected RVF or raw WASM)");
        return ESP_FAIL;
    }

    (void)format;
}

/* ======================================================================
 * GET /wasm/list — List module slots
 * ====================================================================== */

static const char *state_name(wasm_module_state_t state)
{
    switch (state) {
        case WASM_MODULE_EMPTY:   return "empty";
        case WASM_MODULE_LOADED:  return "loaded";
        case WASM_MODULE_RUNNING: return "running";
        case WASM_MODULE_STOPPED: return "stopped";
        case WASM_MODULE_ERROR:   return "error";
        default: return "unknown";
    }
}

static esp_err_t wasm_list_handler(httpd_req_t *req)
{
    wasm_module_info_t info[WASM_MAX_MODULES];
    uint8_t count = 0;
    wasm_runtime_get_info(info, &count);

    /* Build JSON array (larger buffer for manifest fields). */
    char response[2048];
    int pos = 0;
    pos += snprintf(response + pos, sizeof(response) - pos,
                    "{\"modules\":[");

    for (uint8_t i = 0; i < WASM_MAX_MODULES; i++) {
        if (i > 0) pos += snprintf(response + pos, sizeof(response) - pos, ",");
        uint32_t mean_us = (info[i].frame_count > 0)
                           ? (info[i].total_us / info[i].frame_count) : 0;
        const char *name = info[i].module_name[0] ? info[i].module_name : "";
        pos += snprintf(response + pos, sizeof(response) - pos,
                        "{\"id\":%u,\"state\":\"%s\",\"name\":\"%s\","
                        "\"binary_size\":%lu,\"caps\":\"0x%04lx\","
                        "\"frame_count\":%lu,\"event_count\":%lu,\"error_count\":%lu,"
                        "\"mean_us\":%lu,\"max_us\":%lu,\"budget_us\":%lu,"
                        "\"budget_faults\":%lu}",
                        info[i].id, state_name(info[i].state), name,
                        (unsigned long)info[i].binary_size,
                        (unsigned long)info[i].capabilities,
                        (unsigned long)info[i].frame_count,
                        (unsigned long)info[i].event_count,
                        (unsigned long)info[i].error_count,
                        (unsigned long)mean_us,
                        (unsigned long)info[i].max_us,
                        (unsigned long)info[i].manifest_budget_us,
                        (unsigned long)info[i].budget_faults);
    }

    pos += snprintf(response + pos, sizeof(response) - pos,
                    "],\"loaded\":%u,\"max\":%d}", count, WASM_MAX_MODULES);

    httpd_resp_set_type(req, "application/json");
    httpd_resp_send(req, response, pos);
    return ESP_OK;
}

/* ======================================================================
 * POST /wasm/start — Start module by ID (parsed from query string)
 * ====================================================================== */

static int parse_module_id_from_uri(const char *uri, const char *prefix)
{
    const char *id_str = uri + strlen(prefix);
    if (*id_str == '\0') return -1;
    int id = atoi(id_str);
    if (id < 0 || id >= WASM_MAX_MODULES) return -1;
    return id;
}

static esp_err_t wasm_start_handler(httpd_req_t *req)
{
    int id = parse_module_id_from_uri(req->uri, "/wasm/start/");
    if (id < 0) {
        httpd_resp_send_err(req, HTTPD_400_BAD_REQUEST, "Invalid module ID");
        return ESP_FAIL;
    }

    esp_err_t err = wasm_runtime_start((uint8_t)id);
    if (err != ESP_OK) {
        char msg[64];
        snprintf(msg, sizeof(msg), "Start failed: %s", esp_err_to_name(err));
        httpd_resp_send_err(req, HTTPD_400_BAD_REQUEST, msg);
        return ESP_FAIL;
    }

    const char *resp = "{\"status\":\"ok\",\"action\":\"started\"}";
    httpd_resp_set_type(req, "application/json");
    httpd_resp_send(req, resp, strlen(resp));
    return ESP_OK;
}

/* ======================================================================
 * POST /wasm/stop — Stop module by ID
 * ====================================================================== */

static esp_err_t wasm_stop_handler(httpd_req_t *req)
{
    int id = parse_module_id_from_uri(req->uri, "/wasm/stop/");
    if (id < 0) {
        httpd_resp_send_err(req, HTTPD_400_BAD_REQUEST, "Invalid module ID");
        return ESP_FAIL;
    }

    esp_err_t err = wasm_runtime_stop((uint8_t)id);
    if (err != ESP_OK) {
        char msg[64];
        snprintf(msg, sizeof(msg), "Stop failed: %s", esp_err_to_name(err));
        httpd_resp_send_err(req, HTTPD_400_BAD_REQUEST, msg);
        return ESP_FAIL;
    }

    const char *resp = "{\"status\":\"ok\",\"action\":\"stopped\"}";
    httpd_resp_set_type(req, "application/json");
    httpd_resp_send(req, resp, strlen(resp));
    return ESP_OK;
}

/* ======================================================================
 * DELETE /wasm/:id — Unload module
 * ====================================================================== */

static esp_err_t wasm_delete_handler(httpd_req_t *req)
{
    int id = parse_module_id_from_uri(req->uri, "/wasm/");
    if (id < 0) {
        httpd_resp_send_err(req, HTTPD_400_BAD_REQUEST, "Invalid module ID");
        return ESP_FAIL;
    }

    esp_err_t err = wasm_runtime_unload((uint8_t)id);
    if (err != ESP_OK) {
        char msg[64];
        snprintf(msg, sizeof(msg), "Unload failed: %s", esp_err_to_name(err));
        httpd_resp_send_err(req, HTTPD_400_BAD_REQUEST, msg);
        return ESP_FAIL;
    }

    const char *resp = "{\"status\":\"ok\",\"action\":\"unloaded\"}";
    httpd_resp_set_type(req, "application/json");
    httpd_resp_send(req, resp, strlen(resp));
    return ESP_OK;
}

/* ======================================================================
 * Register all endpoints
 * ====================================================================== */

esp_err_t wasm_upload_register(httpd_handle_t server)
{
    if (server == NULL) return ESP_ERR_INVALID_ARG;

    httpd_uri_t upload_uri = {
        .uri      = "/wasm/upload",
        .method   = HTTP_POST,
        .handler  = wasm_upload_handler,
        .user_ctx = NULL,
    };
    httpd_register_uri_handler(server, &upload_uri);

    httpd_uri_t list_uri = {
        .uri      = "/wasm/list",
        .method   = HTTP_GET,
        .handler  = wasm_list_handler,
        .user_ctx = NULL,
    };
    httpd_register_uri_handler(server, &list_uri);

    /* Wildcard URIs for start/stop/delete with module ID. */
    httpd_uri_t start_uri = {
        .uri      = "/wasm/start/*",
        .method   = HTTP_POST,
        .handler  = wasm_start_handler,
        .user_ctx = NULL,
    };
    httpd_register_uri_handler(server, &start_uri);

    httpd_uri_t stop_uri = {
        .uri      = "/wasm/stop/*",
        .method   = HTTP_POST,
        .handler  = wasm_stop_handler,
        .user_ctx = NULL,
    };
    httpd_register_uri_handler(server, &stop_uri);

    httpd_uri_t delete_uri = {
        .uri      = "/wasm/*",
        .method   = HTTP_DELETE,
        .handler  = wasm_delete_handler,
        .user_ctx = NULL,
    };
    httpd_register_uri_handler(server, &delete_uri);

    ESP_LOGI(TAG, "WASM upload endpoints registered:");
    ESP_LOGI(TAG, "  POST   /wasm/upload    — upload .wasm binary");
    ESP_LOGI(TAG, "  GET    /wasm/list      — list modules");
    ESP_LOGI(TAG, "  POST   /wasm/start/:id — start module");
    ESP_LOGI(TAG, "  POST   /wasm/stop/:id  — stop module");
    ESP_LOGI(TAG, "  DELETE /wasm/:id       — unload module");

    return ESP_OK;
}

#else /* !CONFIG_WASM_ENABLE */

#include "esp_log.h"

esp_err_t wasm_upload_register(httpd_handle_t server)
{
    (void)server;
    ESP_LOGW("wasm_upload", "WASM upload disabled (CONFIG_WASM_ENABLE not set)");
    return ESP_OK;
}

#endif /* CONFIG_WASM_ENABLE */
