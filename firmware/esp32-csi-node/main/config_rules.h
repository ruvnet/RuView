/**
 * @file config_rules.h
 * @brief The writable configuration surface and the rules that accept it.
 *
 * Deliberately free of ESP-IDF and of cJSON: the table and the validation
 * live here so that test/test_config_rules.c can pin them on the host,
 * without a device. The HTTP handler decodes a request into a cfg_value_t and
 * asks these rules whether the value is acceptable -- it never decides on its
 * own, and it never writes a value these rules have not accepted.
 */
#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "config_limits.h"

typedef enum { T_STR, T_U8, T_U16, T_U32 } cfg_type_t;

/**
 * The writable surface.
 *
 * `nvs` MUST match the key nvs_config_load() reads, and nothing enforces that
 * match at compile time -- a typo here writes a key nobody loads. The GET
 * handler exists partly as the check: it reads back through the same keys, so
 * a setting that did not take is visible immediately rather than silently
 * ignored.
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

/** One decoded request value, so the rules never have to know about cJSON. */
typedef enum { CFG_VAL_NONE, CFG_VAL_STR, CFG_VAL_NUM } cfg_val_kind_t;

typedef struct {
    cfg_val_kind_t kind;
    const char    *str;   /**< CFG_VAL_STR: the string as it arrived. */
    double         num;   /**< CFG_VAL_NUM: the number as it arrived. */
} cfg_value_t;

/** The writable surface, in GET order. */
extern const cfg_key_t CONFIG_KEYS[];
extern const size_t   CONFIG_KEY_COUNT;

/** The key for a JSON field name, or NULL when the name is not writable. */
const cfg_key_t *config_find_key(const char *json);

/**
 * Accept or reject one value.
 *
 * Returns false with `err` set on the first problem. Nothing is written and
 * nothing is committed: the caller must validate EVERY field of a request
 * before it writes ANY of them, because a rejected request has to leave the
 * node exactly as it was.
 *
 * On success `scaled` receives the integer to store (the value multiplied by
 * the key's scale and rounded); it is unused for T_STR.
 */
bool config_validate_value(const cfg_key_t *k, const cfg_value_t *v,
                            uint32_t *scaled, char *err, size_t errlen);

/**
 * True when a request may write.
 *
 * False while a boot trial is pending (the trial may still revert) and false
 * while a change is in flight (a second change would bank the UNPROVEN values
 * as the fallback, so a revert would restore settings never shown to work).
 */
bool config_write_allowed(bool trial_armed, bool pending_reboot);

/** True for the parameters the running firmware re-reads every cycle. */
bool config_is_live(const char *name);

/** True for the body fields that control the request rather than a parameter. */
bool config_is_meta(const char *name);
