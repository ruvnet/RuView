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
#include "config_rules.h"
#include "esp_log.h"
#include "esp_system.h"
#include "esp_timer.h"
#include "esp_wifi.h"
#include "nvs_flash.h"
#include "nvs.h"

static const char *TAG = "config_api";

/* POST /link/reassoc state (handler near config_api_register). */
#define REASSOC_MIN_INTERVAL_US (60LL * 1000 * 1000)
static uint32_t s_reassoc_count;
static int64_t  s_reassoc_last_us;
static esp_timer_handle_t s_reassoc_timer;

#define CFG_NS        "csi_cfg"     /* the live config nvs_config.c reads */
#define TRIAL_NS      "cfg_trial"   /* trial state, separate so a wipe is easy */
#define TRIAL_DEFAULT 120           /* s to prove association before reverting */
#define TRIAL_MAX     900
#define BODY_MAX      2048

/* The writable surface, the value rules and the "is this field live" test
 * live in config_rules.c, so the host test can pin them without a device. */

static esp_timer_handle_t s_revert_timer = NULL;

/**
 * A boot trial is pending: the node came up on settings nobody has proved and
 * will revert them unless it associates. Set by config_trial_boot_check() and
 * cleared by config_trial_notify_connected() when the association proves the
 * new settings.
 */
static bool s_trial_armed = false;

/**
 * A change has been accepted and the node is about to reboot onto it.
 *
 * This is deliberately NOT s_trial_armed. The boot trial is about settings
 * that are already stored; this is about a request that was just accepted and
 * has not been rebooted into yet. They must stay separate because an
 * association event on the OLD running configuration calls
 * config_trial_notify_connected(), which clears s_trial_armed and erases the
 * trial bank. If the pending state rode on s_trial_armed, that event would
 * unlock the API during the three seconds before the reboot, and the second
 * request would bank the FIRST request's unproven settings as the fallback --
 * so a revert would restore settings never shown to work.
 */
static volatile bool s_pending_reboot = false;

/* ---------------------------------------------------------------- reads --- */

/**
 * Current NVS value as a cJSON node, or JSON null when the key is unset.
 *
 * `redact` must be false when banking values for a trial: the bank is written
 * back verbatim on revert, so a redacted password would be restored as the
 * literal string "(set)" and lock the node off the network permanently -- the
 * exact failure the trial exists to prevent.
 */
static cJSON *read_key(nvs_handle_t h, const cfg_key_t *k, bool redact,
                       esp_err_t *read_status)
{
    if (read_status) *read_status = ESP_OK;
    if (k->type == T_STR) {
        char buf[96];
        size_t len = sizeof(buf);
        esp_err_t e = nvs_get_str(h, k->nvs, buf, &len);
        if (read_status) *read_status = e;
        if (e != ESP_OK) return cJSON_CreateNull();
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
        uint8_t x = 0; e = nvs_get_u8(h, k->nvs, &x); v = x;
    } else if (k->type == T_U16) {
        uint16_t x = 0; e = nvs_get_u16(h, k->nvs, &x); v = x;
    } else {
        e = nvs_get_u32(h, k->nvs, &v);
    }
    if (read_status) *read_status = e;
    if (e != ESP_OK) return cJSON_CreateNull();
    if (k->scale > 0) return cJSON_CreateNumber((double)v / k->scale);
    return cJSON_CreateNumber((double)v);
}

/* --------------------------------------------------------------- writes --- */

/**
 * Stage one value.
 *
 * The value MUST already have been accepted by config_validate_value(): this
 * function only stores. Splitting the two is the whole point -- see the
 * handler below, where every field is validated before any field is written.
 *
 * `num` is the integer to store, already scaled and rounded. It is ignored
 * for T_STR, whose text comes from `v`.
 */
static bool store_key(nvs_handle_t h, const cfg_key_t *k, const cJSON *v,
                      uint32_t num, char *err, size_t errlen)
{
    if (k->type == T_STR) {
        if (!cJSON_IsString(v)) {
            snprintf(err, errlen, "%s must be a string", k->json);
            return false;
        }
        if (nvs_set_str(h, k->nvs, v->valuestring) != ESP_OK) {
            snprintf(err, errlen, "could not write %s", k->json);
            return false;
        }
        return true;
    }

    esp_err_t e;
    if (k->type == T_U8)       e = nvs_set_u8(h, k->nvs, (uint8_t)num);
    else if (k->type == T_U16) e = nvs_set_u16(h, k->nvs, (uint16_t)num);
    else                       e = nvs_set_u32(h, k->nvs, num);
    if (e != ESP_OK) {
        snprintf(err, errlen, "could not write %s", k->json);
        return false;
    }
    return true;
}

/**
 * Decode a cJSON value into the shape the rules work with.
 *
 * A field the body carries as JSON null is treated as absent: the rules
 * reject it, because "unset" is expressed by omitting the key.
 */
static void decode_value(const cJSON *v, cfg_value_t *out)
{
    out->kind = CFG_VAL_NONE;
    out->str = NULL;
    out->num = 0.0;
    if (!v) return;
    if (cJSON_IsString(v)) {
        out->kind = CFG_VAL_STR;
        out->str = v->valuestring;
    } else if (cJSON_IsNumber(v)) {
        out->kind = CFG_VAL_NUM;
        out->num = v->valuedouble;
    }
}

/**
 * Validate one field of a request without storing anything.
 *
 * Returns false with `err` set on a bad value. `scaled` receives the integer
 * that would be stored.
 */
static bool validate_field(const cfg_key_t *k, const cJSON *v, uint32_t *scaled,
                            char *err, size_t errlen)
{
    cfg_value_t val;
    decode_value(v, &val);
    return config_validate_value(k, &val, scaled, err, errlen);
}

/**
 * The integer to store for a banked value.
 *
 * The bank holds a value exactly as it was read from NVS, so it is in range
 * for the key's type -- but it may be OUTSIDE the range the API accepts
 * today (an older firmware wrote vital_window=4096, which the loader
 * ignores). A revert has to put back exactly what was there, so it stores the
 * value as read and never re-validates it against the current limits.
 */
static uint32_t banked_number(const cfg_key_t *k, const cJSON *v)
{
    double d = v->valuedouble;
    if (k->scale > 0) d *= k->scale;
    if (!(d >= 0.0)) return 0;                 /* NaN or negative */
    if (d > 4294967295.0) return 4294967295u;  /* unreachable for a banked value */
    return (uint32_t)(d + 0.5);
}

/** Restore one banked value; JSON null means the key was previously unset. */
static bool restore_key(nvs_handle_t h, const cfg_key_t *k, const cJSON *v)
{
    if (cJSON_IsNull(v)) {
        esp_err_t e = nvs_erase_key(h, k->nvs);
        return e == ESP_OK || e == ESP_ERR_NVS_NOT_FOUND;
    }
    char err[64];
    return store_key(h, k, v, banked_number(k, v), err, sizeof(err));
}

/**
 * Put every banked value back.
 *
 * Used when a write failed part way through: the node is left holding a mix
 * of old and new values, and the only honest way out is to restore the whole
 * bank rather than leave it half applied.
 */
static bool restore_bank(nvs_handle_t h, const cJSON *bank)
{
    bool ok = true;
    cJSON *it = NULL;
    cJSON_ArrayForEach(it, bank) {
        const cfg_key_t *k = config_find_key(it->string);
        if (!k || !restore_key(h, k, it)) {
            ESP_LOGE(TAG, "could not restore banked key");
            ok = false;
        }
    }
    return ok;
}

/* Recovery metadata is discarded only after the previous state is durable. */
static bool clear_recovery_bank(void)
{
    nvs_handle_t h;
    if (nvs_open(TRIAL_NS, NVS_READWRITE, &h) != ESP_OK) return false;
    bool ok = nvs_erase_all(h) == ESP_OK && nvs_commit(h) == ESP_OK;
    nvs_close(h);
    return ok;
}

static bool finish_recovery_bank(bool needs_trial)
{
    if (!needs_trial) return clear_recovery_bank();
    nvs_handle_t h;
    if (nvs_open(TRIAL_NS, NVS_READWRITE, &h) != ESP_OK) return false;
    bool ok = nvs_set_u8(h, "rollback", 0) == ESP_OK && nvs_commit(h) == ESP_OK;
    nvs_close(h);
    return ok;
}

/* ---------------------------------------------------------------- trial --- */

/** Apply the banked values and reboot. Runs from the esp_timer task. */
static void trial_revert(void *arg)
{
    (void)arg;
    s_pending_reboot = true;
    nvs_handle_t t;
    if (nvs_open(TRIAL_NS, NVS_READONLY, &t) != ESP_OK) {
        ESP_LOGE(TAG, "recovery bank unreadable; retaining write lock");
        return;
    }
    size_t len = 0;
    char *json = NULL;
    bool loaded = nvs_get_str(t, "bank", NULL, &len) == ESP_OK && len > 1 &&
                  len <= BODY_MAX * 2 && (json = malloc(len)) != NULL &&
                  nvs_get_str(t, "bank", json, &len) == ESP_OK;
    nvs_close(t);
    cJSON *bank = loaded ? cJSON_Parse(json) : NULL;
    free(json);
    bool restored = false;
    nvs_handle_t c;
    if (cJSON_IsObject(bank) && nvs_open(CFG_NS, NVS_READWRITE, &c) == ESP_OK) {
        restored = restore_bank(c, bank) && nvs_commit(c) == ESP_OK;
        nvs_close(c);
    }
    cJSON_Delete(bank);
    if (!restored || !clear_recovery_bank()) {
        ESP_LOGE(TAG, "recovery failed; keeping bank and write lock for retry");
        return;
    }
    ESP_LOGW(TAG, "previous configuration restored; rebooting");
    esp_restart();
}

void config_trial_boot_check(void)
{
    nvs_handle_t t;
    if (nvs_open(TRIAL_NS, NVS_READONLY, &t) != ESP_OK) return;

    uint8_t armed = 0;
    uint8_t rollback = 0;
    uint32_t deadline = TRIAL_DEFAULT;
    nvs_get_u8(t, "armed", &armed);
    nvs_get_u8(t, "rollback", &rollback);
    nvs_get_u32(t, "deadline", &deadline);
    nvs_close(t);
    if (rollback) {
        trial_revert(NULL);
        return;
    }
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
    if (!s_trial_armed || s_pending_reboot) return;
    if (!clear_recovery_bank()) {
        ESP_LOGE(TAG, "could not commit trial; preserving recovery state");
        return;
    }
    s_trial_armed = false;

    if (s_revert_timer) {
        esp_timer_stop(s_revert_timer);
        esp_timer_delete(s_revert_timer);
        s_revert_timer = NULL;
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
        for (size_t i = 0; i < CONFIG_KEY_COUNT; i++) {
            cJSON_AddItemToObject(vals, CONFIG_KEYS[i].json, read_key(h, &CONFIG_KEYS[i], true, NULL));
        }
        nvs_close(h);
    }

    /* Tell the caller which fields will force a reboot-and-prove cycle, so a
     * tool can warn before sending rather than discovering it afterwards. */
    cJSON *risky = cJSON_AddArrayToObject(root, "requires_trial");
    for (size_t i = 0; i < CONFIG_KEY_COUNT; i++) {
        if (CONFIG_KEYS[i].assoc) cJSON_AddItemToArray(risky, cJSON_CreateString(CONFIG_KEYS[i].json));
    }
    cJSON_AddBoolToObject(root, "trial_pending", s_trial_armed);
    cJSON_AddBoolToObject(root, "write_blocked",
                          !config_write_allowed(s_trial_armed, s_pending_reboot));
    cJSON_AddNumberToObject(root, "reassoc_count", s_reassoc_count);
    cJSON_AddNumberToObject(root, "reassoc_last_s_ago",
        s_reassoc_last_us ? (double)((esp_timer_get_time() - s_reassoc_last_us) / 1000000) : -1);

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
    if (!config_write_allowed(s_trial_armed, s_pending_reboot)) {
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
        if (config_is_meta(it->string)) continue;
        const cfg_key_t *k = config_find_key(it->string);
        if (!k) {
            snprintf(err, sizeof(err), "unknown parameter '%s'", it->string);
            break;
        }
        if (k->assoc) needs_trial = true;
        if (!config_is_live(k->json)) all_live = false;
    }
    if (err[0]) {
        cJSON_Delete(root);
        httpd_resp_send_err(req, HTTPD_400_BAD_REQUEST, err);
        return ESP_FAIL;
    }

    /* Pass 2: validate EVERY value before touching storage.
     *
     * nvs_set_* writes through to flash in the setter itself. In ESP-IDF 5.4
     * nvs_open() hands back an NVSHandleSimple, whose setters call
     * Storage::writeItem() straight away and whose commit() only returns
     * ESP_OK -- closing the handle does NOT discard what was written. A
     * request that validated and wrote in the same pass could therefore leave
     * a REJECTED request half applied: {"wifi_ssid":"wrong-network",
     * "edge_tier":"invalid"} would answer 400 with the SSID already changed
     * and no recovery bank armed, so the next reboot could leave the node
     * unreachable with no way back.
     *
     * Validating the whole body first makes that impossible: a body with one
     * bad field changes nothing at all. */
    cJSON_ArrayForEach(it, root) {
        if (config_is_meta(it->string)) continue;
        const cfg_key_t *k = config_find_key(it->string);
        uint32_t num = 0;
        if (!validate_field(k, it, &num, err, sizeof(err))) {
            if (!err[0]) snprintf(err, sizeof(err), "invalid value for %s", k->json);
            break;
        }
    }
    if (err[0]) {
        cJSON_Delete(root);
        httpd_resp_send_err(req, HTTPD_400_BAD_REQUEST, err);
        return ESP_FAIL;
    }

    cJSON *rb = cJSON_GetObjectItem(root, "reboot");
    bool reboot = cJSON_IsBool(rb) ? cJSON_IsTrue(rb) : !all_live;
    uint32_t deadline = TRIAL_DEFAULT;
    if (needs_trial) {
        cJSON *d = cJSON_GetObjectItem(root, "trial_seconds");
        if (cJSON_IsNumber(d) && d->valuedouble >= 30 && d->valuedouble <= TRIAL_MAX) {
            deadline = (uint32_t)d->valuedouble;
        }
    }

    /* Pass 3: bank the CURRENT values before anything is written, reading
     * through a separate read-only handle so pending writes cannot leak in.
     * The bank is always built, not only for a trial: a write that fails part
     * way through has to be undone, and the bank is what it is undone from. */
    cJSON *bank = cJSON_CreateObject();
    if (!bank) {
        cJSON_Delete(root);
        httpd_resp_send_err(req, HTTPD_500_INTERNAL_SERVER_ERROR, "Out of memory");
        return ESP_FAIL;
    }
    {
        nvs_handle_t ro;
        if (nvs_open(CFG_NS, NVS_READONLY, &ro) != ESP_OK) {
            /* Without a bank there is nothing to revert to, and a write that
             * failed half way through could not be undone. Refuse the change
             * rather than apply one the node could not back out of. */
            cJSON_Delete(root);
            cJSON_Delete(bank);
            httpd_resp_send_err(req, HTTPD_500_INTERNAL_SERVER_ERROR,
                                "Could not read the current config");
            return ESP_FAIL;
        }
        cJSON_ArrayForEach(it, root) {
            if (config_is_meta(it->string)) continue;
            const cfg_key_t *k = config_find_key(it->string);
            esp_err_t read_status;
            cJSON *value = read_key(ro, k, false, &read_status);
            if (!value || (read_status != ESP_OK && read_status != ESP_ERR_NVS_NOT_FOUND) ||
                !cJSON_AddItemToObject(bank, k->json, value)) {
                cJSON_Delete(value);
                nvs_close(ro);
                cJSON_Delete(root);
                cJSON_Delete(bank);
                httpd_resp_send_err(req, HTTPD_500_INTERNAL_SERVER_ERROR,
                                    "Could not snapshot the complete current config");
                return ESP_FAIL;
            }
        }
        nvs_close(ro);
    }

    /* Pass 4: persist the recovery state BEFORE the first live write.
     *
     * If the bank cannot be stored we must not proceed: writing first would
     * leave the node on unproven settings with nothing to fall back to. Once
     * the bank IS on flash the node can get back to the previous
     * configuration even if everything after this point fails. */
    {
        char *bj = cJSON_PrintUnformatted(bank);
        nvs_handle_t t;
        bool ok = bj && nvs_open(TRIAL_NS, NVS_READWRITE, &t) == ESP_OK;
        if (ok) {
            ok = nvs_set_str(t, "bank", bj) == ESP_OK &&
                 nvs_set_u32(t, "deadline", deadline) == ESP_OK &&
                 nvs_set_u8(t, "rollback", 1) == ESP_OK &&
                 nvs_set_u8(t, "armed", needs_trial ? 1 : 0) == ESP_OK &&
                 nvs_commit(t) == ESP_OK;
            nvs_close(t);
        }
        free(bj);
        if (!ok) {
            cJSON_Delete(root);
            cJSON_Delete(bank);
            httpd_resp_send_err(req, HTTPD_500_INTERNAL_SERVER_ERROR,
                                "Could not bank the previous config; refusing to apply");
            return ESP_FAIL;
        }
    }

    /* From here the node has a way back, and a change is in flight. Lock the
     * API for the rest of this request: a second request would bank the
     * UNPROVEN settings over the good ones, so a revert would restore
     * settings never shown to work. */
    s_pending_reboot = true;

    /* Pass 5: write the live values. Every one of them was accepted in pass
     * 2, and the recovery bank is already on flash. */
    nvs_handle_t rw;
    if (nvs_open(CFG_NS, NVS_READWRITE, &rw) != ESP_OK) {
        s_pending_reboot = !clear_recovery_bank();
        cJSON_Delete(root);
        cJSON_Delete(bank);
        httpd_resp_send_err(req, HTTPD_500_INTERNAL_SERVER_ERROR, "NVS unavailable");
        return ESP_FAIL;
    }
    int changed = 0;
    cJSON_ArrayForEach(it, root) {
        if (config_is_meta(it->string)) continue;
        const cfg_key_t *k = config_find_key(it->string);
        /* Re-validate instead of carrying the value from pass 2: the rules
         * are cheap and deterministic, and this way the integer that reaches
         * the flash is the one the rules accepted at write time. */
        uint32_t num = 0;
        if (!validate_field(k, it, &num, err, sizeof(err)) ||
            !store_key(rw, k, it, num, err, sizeof(err))) {
            if (!err[0]) snprintf(err, sizeof(err), "could not write %s", k->json);
            break;
        }
        changed++;
    }
    esp_err_t ce = err[0] ? ESP_FAIL : nvs_commit(rw);
    nvs_close(rw);
    if (ce == ESP_OK && !finish_recovery_bank(needs_trial)) ce = ESP_FAIL;

    if (err[0] || ce != ESP_OK) {
        /* A write failed part way through: the node is holding a mix of old
         * and new values. Put the whole bank back rather than leave it half
         * applied, and disarm the trial armed a moment ago. */
        ESP_LOGE(TAG, "config write failed (%s); restoring the previous config", err);
        nvs_handle_t r;
        bool restored = false;
        if (nvs_open(CFG_NS, NVS_READWRITE, &r) == ESP_OK) {
            restored = restore_bank(r, bank) && nvs_commit(r) == ESP_OK;
            nvs_close(r);
        }
        if (restored) restored = clear_recovery_bank();
        /* A failed restore must retain the persisted bank and write lock.
         * The rollback marker makes the next boot retry recovery BEFORE WiFi,
         * instead of promoting a partially written config on association. */
        s_pending_reboot = !restored;
        cJSON_Delete(root);
        cJSON_Delete(bank);
        if (!restored) {
            httpd_resp_send_err(req, HTTPD_500_INTERNAL_SERVER_ERROR,
                                "Write failed and the previous config could not be restored");
            return ESP_FAIL;
        }
        if (ce != ESP_OK && !err[0]) {
            httpd_resp_send_err(req, HTTPD_500_INTERNAL_SERVER_ERROR, "Commit failed");
            return ESP_FAIL;
        }
        httpd_resp_send_err(req, HTTPD_500_INTERNAL_SERVER_ERROR, err);
        return ESP_FAIL;
    }

    /* Push live parameters into the running config so they apply now.
     * Values were range-checked in pass 2, so these are already valid. */
    cJSON *lm = cJSON_GetObjectItem(root, "led_mode");
    if (cJSON_IsNumber(lm)) g_nvs_config.led_mode = (uint8_t)lm->valuedouble;
    cJSON *lb = cJSON_GetObjectItem(root, "led_brightness");
    if (cJSON_IsNumber(lb)) g_nvs_config.led_brightness = (uint8_t)lb->valuedouble;

    cJSON_Delete(root);
    cJSON_Delete(bank);

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
        if (esp_timer_create(&a, &th) == ESP_OK) {
            if (esp_timer_start_once(th, 3000000ULL) != ESP_OK) {
                esp_timer_delete(th);
                ESP_LOGE(TAG, "reboot timer failed; retaining write lock until reset");
            }
        } else {
            /* No reboot means no pending window. Leaving the lock set would
             * refuse every later write for the life of the process. */
            ESP_LOGE(TAG, "could not schedule the reboot; clearing the pending lock");
            /* Keep the bank protected from a second request until reset. */
        }
    } else {
        /* Applied immediately, nothing in flight. */
        s_pending_reboot = false;
    }
    return ESP_OK;
}

/* POST /link/reassoc: drop and re-join the AP so rate control starts fresh.
 * Measured on a 9-node ESP32-C6 fleet: a node whose uplink PHY rate falls to
 * 1-2 Mbps stays there for 24-425 min without help, and that one slow node
 * fills the channel for everyone. A fresh association restored the rate every
 * time (15 of 15 requests that reached the node, 10-09..10-16). Detection is
 * left to the caller (e.g. a service watching the AP's per-client rx rate);
 * the 60 s floor here only stops a runaway caller. The reconnect is the
 * normal STA_DISCONNECTED path in main.c, with its usual backoff. */
static void reassoc_timer_cb(void *arg)
{
    (void)arg;
    esp_err_t r = esp_wifi_disconnect();
    ESP_LOGW(TAG, "link reassoc #%lu: esp_wifi_disconnect -> %s",
             (unsigned long)s_reassoc_count, esp_err_to_name(r));
}

static esp_err_t link_reassoc_handler(httpd_req_t *req)
{
    if (!ota_auth_check(req)) {
        httpd_resp_send_err(req, HTTPD_403_FORBIDDEN,
                            "Authentication required. Use: Authorization: Bearer <psk>");
        return ESP_FAIL;
    }
    int64_t now = esp_timer_get_time();
    httpd_resp_set_type(req, "application/json");
    if (s_reassoc_last_us != 0 && now - s_reassoc_last_us < REASSOC_MIN_INTERVAL_US) {
        httpd_resp_set_status(req, "429 Too Many Requests");
        return httpd_resp_sendstr(req, "{\"error\":\"reassoc requested < 60 s ago\"}");
    }
    if (s_reassoc_timer == NULL) {
        const esp_timer_create_args_t args = { .callback = reassoc_timer_cb, .name = "reassoc" };
        if (esp_timer_create(&args, &s_reassoc_timer) != ESP_OK) {
            return httpd_resp_send_err(req, HTTPD_500_INTERNAL_SERVER_ERROR, "timer create failed");
        }
    }
    s_reassoc_count++;
    s_reassoc_last_us = now;
    /* Reply first; drop the link 200 ms later so the response gets out. */
    esp_err_t sent = httpd_resp_sendstr(req, "{\"status\":\"reassociating\"}");
    esp_timer_stop(s_reassoc_timer);
    esp_timer_start_once(s_reassoc_timer, 200 * 1000);
    return sent;
}

esp_err_t config_api_register(httpd_handle_t server)
{
    static const httpd_uri_t get_uri = {
        .uri = "/config", .method = HTTP_GET, .handler = config_get_handler,
    };
    static const httpd_uri_t post_uri = {
        .uri = "/config", .method = HTTP_POST, .handler = config_post_handler,
    };
    static const httpd_uri_t reassoc_uri = {
        .uri = "/link/reassoc", .method = HTTP_POST, .handler = link_reassoc_handler,
    };
    esp_err_t e = httpd_register_uri_handler(server, &get_uri);
    if (e != ESP_OK) return e;
    e = httpd_register_uri_handler(server, &post_uri);
    if (e != ESP_OK) return e;
    return httpd_register_uri_handler(server, &reassoc_uri);
}
