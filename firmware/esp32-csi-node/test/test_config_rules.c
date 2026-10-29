/**
 * Host-side test for the configuration rules.
 *
 * Pins the behaviour the HTTP configuration API depends on, without a device:
 *
 *   - the API table and the boot loader agree on the accepted ranges, so a
 *     value the API accepts is a value the loader will actually apply;
 *   - a value outside the range is rejected BEFORE anything is written, so a
 *     rejected request leaves the node exactly as it was;
 *   - a second request is refused while a change is in flight or a trial is
 *     pending, so a revert can never restore settings that were never shown
 *     to work.
 *
 * The rules themselves live in ../main/config_rules.c, which is the same
 * translation unit the firmware links -- so this test and the device cannot
 * disagree.
 *
 * Build and run:
 *   make run_config_rules
 */

#include <stdio.h>
#include <string.h>
#include <math.h>

#include "config_rules.h"

static int failures;

#define CHECK(cond, ...)                                                   \
    do {                                                                   \
        if (!(cond)) {                                                     \
            failures++;                                                    \
            printf("FAIL %s:%d: ", __FILE__, __LINE__);                    \
            printf(__VA_ARGS__);                                           \
            printf("\n");                                                  \
        }                                                                  \
    } while (0)

/** Accept a numeric value for a key; returns the integer that would be stored. */
static bool accept_num(const char *json, double num, uint32_t *out)
{
    const cfg_key_t *k = config_find_key(json);
    if (!k) {
        failures++;
        printf("FAIL unknown key '%s'\n", json);
        return false;
    }
    char err[96] = {0};
    cfg_value_t v = { .kind = CFG_VAL_NUM, .str = NULL, .num = num };
    uint32_t scaled = 0;
    if (!config_validate_value(k, &v, &scaled, err, sizeof(err))) {
        failures++;
        printf("FAIL %s=%g was rejected: %s\n", json, num, err);
        return false;
    }
    if (out) *out = scaled;
    return true;
}

static bool reject_num(const char *json, double num)
{
    const cfg_key_t *k = config_find_key(json);
    if (!k) {
        failures++;
        printf("FAIL unknown key '%s'\n", json);
        return false;
    }
    char err[96] = {0};
    cfg_value_t v = { .kind = CFG_VAL_NUM, .str = NULL, .num = num };
    uint32_t scaled = 0;
    if (config_validate_value(k, &v, &scaled, err, sizeof(err))) {
        failures++;
        printf("FAIL %s=%g was accepted but must be rejected\n", json, num);
        return false;
    }
    return true;
}

/* ------------------------------------------------------------------ table --- */

/**
 * The API table must carry the shared limits verbatim.
 *
 * This is the guard against the drift that made the API accept values the
 * loader silently ignored.
 */
static void test_table_matches_shared_limits(void)
{
    const cfg_key_t *k;

    k = config_find_key("vital_window");
    CHECK(k != NULL, "vital_window must be writable");
    if (k) {
        CHECK(k->lo == CFG_LIMIT_VITAL_WINDOW_MIN,
              "vital_window min is %lu, expected %u",
              (unsigned long)k->lo, CFG_LIMIT_VITAL_WINDOW_MIN);
        CHECK(k->hi == CFG_LIMIT_VITAL_WINDOW_MAX,
              "vital_window max is %lu, expected %u",
              (unsigned long)k->hi, CFG_LIMIT_VITAL_WINDOW_MAX);
    }

    k = config_find_key("top_k_count");
    CHECK(k != NULL, "top_k_count must be writable");
    if (k) {
        CHECK(k->lo == CFG_LIMIT_TOP_K_MIN,
              "top_k_count min is %lu, expected %u",
              (unsigned long)k->lo, CFG_LIMIT_TOP_K_MIN);
        CHECK(k->hi == CFG_LIMIT_TOP_K_MAX,
              "top_k_count max is %lu, expected %u",
              (unsigned long)k->hi, CFG_LIMIT_TOP_K_MAX);
    }

    k = config_find_key("channel_hop_count");
    CHECK(k != NULL, "channel_hop_count must be writable");
    if (k) {
        CHECK(k->lo == CFG_LIMIT_HOP_COUNT_MIN,
              "channel_hop_count min is %lu, expected %u",
              (unsigned long)k->lo, CFG_LIMIT_HOP_COUNT_MIN);
        CHECK(k->hi == CFG_LIMIT_HOP_COUNT_MAX,
              "channel_hop_count max is %lu, expected %u",
              (unsigned long)k->hi, CFG_LIMIT_HOP_COUNT_MAX);
    }
}

/* -------------------------------------------------------------- validation --- */

/**
 * The boundaries the loader accepts.
 *
 * A value just outside the range must be rejected, so it never reaches NVS to
 * be stored, reported as applied and then ignored at boot.
 */
static void test_range_boundaries(void)
{
    /* vital_window: the loader accepts 32..256. */
    accept_num("vital_window", 32, NULL);
    accept_num("vital_window", 256, NULL);
    reject_num("vital_window", 31);
    reject_num("vital_window", 257);
    reject_num("vital_window", 4096);

    /* top_k_count: the loader accepts 1..32. */
    accept_num("top_k_count", 1, NULL);
    accept_num("top_k_count", 32, NULL);
    reject_num("top_k_count", 0);
    reject_num("top_k_count", 33);
    reject_num("top_k_count", 64);

    /* channel_hop_count: the channel list holds 6 entries. */
    accept_num("channel_hop_count", 1, NULL);
    accept_num("channel_hop_count", 6, NULL);
    reject_num("channel_hop_count", 0);
    reject_num("channel_hop_count", 7);
    reject_num("channel_hop_count", 14);
}

/**
 * A number that cannot be represented must be rejected, not truncated into a
 * value that looks valid. The old code cast before it range-checked, so a
 * huge number became an arbitrary integer.
 */
static void test_rejects_unrepresentable_numbers(void)
{
    reject_num("node_id", 1e300);
    reject_num("node_id", INFINITY);
    reject_num("node_id", -INFINITY);
    reject_num("node_id", NAN);
    reject_num("vital_window", 1e300);
    reject_num("top_k_count", 1e300);
}

/** A field of the wrong type is rejected. */
static void test_rejects_wrong_type(void)
{
    const cfg_key_t *k = config_find_key("wifi_ssid");
    CHECK(k != NULL, "wifi_ssid must be writable");
    if (!k) return;

    char err[96] = {0};
    uint32_t scaled = 0;
    cfg_value_t num = { .kind = CFG_VAL_NUM, .str = NULL, .num = 5 };
    CHECK(!config_validate_value(k, &num, &scaled, err, sizeof(err)),
          "a number must not be accepted for a string field");

    cfg_value_t str = { .kind = CFG_VAL_STR, .str = "x", .num = 0 };
    k = config_find_key("vital_window");
    CHECK(!config_validate_value(k, &str, &scaled, err, sizeof(err)),
          "a string must not be accepted for a numeric field");
}

/** A string longer than the field can hold is rejected. */
static void test_rejects_overlong_string(void)
{
    const cfg_key_t *k = config_find_key("wifi_ssid");
    CHECK(k != NULL, "wifi_ssid must be writable");
    if (!k) return;

    char long_ssid[64];
    memset(long_ssid, 'a', sizeof(long_ssid) - 1);
    long_ssid[sizeof(long_ssid) - 1] = '\0';

    char err[96] = {0};
    uint32_t scaled = 0;
    cfg_value_t v = { .kind = CFG_VAL_STR, .str = long_ssid, .num = 0 };
    CHECK(!config_validate_value(k, &v, &scaled, err, sizeof(err)),
          "an over-long SSID must be rejected");
}

/** The scaled fields store the value multiplied by the fixed-point scale. */
static void test_scaled_fields(void)
{
    uint32_t stored = 0;
    /* presence_thresh is stored as value*1000. */
    accept_num("presence_thresh", 1.5, &stored);
    CHECK(stored == 1500, "presence_thresh 1.5 must store 1500, got %lu",
          (unsigned long)stored);

    /* 65.535 is the largest value that fits the u16 the loader reads. */
    accept_num("presence_thresh", 65.535, &stored);
    CHECK(stored == 65535, "presence_thresh 65.535 must store 65535, got %lu",
          (unsigned long)stored);
    reject_num("presence_thresh", 65.536);
    reject_num("presence_thresh", 1000.0);
}

/* ------------------------------------------------------------------ gating --- */

/**
 * A write is allowed only when no trial is pending and no change is in
 * flight. The second case is the one that let a second request bank the
 * first request's unproven settings.
 */
static void test_write_gating(void)
{
    CHECK(config_write_allowed(false, false), "a quiet node must accept writes");
    CHECK(!config_write_allowed(true, false),
          "a pending trial must refuse writes");
    CHECK(!config_write_allowed(false, true),
          "a change in flight must refuse writes");
    CHECK(!config_write_allowed(true, true),
          "a pending trial and a change in flight must refuse writes");
}

/** The classification of the request-control fields and the live fields. */
static void test_field_classification(void)
{
    CHECK(config_is_meta("reboot"), "reboot is a request-control field");
    CHECK(config_is_meta("trial_seconds"), "trial_seconds is a request-control field");
    CHECK(!config_is_meta("vital_window"), "vital_window is a parameter");

    CHECK(config_is_live("led_mode"), "led_mode is applied immediately");
    CHECK(config_is_live("led_brightness"), "led_brightness is applied immediately");
    CHECK(!config_is_live("vital_window"), "vital_window is read at boot only");

    /* The keys that can orphan a node are the ones that go through a trial. */
    CHECK(config_find_key("wifi_ssid")->assoc, "wifi_ssid must require a trial");
    CHECK(config_find_key("wifi_password")->assoc, "wifi_password must require a trial");
    CHECK(!config_find_key("vital_window")->assoc,
          "vital_window must not require a trial");
    CHECK(!config_find_key("led_mode")->assoc, "led_mode must not require a trial");
}

int main(void)
{
    test_table_matches_shared_limits();
    test_range_boundaries();
    test_rejects_unrepresentable_numbers();
    test_rejects_wrong_type();
    test_rejects_overlong_string();
    test_scaled_fields();
    test_write_gating();
    test_field_classification();

    if (failures) {
        printf("config rules: %d check(s) FAILED\n", failures);
        return 1;
    }
    printf("config rules: all checks passed\n");
    return 0;
}
