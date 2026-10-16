/**
 * @file config_api.c
 * @brief Remote configuration endpoint. See config_api.h for the design.
 */

#include "config_api.h"

#include <string.h>
#include <stdlib.h>
#include "cJSON.h"
#include "ota_update.h"
#include "nvs_config.h"
#include "esp_log.h"
#include "esp_system.h"
#include "esp_timer.h"
#include "nvs_flash.h"
#include "nvs.h"

static const char *TAG = "config_api";

#define CFG_NS        "csi_cfg"     /* the live config nvs_config.c reads */
#define TRIAL_NS      "cfg_trial"   /* trial state, separate so a wipe is easy */
#define TRIAL_DEFAULT 120           /* s to prove association before reverting */
#define TRIAL_MAX     900
#define BODY_MAX      2048

typedef enum { T_STR, T_U8, T_U16, T_U32 } cfg_type_t;

/**
 * The writable surface.
 *
 * `nvs` MUST match the key nvs_config_load() reads, and nothing enforces that
 * -- a typo here writes a key nobody loads. The GET handler exists partly as
 * the check: it reads back through the same keys, so a setting that did not
 * take is visible immediately rather than silently ignored.
 *
 * `scale` handles the thresholds nvs_config.c stores as fixed point (u16 of
 * value*1000) while exposing a float; 0 means the value passes through.
 *
 * `assoc` marks the keys that can orphan a node, and is the ONLY thing that
 * decides whether a change goes through the trial path.
 */
typedef struct {
    const char *json;
    const char *nvs;
    cfg_type_t  type;
    uint32_t    lo, hi;
    size_t      maxlen;
    float       scale;
    bool        assoc;
} cfg_key_t;

static const cfg_key_t KEYS[] = {
    /* json                 nvs             type    lo    hi     len  scale  assoc */
    { "wifi_ssid",          "ssid",         T_STR,   0,     0,    32,   0,   true  },
    { "wifi_password",      "password",     T_STR,   0,     0,    64,   0,   true  },
    { "channel_hop_count",  "hop_count",    T_U8,    1,    14,     0,   0,   true  },
    { "dwell_ms",           "dwell_ms",     T_U32,  10, 60000,     0,   0,   true  },
    { "csi_channel",        "csi_channel",  T_U8,    0,    14,     0,   0,   true  },

    { "target_ip",          "target_ip",    T_STR,   0,     0,    16,   0,   false },
    { "target_port",        "target_port",  T_U16,   1, 65535,     0,   0,   false },
    { "node_id",            "node_id",      T_U8,    0,   254,     0,   0,   false },
    { "tdm_slot_index",     "tdm_slot",     T_U8,    0,    63,     0,   0,   false },
    { "tdm_node_count",     "tdm_nodes",    T_U8,    1,    64,     0,   0,   false },
    { "beacon_period_ms",   "beacon_ms",    T_U16,   0, 10000,     0,   0,   false },
    { "edge_tier",          "edge_tier",    T_U8,    0,     2,     0,   0,   false },
    { "presence_thresh",    "pres_thresh",  T_U16,   0, 65535,     0, 1000,  false },
    { "fall_thresh",        "fall_thresh",  T_U16,   0, 65535,     0, 1000,  false },
    { "vital_window",       "vital_win",    T_U16,   8,  4096,     0,   0,   false },
    { "vital_interval_ms",  "vital_int",    T_U16, 100, 60000,     0,   0,   false },
    { "top_k_count",        "subk_count",   T_U8,    1,    64,     0,   0,   false },
    { "power_duty",         "power_duty",   T_U8,   10,   100,     0,   0,   false },
    { "zone_name",          "zone_name",    T_STR,   0,     0,    16,   0,   false },
    { "swarm_heartbeat_sec","swarm_hb",     T_U16,   1, 65535,     0,   0,   false },
    { "swarm_ingest_sec",   "swarm_ingest", T_U16,   1, 65535,     0,   0,   false },
    { "led_mode",           "led_mode",     T_U8,    0,     2,     0,   0,   false },
    { "led_brightness",     "led_bright",   T_U8,    0,   100,     0,   0,   false },
};
#define NKEYS (sizeof(KEYS) / sizeof(KEYS[0]))

/* Body fields that control the request rather than naming a stored parameter.
 * They are not in KEYS, so every pass over the body must skip them. */
/**
 * Parameters the running firmware re-reads every cycle, so a change takes
 * effect with no restart. Everything else is consumed once during init, which
 * is why a plain config push reboots by default.
 */
static bool is_live(const char *name)
{
    return strcmp(name, "led_mode") == 0 ||
           strcmp(name, "led_brightness") == 0;
}

static bool is_meta(const char *name)
{
    return strcmp(name, "trial_seconds") == 0 ||
           strcmp(name, "reboot") == 0;
}

static esp_timer_handle_t s_revert_timer = NULL;
static bool s_trial_armed = false;

static const cfg_key_t *find_key(const char *json)
{
    for (size_t i = 0; i < NKEYS; i++) {
        if (strcmp(KEYS[i].json, json) == 0) return &KEYS[i];
    }
    return NULL;
}

/* ---------------------------------------------------------------- reads --- */

/**
 * Current NVS value as a cJSON node, or JSON null when the key is unset.
 *
 * `redact` must be false when banking values for a trial: the bank is written
 * back verbatim on revert, so a redacted password would be restored as the
 * literal string "(set)" and lock the node off the network permanently -- the
 * exact failure the trial exists to prevent.
 */
static cJSON *read_key(nvs_handle_t h, const cfg_key_t *k, bool redact)
{
    if (k->type == T_STR) {
        char buf[96];
        size_t len = sizeof(buf);
        if (nvs_get_str(h, k->nvs, buf, &len) != ESP_OK) return cJSON_CreateNull();
        /* Never hand back key material, even to an authenticated caller: it
         * would end up in logs and terminal scrollback. Presence only. */
        if (redact && strcmp(k->json, "wifi_password") == 0) {
            return cJSON_CreateString(buf[0] ? "(set)" : "");
        }
        return cJSON_CreateString(buf);
    }
    uint32_t v = 0;
    esp_err_t e;
    if (k->type == T_U8) {
        uint8_t x; e = nvs_get_u8(h, k->nvs, &x); v = x;
    } else if (k->type == T_U16) {
        uint16_t x; e = nvs_get_u16(h, k->nvs, &x); v = x;
    } else {
        e = nvs_get_u32(h, k->nvs, &v);
    }
    if (e != ESP_OK) return cJSON_CreateNull();
    if (k->scale > 0) return cJSON_CreateNumber((double)v / k->scale);
    return cJSON_CreateNumber((double)v);
}

/* --------------------------------------------------------------- writes --- */

/**
 * Validate and stage one value. Returns false with `err` set on a bad value.
 * Nothing is committed here; the caller commits once the whole body validates,
 * so a body with one bad field changes nothing at all.
 */
static bool write_key(nvs_handle_t h, const cfg_key_t *k, const cJSON *v,
                      char *err, size_t errlen)
{
    if (k->type == T_STR) {
        if (!cJSON_IsString(v)) {
            snprintf(err, errlen, "%s must be a string", k->json);
            return false;
        }
        if (strlen(v->valuestring) >= k->maxlen) {
            snprintf(err, errlen, "%s exceeds %u chars", k->json,
                     (unsigned)(k->maxlen - 1));
            return false;
        }
        return nvs_set_str(h, k->nvs, v->valuestring) == ESP_OK;
    }

    if (!cJSON_IsNumber(v)) {
        snprintf(err, errlen, "%s must be a number", k->json);
        return false;
    }
    double d = v->valuedouble;
    if (k->scale > 0) d *= k->scale;
    if (d < 0) d = 0;
    uint32_t val = (uint32_t)(d + 0.5);
    if (val < k->lo || val > k->hi) {
        snprintf(err, errlen, "%s out of range %lu..%lu", k->json,
                 (unsigned long)k->lo, (unsigned long)k->hi);
        return false;
    }
    if (k->type == T_U8)  return nvs_set_u8(h, k->nvs, (uint8_t)val) == ESP_OK;
    if (k->type == T_U16) return nvs_set_u16(h, k->nvs, (uint16_t)val) == ESP_OK;
    return nvs_set_u32(h, k->nvs, val) == ESP_OK;
}

/** Restore one banked value; JSON null means the key was previously unset. */
static void restore_key(nvs_handle_t h, const cfg_key_t *k, const cJSON *v)
{
    if (cJSON_IsNull(v)) {
        nvs_erase_key(h, k->nvs);       /* absence is a state worth restoring */
        return;
    }
    char err[64];
    write_key(h, k, v, err, sizeof(err));
}

/* ---------------------------------------------------------------- trial --- */

/** Apply the banked values and reboot. Runs from the esp_timer task. */
static void trial_revert(void *arg)
{
    (void)arg;
    ESP_LOGE(TAG, "config trial FAILED to associate -- reverting");

    nvs_handle_t t, c;
    if (nvs_open(TRIAL_NS, NVS_READWRITE, &t) != ESP_OK) {
        /* Cannot read the bank, so cannot revert. Rebooting would spin
         * forever; stay up so the node is at least diagnosable if it is
         * somehow still reachable. */
        ESP_LOGE(TAG, "trial namespace unreadable; cannot revert");
        return;
    }

    size_t len = 0;
    char *json = NULL;
    if (nvs_get_str(t, "bank", NULL, &len) == ESP_OK && len > 1 &&
        (json = malloc(len)) != NULL &&
        nvs_get_str(t, "bank", json, &len) == ESP_OK &&
        nvs_open(CFG_NS, NVS_READWRITE, &c) == ESP_OK) {

        cJSON *root = cJSON_Parse(json);
        if (root) {
            cJSON *it = NULL;
            cJSON_ArrayForEach(it, root) {
                const cfg_key_t *k = find_key(it->string);
                if (k) {
                    restore_key(c, k, it);
                    ESP_LOGW(TAG, "reverted %s", k->json);
                }
            }
            cJSON_Delete(root);
        }
        nvs_commit(c);
        nvs_close(c);
    }
    free(json);

    /* Clear the trial BEFORE rebooting: if the restored config is itself
     * broken we must not loop reverting forever. One attempt, then the node
     * comes up on the old values and stays there. */
    nvs_erase_all(t);
    nvs_commit(t);
    nvs_close(t);

    ESP_LOGE(TAG, "rebooting onto the previous configuration");
    esp_restart();
}

void config_trial_boot_check(void)
{
    nvs_handle_t t;
    if (nvs_open(TRIAL_NS, NVS_READONLY, &t) != ESP_OK) return;

    uint8_t armed = 0;
    uint32_t deadline = TRIAL_DEFAULT;
    nvs_get_u8(t, "armed", &armed);
    nvs_get_u32(t, "deadline", &deadline);
    nvs_close(t);
    if (!armed) return;

    s_trial_armed = true;
    if (deadline == 0 || deadline > TRIAL_MAX) deadline = TRIAL_DEFAULT;
    ESP_LOGW(TAG, "config trial pending: %lu s to associate or revert",
             (unsigned long)deadline);

    const esp_timer_create_args_t args = {
        .callback = trial_revert,
        .name = "cfg_trial",
    };
    if (esp_timer_create(&args, &s_revert_timer) == ESP_OK) {
        esp_timer_start_once(s_revert_timer, (uint64_t)deadline * 1000000ULL);
    } else {
        ESP_LOGE(TAG, "could not arm revert timer; trial cannot self-heal");
    }
}

void config_trial_notify_connected(void)
{
    if (!s_trial_armed) return;
    s_trial_armed = false;

    if (s_revert_timer) {
        esp_timer_stop(s_revert_timer);
        esp_timer_delete(s_revert_timer);
        s_revert_timer = NULL;
    }

    nvs_handle_t t;
    if (nvs_open(TRIAL_NS, NVS_READWRITE, &t) == ESP_OK) {
        nvs_erase_all(t);
        nvs_commit(t);
        nvs_close(t);
    }
    ESP_LOGI(TAG, "config trial COMMITTED -- association proved on new settings");
}

/* ------------------------------------------------------------- handlers --- */

static esp_err_t recv_body(httpd_req_t *req, char **out)
{
    if (req->content_len <= 0 || req->content_len > BODY_MAX) return ESP_FAIL;
    char *buf = malloc(req->content_len + 1);
    if (!buf) return ESP_ERR_NO_MEM;
    int got = 0;
    while (got < req->content_len) {
        int r = httpd_req_recv(req, buf + got, req->content_len - got);
        if (r <= 0) { free(buf); return ESP_FAIL; }
        got += r;
    }
    buf[got] = '\0';
    *out = buf;
    return ESP_OK;
}

static esp_err_t config_get_handler(httpd_req_t *req)
{
    if (!ota_auth_check(req)) {
        httpd_resp_send_err(req, HTTPD_403_FORBIDDEN,
                            "Authentication required. Use: Authorization: Bearer <psk>");
        return ESP_FAIL;
    }

    nvs_handle_t h;
    cJSON *root = cJSON_CreateObject();
    cJSON *vals = cJSON_AddObjectToObject(root, "config");
    if (nvs_open(CFG_NS, NVS_READONLY, &h) == ESP_OK) {
        for (size_t i = 0; i < NKEYS; i++) {
            cJSON_AddItemToObject(vals, KEYS[i].json, read_key(h, &KEYS[i], true));
        }
        nvs_close(h);
    }

    /* Tell the caller which fields will force a reboot-and-prove cycle, so a
     * tool can warn before sending rather than discovering it afterwards. */
    cJSON *risky = cJSON_AddArrayToObject(root, "requires_trial");
    for (size_t i = 0; i < NKEYS; i++) {
        if (KEYS[i].assoc) cJSON_AddItemToArray(risky, cJSON_CreateString(KEYS[i].json));
    }
    cJSON_AddBoolToObject(root, "trial_pending", s_trial_armed);

    char *out = cJSON_PrintUnformatted(root);
    cJSON_Delete(root);
    httpd_resp_set_type(req, "application/json");
    httpd_resp_sendstr(req, out ? out : "{}");
    free(out);
    return ESP_OK;
}

static void reboot_soon(void *arg)
{
    (void)arg;
    esp_restart();
}

static esp_err_t config_post_handler(httpd_req_t *req)
{
    if (!ota_auth_check(req)) {
        ESP_LOGW(TAG, "config write rejected: authentication failed");
        httpd_resp_send_err(req, HTTPD_403_FORBIDDEN,
                            "Authentication required. Use: Authorization: Bearer <psk>");
        return ESP_FAIL;
    }
    if (s_trial_armed) {
        /* A second change stacked on an unproven one would bank the UNPROVEN
         * values as the fallback, so a revert would restore settings never
         * shown to work. Refuse until the pending trial resolves. */
        httpd_resp_set_status(req, "409 Conflict");
        httpd_resp_set_type(req, "application/json");
        httpd_resp_sendstr(req,
            "{\"error\":\"a config trial is still pending; "
            "wait for it to commit or revert\"}");
        return ESP_OK;
    }

    char *body = NULL;
    if (recv_body(req, &body) != ESP_OK) {
        httpd_resp_send_err(req, HTTPD_400_BAD_REQUEST, "Missing or oversized body");
        return ESP_FAIL;
    }
    cJSON *root = cJSON_Parse(body);
    free(body);
    if (!root || !cJSON_IsObject(root)) {
        cJSON_Delete(root);
        httpd_resp_send_err(req, HTTPD_400_BAD_REQUEST, "Body must be a JSON object");
        return ESP_FAIL;
    }

    /* Pass 1: recognise every field and note whether any can orphan the node.
     * Rejecting an unknown key rather than ignoring it turns a typo into a
     * visible error instead of a setting that silently never applied. */
    bool needs_trial = false;
    bool all_live = true;
    char err[96] = {0};
    cJSON *it = NULL;
    cJSON_ArrayForEach(it, root) {
        if (is_meta(it->string)) continue;
        const cfg_key_t *k = find_key(it->string);
        if (!k) {
            snprintf(err, sizeof(err), "unknown parameter '%s'", it->string);
            break;
        }
        if (k->assoc) needs_trial = true;
        if (!is_live(k->json)) all_live = false;
    }
    if (err[0]) {
        cJSON_Delete(root);
        httpd_resp_send_err(req, HTTPD_400_BAD_REQUEST, err);
        return ESP_FAIL;
    }

    /* Pass 2: bank the CURRENT values before anything is written, reading
     * through a separate read-only handle so pending writes cannot leak in. */
    cJSON *bank = NULL;
    if (needs_trial) {
        nvs_handle_t ro;
        bank = cJSON_CreateObject();
        if (nvs_open(CFG_NS, NVS_READONLY, &ro) == ESP_OK) {
            cJSON_ArrayForEach(it, root) {
                if (is_meta(it->string)) continue;
                const cfg_key_t *k = find_key(it->string);
                cJSON_AddItemToObject(bank, k->json, read_key(ro, k, false));
            }
            nvs_close(ro);
        }
    }

    /* Pass 3: stage every write. Nothing is committed yet, so a single bad
     * value leaves the node exactly as it was. */
    nvs_handle_t rw;
    if (nvs_open(CFG_NS, NVS_READWRITE, &rw) != ESP_OK) {
        cJSON_Delete(root);
        cJSON_Delete(bank);
        httpd_resp_send_err(req, HTTPD_500_INTERNAL_SERVER_ERROR, "NVS unavailable");
        return ESP_FAIL;
    }
    int changed = 0;
    cJSON_ArrayForEach(it, root) {
        if (is_meta(it->string)) continue;
        const cfg_key_t *k = find_key(it->string);
        if (!write_key(rw, k, it, err, sizeof(err))) {
            if (!err[0]) snprintf(err, sizeof(err), "could not write %s", k->json);
            break;
        }
        changed++;
    }
    if (err[0]) {
        nvs_close(rw);                      /* uncommitted writes are discarded */
        cJSON_Delete(root);
        cJSON_Delete(bank);
        httpd_resp_send_err(req, HTTPD_400_BAD_REQUEST, err);
        return ESP_FAIL;
    }

    cJSON *rb = cJSON_GetObjectItem(root, "reboot");
    bool reboot = cJSON_IsBool(rb) ? cJSON_IsTrue(rb) : !all_live;

    /* Pass 4: arm the trial BEFORE committing the new config. If the bank
     * cannot be stored we must not proceed -- committing first would reboot
     * the node onto unproven settings with nothing to fall back to. */
    uint32_t deadline = TRIAL_DEFAULT;
    if (needs_trial) {
        cJSON *d = cJSON_GetObjectItem(root, "trial_seconds");
        if (cJSON_IsNumber(d) && d->valuedouble >= 30 && d->valuedouble <= TRIAL_MAX) {
            deadline = (uint32_t)d->valuedouble;
        }
        char *bj = cJSON_PrintUnformatted(bank);
        nvs_handle_t t;
        bool ok = bj && nvs_open(TRIAL_NS, NVS_READWRITE, &t) == ESP_OK;
        if (ok) {
            ok = nvs_set_str(t, "bank", bj) == ESP_OK &&
                 nvs_set_u32(t, "deadline", deadline) == ESP_OK &&
                 nvs_set_u8(t, "armed", 1) == ESP_OK &&
                 nvs_commit(t) == ESP_OK;
            nvs_close(t);
        }
        free(bj);
        if (!ok) {
            nvs_close(rw);
            cJSON_Delete(root);
            cJSON_Delete(bank);
            httpd_resp_send_err(req, HTTPD_500_INTERNAL_SERVER_ERROR,
                                "Could not bank the previous config; refusing to apply");
            return ESP_FAIL;
        }
    }

    esp_err_t ce = nvs_commit(rw);
    nvs_close(rw);

    if (ce == ESP_OK) {
        /* Push live parameters into the running config so they apply now.
         * Values were range-checked in pass 3, so these are already valid. */
        cJSON *lm = cJSON_GetObjectItem(root, "led_mode");
        if (cJSON_IsNumber(lm)) g_nvs_config.led_mode = (uint8_t)lm->valuedouble;
        cJSON *lb = cJSON_GetObjectItem(root, "led_brightness");
        if (cJSON_IsNumber(lb)) g_nvs_config.led_brightness = (uint8_t)lb->valuedouble;
    }

    cJSON_Delete(root);
    cJSON_Delete(bank);
    if (ce != ESP_OK) {
        httpd_resp_send_err(req, HTTPD_500_INTERNAL_SERVER_ERROR, "Commit failed");
        return ESP_FAIL;
    }

    cJSON *resp = cJSON_CreateObject();
    cJSON_AddNumberToObject(resp, "changed", changed);
    cJSON_AddBoolToObject(resp, "trial", needs_trial);
    cJSON_AddBoolToObject(resp, "rebooting", needs_trial || reboot);
    if (needs_trial) {
        cJSON_AddNumberToObject(resp, "trial_seconds", deadline);
        cJSON_AddStringToObject(resp, "note",
            "rebooting; reverts automatically unless the node re-associates");
    } else if (reboot) {
        cJSON_AddStringToObject(resp, "note", "written; rebooting to apply");
    } else if (all_live) {
        cJSON_AddStringToObject(resp, "note", "applied immediately; no restart needed");
    } else {
        cJSON_AddStringToObject(resp, "note",
            "written but NOT yet in effect; config is read only at boot");
    }
    char *out = cJSON_PrintUnformatted(resp);
    cJSON_Delete(resp);
    httpd_resp_set_type(req, "application/json");
    httpd_resp_sendstr(req, out ? out : "{}");
    free(out);

    if (needs_trial || reboot) {
        if (needs_trial) {
            ESP_LOGW(TAG, "config trial armed (%lu s); rebooting",
                     (unsigned long)deadline);
        } else {
            ESP_LOGI(TAG, "config written (%d field(s)); rebooting to apply", changed);
        }
        /* Deferred so the response actually reaches the caller -- otherwise
         * the tool sees a dropped connection and cannot tell success from a
         * crash at the worst possible moment.
         *
         * 3 s, not 1 s: at 1 s, two of three trial pushes to a node under
         * sendto-ENOMEM congestion returned an empty body. The write had
         * succeeded and the node rebooted correctly, but the caller could not
         * tell that from a failure -- and the natural response to an apparent
         * failure is to push again, which is the worst thing to do to a node
         * that is mid-trial. */
        const esp_timer_create_args_t a = { .callback = reboot_soon, .name = "cfg_boot" };
        esp_timer_handle_t th;
        if (esp_timer_create(&a, &th) == ESP_OK) esp_timer_start_once(th, 3000000ULL);
    }
    return ESP_OK;
}

esp_err_t config_api_register(httpd_handle_t server)
{
    static const httpd_uri_t get_uri = {
        .uri = "/config", .method = HTTP_GET, .handler = config_get_handler,
    };
    static const httpd_uri_t post_uri = {
        .uri = "/config", .method = HTTP_POST, .handler = config_post_handler,
    };
    esp_err_t e = httpd_register_uri_handler(server, &get_uri);
    if (e != ESP_OK) return e;
    return httpd_register_uri_handler(server, &post_uri);
}
