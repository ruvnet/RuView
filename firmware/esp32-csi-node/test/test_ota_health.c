/**
 * @file test_ota_health.c
 * @brief Pins the OTA first-boot health check decision (ADR-379).
 *
 * Exercises `ota_health_step()` from ../main/ota_health.h directly -- the same
 * function the device build runs, not a copy -- so the test and the firmware
 * cannot drift apart.
 *
 * The regression that motivated this: nothing ever called
 * esp_ota_mark_app_valid_cancel_rollback(), so on rollback-enabled nodes every
 * OTA'd image reverted at the next reset, and a bad image was never caught.
 *
 * Config under test: timeout 120 s, minimum uptime 30 s.
 */

#include <stdio.h>
#include <string.h>

#include "ota_health.h"

#define TIMEOUT_MS 120000u
#define MIN_UP_MS   30000u

static int g_failures = 0;

static const char *name(ota_health_verdict_t v)
{
    switch (v) {
    case OTA_HEALTH_WAIT:       return "WAIT";
    case OTA_HEALTH_MARK_VALID: return "MARK_VALID";
    case OTA_HEALTH_ROLLBACK:   return "ROLLBACK";
    }
    return "?";
}

static void expect(ota_health_verdict_t got, ota_health_verdict_t want,
                   const char *why)
{
    if (got != want) {
        printf("  FAIL: got %s, expected %s\n        %s\n", name(got), name(want), why);
        g_failures++;
    }
}

static void expect_reason(const ota_health_sm_t *sm, const char *needle,
                          const char *why)
{
    if (strstr(sm->reason, needle) == NULL) {
        printf("  FAIL: reason \"%s\" lacks \"%s\"\n        %s\n",
               sm->reason, needle, why);
        g_failures++;
    }
}

static const ota_health_cfg_t CFG = { TIMEOUT_MS, MIN_UP_MS, false };

int main(void)
{
    ota_health_sm_t sm;

    printf("ota health check (timeout %u ms, min uptime %u ms)\n",
           TIMEOUT_MS, MIN_UP_MS);

    /* Healthy boot: IP at 4 s, first CSI frame at 6 s, confirmed at 30 s. */
    ota_health_sm_init(&sm);
    expect(ota_health_step(&sm, &CFG, false, false, 1000), OTA_HEALTH_WAIT,
           "nothing yet, well inside the deadline");
    expect(ota_health_step(&sm, &CFG, true, false, 4000), OTA_HEALTH_WAIT,
           "IP alone is not enough");
    expect(ota_health_step(&sm, &CFG, false, true, 6000), OTA_HEALTH_WAIT,
           "both signals in, but below the minimum uptime");
    expect(ota_health_step(&sm, &CFG, false, false, 29999), OTA_HEALTH_WAIT,
           "one ms short of the minimum uptime");
    expect(ota_health_step(&sm, &CFG, false, false, 30000), OTA_HEALTH_MARK_VALID,
           "latched IP + CSI at the minimum uptime must confirm the image");
    expect_reason(&sm, "IP", "the log names the signals that passed");

    /* Terminal: a verdict never changes, even past the deadline. */
    expect(ota_health_step(&sm, &CFG, false, false, 500000), OTA_HEALTH_MARK_VALID,
           "a confirmed image must not later be rolled back by the same check");

    /* Signals arriving after the minimum uptime confirm immediately. */
    ota_health_sm_init(&sm);
    expect(ota_health_step(&sm, &CFG, true, true, 45000), OTA_HEALTH_MARK_VALID,
           "both signals after min uptime -> confirm now");

    /* No IP by the deadline. */
    ota_health_sm_init(&sm);
    expect(ota_health_step(&sm, &CFG, false, false, 119999), OTA_HEALTH_WAIT,
           "one ms before the deadline");
    expect(ota_health_step(&sm, &CFG, false, false, 120000), OTA_HEALTH_ROLLBACK,
           "no signals at the deadline -> roll back");
    expect_reason(&sm, "no IP and no CSI", "both missing");

    /* IP but the CSI pipeline never delivers. */
    ota_health_sm_init(&sm);
    ota_health_step(&sm, &CFG, true, false, 5000);
    expect(ota_health_step(&sm, &CFG, false, false, 120000), OTA_HEALTH_ROLLBACK,
           "associated but no CSI frame -> roll back");
    expect_reason(&sm, "no CSI frame", "names the missing CSI signal");

    /* CSI sent (can happen only with IP in practice) but no IP event seen. */
    ota_health_sm_init(&sm);
    ota_health_step(&sm, &CFG, false, true, 5000);
    expect(ota_health_step(&sm, &CFG, false, false, 130000), OTA_HEALTH_ROLLBACK,
           "no IP -> roll back");
    expect_reason(&sm, "no IP", "names the missing IP signal");

    /* Rollback is terminal too. */
    expect(ota_health_step(&sm, &CFG, true, true, 131000), OTA_HEALTH_ROLLBACK,
           "late signals must not resurrect a rolled-back verdict");

    /* Force-fail hook rolls back exactly where it would have confirmed. */
    {
        const ota_health_cfg_t ff = { TIMEOUT_MS, MIN_UP_MS, true };
        ota_health_sm_init(&sm);
        expect(ota_health_step(&sm, &ff, true, true, 10000), OTA_HEALTH_WAIT,
               "force-fail still waits for the minimum uptime");
        expect(ota_health_step(&sm, &ff, false, false, 30000), OTA_HEALTH_ROLLBACK,
               "force-fail must roll back instead of confirming");
        expect_reason(&sm, "FORCE_FAIL", "names the test hook");
    }

    /* Minimum uptime above the timeout is clamped, not a guaranteed rollback. */
    {
        const ota_health_cfg_t bad = { 60000u, 600000u, false };
        ota_health_sm_init(&sm);
        expect(ota_health_step(&sm, &bad, true, true, 59999), OTA_HEALTH_WAIT,
               "clamped min uptime: still waiting just before it");
        expect(ota_health_step(&sm, &bad, false, false, 60000), OTA_HEALTH_MARK_VALID,
               "healthy at the deadline must confirm, not roll back");
    }

    /* Zero minimum uptime confirms on the first step with both signals. */
    {
        const ota_health_cfg_t zero = { TIMEOUT_MS, 0u, false };
        ota_health_sm_init(&sm);
        expect(ota_health_step(&sm, &zero, true, true, 0), OTA_HEALTH_MARK_VALID,
               "no soak configured -> confirm immediately");
    }

    if (g_failures) {
        printf("FAILED: %d case(s)\n", g_failures);
        return 1;
    }
    printf("OK: all OTA health cases pass\n");
    return 0;
}
