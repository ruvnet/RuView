/**
 * @file config_rules.c
 * @brief The writable configuration surface and the rules that accept it.
 *
 * See config_rules.h. No ESP-IDF, no cJSON: this file is compiled on the host
 * by test/test_config_rules.c as well as on the device.
 */

#include "config_rules.h"

#include <math.h>
#include <stdio.h>
#include <string.h>

const cfg_key_t CONFIG_KEYS[] = {
    /* json                 nvs             type    lo    hi     len  scale  assoc */
    { "wifi_ssid",          "ssid",         T_STR,   0,     0,    32,   0,   true  },
    { "wifi_password",      "password",     T_STR,   0,     0,    64,   0,   true  },
    { "channel_hop_count",  "hop_count",    T_U8,    CFG_LIMIT_HOP_COUNT_MIN,
                                                        CFG_LIMIT_HOP_COUNT_MAX,
                                                        0,   0,   true  },
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
    { "vital_window",       "vital_win",    T_U16,   CFG_LIMIT_VITAL_WINDOW_MIN,
                                                        CFG_LIMIT_VITAL_WINDOW_MAX,
                                                        0,   0,   false },
    { "vital_interval_ms",  "vital_int",    T_U16, 100, 60000,     0,   0,   false },
    { "top_k_count",        "subk_count",   T_U8,    CFG_LIMIT_TOP_K_MIN,
                                                        CFG_LIMIT_TOP_K_MAX,
                                                        0,   0,   false },
    { "power_duty",         "power_duty",   T_U8,   10,   100,     0,   0,   false },
    { "zone_name",          "zone_name",    T_STR,   0,     0,    16,   0,   false },
    { "swarm_heartbeat_sec","swarm_hb",     T_U16,   1, 65535,     0,   0,   false },
    { "swarm_ingest_sec",   "swarm_ingest", T_U16,   1, 65535,     0,   0,   false },
    { "led_mode",           "led_mode",     T_U8,    0,     2,     0,   0,   false },
    { "led_brightness",     "led_bright",   T_U8,    0,   100,     0,   0,   false },
};

const size_t CONFIG_KEY_COUNT = sizeof(CONFIG_KEYS) / sizeof(CONFIG_KEYS[0]);

const cfg_key_t *config_find_key(const char *json)
{
    if (!json) return NULL;
    for (size_t i = 0; i < CONFIG_KEY_COUNT; i++) {
        if (strcmp(CONFIG_KEYS[i].json, json) == 0) return &CONFIG_KEYS[i];
    }
    return NULL;
}

bool config_validate_value(const cfg_key_t *k, const cfg_value_t *v,
                            uint32_t *scaled, char *err, size_t errlen)
{
    if (k->type == T_STR) {
        if (v->kind != CFG_VAL_STR || v->str == NULL) {
            snprintf(err, errlen, "%s must be a string", k->json);
            return false;
        }
        if (strlen(v->str) >= k->maxlen) {
            snprintf(err, errlen, "%s exceeds %u chars", k->json,
                     (unsigned)(k->maxlen - 1));
            return false;
        }
        return true;
    }

    if (v->kind != CFG_VAL_NUM) {
        snprintf(err, errlen, "%s must be a number", k->json);
        return false;
    }

    double d = v->num;
    if (k->scale > 0) d *= k->scale;

    /* Reject a value that is not a number at all -- NaN, or an infinity the
     * scale just overflowed into. The negative clamp below would turn -inf
     * into a perfectly valid 0, and the old code cast before it range-checked,
     * so 1e300 became an arbitrary integer. */
    if (!isfinite(d)) {
        snprintf(err, errlen, "%s is not a finite number", k->json);
        return false;
    }

    if (d < 0) d = 0;                        /* negatives clamp to 0 */

    /* Reject BEFORE the cast. A NaN, an infinity or a number past the
     * accepted window must never be truncated into a value that looks valid:
     * the old code cast first, so 1e300 became an arbitrary integer. The
     * half-step tolerance keeps the rounding of the old path: a value that
     * rounds inside the window is accepted, one that rounds outside is not. */
    if (!(d >= (double)k->lo - 0.5 && d <= (double)k->hi + 0.5)) {
        snprintf(err, errlen, "%s out of range %lu..%lu", k->json,
                 (unsigned long)k->lo, (unsigned long)k->hi);
        return false;
    }

    uint32_t val = (uint32_t)(d + 0.5);
    if (val < k->lo || val > k->hi) {
        snprintf(err, errlen, "%s out of range %lu..%lu", k->json,
                 (unsigned long)k->lo, (unsigned long)k->hi);
        return false;
    }

    if (scaled) *scaled = val;
    return true;
}

bool config_write_allowed(bool trial_armed, bool pending_reboot)
{
    return !trial_armed && !pending_reboot;
}

bool config_is_live(const char *name)
{
    return strcmp(name, "led_mode") == 0 ||
           strcmp(name, "led_brightness") == 0;
}

bool config_is_meta(const char *name)
{
    return strcmp(name, "trial_seconds") == 0 ||
           strcmp(name, "reboot") == 0;
}
