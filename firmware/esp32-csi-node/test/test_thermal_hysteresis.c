/**
 * @file test_thermal_hysteresis.c
 * @brief Pins every thermal transition, in both directions.
 *
 * Exercises `thermal_next_state()` from ../main/thermal.h directly -- the same
 * function the device build runs, not a copy -- so the test and the firmware
 * cannot drift apart.
 *
 * The regression that motivated this: the CRITICAL -> THROTTLED edge was
 * unconditional. Any reading below `critical_c` dropped the state at once, so a
 * node sitting on the critical boundary flapped between minimum and step-2
 * transmit power forever. Every oscillation moves the RSSI of every link that
 * node has, at both ends, which a reader cannot tell apart from an obstruction
 * -- exactly the failure the hysteresis on the other edges exists to prevent.
 *
 * Defaults under test: warn 65, throttle 72, critical 80, hysteresis 8.
 */

#include <stdio.h>

#include "thermal.h"

#define WARN_C      65
#define THROTTLE_C  72
#define CRITICAL_C  80
#define HYST_C       8

static int g_failures = 0;

static const char *name(thermal_state_t s)
{
    switch (s) {
    case THERMAL_OK:        return "OK";
    case THERMAL_WARN:      return "WARN";
    case THERMAL_THROTTLED: return "THROTTLED";
    case THERMAL_CRITICAL:  return "CRITICAL";
    }
    return "?";
}

static thermal_state_t step(thermal_state_t cur, float c)
{
    return thermal_next_state(cur, c, WARN_C, THROTTLE_C, CRITICAL_C, HYST_C);
}

static void expect(thermal_state_t cur, float c, thermal_state_t want,
                   const char *why)
{
    thermal_state_t got = step(cur, c);
    if (got != want) {
        printf("  FAIL: from %-9s at %5.1f C -> %s, expected %s\n        %s\n",
               name(cur), c, name(got), name(want), why);
        g_failures++;
    }
}

int main(void)
{
    printf("thermal hysteresis (warn %d, throttle %d, critical %d, hyst %d)\n",
           WARN_C, THROTTLE_C, CRITICAL_C, HYST_C);

    /* ---- Rising edges are immediate. Heat is a reason to act now. ---- */
    expect(THERMAL_OK,        64.9f, THERMAL_OK,        "below warn");
    expect(THERMAL_OK,        65.0f, THERMAL_WARN,      "warn trips at the threshold");
    expect(THERMAL_WARN,      72.0f, THERMAL_THROTTLED, "throttle trips at the threshold");
    expect(THERMAL_THROTTLED, 80.0f, THERMAL_CRITICAL,  "critical trips at the threshold");
    /* A jump straight past intermediate bands must not be damped on the way up. */
    expect(THERMAL_OK,        95.0f, THERMAL_CRITICAL,  "a large rise skips straight to critical");

    /* ---- THE REGRESSION: falling out of CRITICAL. ----
     *
     * Before the fix, 79.9 C dropped the state to THROTTLED immediately, so a
     * node hovering at 80 alternated TXP_MIN and TXP_STEP2 indefinitely. */
    expect(THERMAL_CRITICAL, 79.9f, THERMAL_CRITICAL,
           "0.1 C below critical must NOT step down -- this is the flap");
    expect(THERMAL_CRITICAL, 75.0f, THERMAL_CRITICAL,
           "still inside the hysteresis band");
    expect(THERMAL_CRITICAL, 72.1f, THERMAL_CRITICAL,
           "just inside the band (critical 80 - hyst 8 = 72)");
    /* Once it has cooled a full band it adopts the band it is ACTUALLY in,
     * which under these defaults is WARN, not THROTTLED: the critical exit
     * point (80 - 8 = 72) coincides exactly with the throttle threshold, so
     * anything cool enough to leave CRITICAL is already below THROTTLED's
     * entry. THROTTLED is therefore unreachable descending from CRITICAL at
     * hysteresis 8 -- a consequence of the chosen constants, not of the logic.
     * With a smaller hysteresis the intermediate step reappears, which the
     * next assertion pins. */
    expect(THERMAL_CRITICAL, 71.9f, THERMAL_WARN,
           "a full band below critical, and 71.9 is in the warn band");

    /* Same descent with hysteresis 4: the exit point is 76, which lands inside
     * THROTTLED's band, so the intermediate power step is used. */
    {
        thermal_state_t got = thermal_next_state(THERMAL_CRITICAL, 75.9f,
                                                 WARN_C, THROTTLE_C, CRITICAL_C, 4);
        if (got != THERMAL_THROTTLED) {
            printf("  FAIL: from CRITICAL at 75.9 C with hyst 4 -> %s, "
                   "expected THROTTLED\n", name(got));
            g_failures++;
        }
        thermal_state_t held = thermal_next_state(THERMAL_CRITICAL, 76.1f,
                                                  WARN_C, THROTTLE_C, CRITICAL_C, 4);
        if (held != THERMAL_CRITICAL) {
            printf("  FAIL: from CRITICAL at 76.1 C with hyst 4 -> %s, "
                   "expected CRITICAL\n", name(held));
            g_failures++;
        }
    }

    /* Prove the flap is gone by walking across the boundary repeatedly: a node
     * dithering either side of 80 must not change state on the down-swings. */
    {
        thermal_state_t s = THERMAL_CRITICAL;
        int changes = 0;
        const float dither[] = {80.2f, 79.8f, 80.1f, 79.7f, 80.3f, 79.9f};
        for (unsigned i = 0; i < sizeof(dither) / sizeof(dither[0]); i++) {
            thermal_state_t n = step(s, dither[i]);
            if (n != s) changes++;
            s = n;
        }
        if (changes != 0) {
            printf("  FAIL: dithering around the critical boundary produced %d "
                   "state change(s); each one steps transmit power\n", changes);
            g_failures++;
        }
    }

    /* ---- Falling out of THROTTLED. Behaviour preserved from before. ---- */
    expect(THERMAL_THROTTLED, 71.9f, THERMAL_THROTTLED, "just below the throttle point holds");
    expect(THERMAL_THROTTLED, 64.1f, THERMAL_THROTTLED, "still inside the band");
    expect(THERMAL_THROTTLED, 63.9f, THERMAL_OK,        "throttle 72 - hyst 8 = 64 releases");

    /* ---- Falling out of WARN. ---- */
    expect(THERMAL_WARN, 64.9f, THERMAL_WARN, "just below warn holds");
    expect(THERMAL_WARN, 57.1f, THERMAL_WARN, "still inside the band");
    expect(THERMAL_WARN, 56.9f, THERMAL_OK,   "warn 65 - hyst 8 = 57 releases");

    /* ---- OK is stable well below everything. ---- */
    expect(THERMAL_OK, 30.0f, THERMAL_OK, "a cold node stays OK");

    if (g_failures == 0) {
        printf("  PASS\n");
        return 0;
    }
    printf("  %d check(s) FAILED\n", g_failures);
    return 1;
}
