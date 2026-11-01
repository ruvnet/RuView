/*
 * Host unit test for the adaptive floor guard fixed in this branch
 * (BACKLOG row 127 / PR #1821: "PR #1821 dead-code floor").
 *
 * `edge_processing.c` cannot be linked on host: process_frame() and
 * calibration_update() are `static` inside a ~1500-line translation unit
 * that pulls in FreeRTOS, esp_log, esp_timer, nvs_config, csi_collector,
 * mmwave_sensor, wasm_runtime and stream_sender. None of those are
 * available (or meaningful) outside the target, and stubbing all of them
 * just to reach two static functions is not a proportionate change to a
 * one-line guard fix.
 *
 * Instead this test mirrors the exact two pieces of logic the bug lived
 * in, quoting them verbatim so drift is visible in review:
 *
 *   edge_processing.c:1224 (fixed, this branch)
 *     if (s_cfg.presence_thresh == 0.0f) {
 *         calibration_update(s_motion_energy);
 *     }
 *
 *   edge_processing.c:1224 (BUGGY, pre-fix / contrib/adaptive-floor@9cce83c6)
 *     if (!s_calibrated && s_cfg.presence_thresh == 0.0f) {
 *         calibration_update(s_motion_energy);
 *     }
 *
 *   calibration_update() body, edge_processing.c:559-597 -- reproduced
 *   verbatim below as mirror_calibration_update(), sans the ESP_LOGI
 *   calls (no host logging shim exists or is needed for this test).
 *
 * The constants (EDGE_CALIB_FRAMES, EDGE_FLOOR_LEAK, EDGE_FLOOR_MULT) are
 * NOT duplicated -- they're pulled from the real edge_processing.h, so
 * this test tracks the real tuning if it ever changes.
 *
 * Build + run (from this directory):
 *   make -f Makefile
 *   ./test_edge_calibration_floor
 */

#include <assert.h>
#include <stdio.h>
#include <stdint.h>
#include <stdbool.h>

#include "edge_processing.h"

static int g_pass = 0, g_fail = 0;

#define CHECK(cond, msg) do {                                             \
    if (cond) { g_pass++; }                                               \
    else { g_fail++; printf("  FAIL: %s (line %d)\n", msg, __LINE__); }   \
} while (0)

/* Mirrors s_calibrated / s_floor / s_calib_sum / s_calib_count /
 * s_adaptive_threshold from edge_processing.c:394-399. */
typedef struct {
    bool     calibrated;
    float    floor;
    float    calib_sum;
    uint32_t calib_count;
    float    adaptive_threshold;
} calib_state_t;

static void calib_state_reset(calib_state_t *st)
{
    /* Mirrors edge_processing.c:1479-1483. */
    st->calibrated = false;
    st->calib_sum = 0.0f;
    st->calib_count = 0;
    st->floor = 0.0f;
    st->adaptive_threshold = 0.05f;
}

/* Verbatim port of calibration_update(), edge_processing.c:559-597,
 * minus the two ESP_LOGI calls (no host equivalent, and logging is not
 * part of the property under test). */
static void mirror_calibration_update(calib_state_t *st, float motion)
{
    if (!st->calibrated) {
        st->calib_sum += motion;
        st->calib_count++;
        if (st->calib_count >= EDGE_CALIB_FRAMES) {
            st->floor = st->calib_sum / (float)st->calib_count;
            st->calibrated = true;
        }
        return;
    }

    if (motion < st->floor) {
        st->floor = motion;
    } else {
        st->floor *= EDGE_FLOOR_LEAK;
    }
    if (st->floor < 1e-5f) {
        st->floor = 1e-5f;
    }

    st->adaptive_threshold = st->floor * EDGE_FLOOR_MULT;
    if (st->adaptive_threshold < 0.01f) {
        st->adaptive_threshold = 0.01f;
    }
}

/* The call-site guard is the entire bug. `fixed_guard` selects which
 * variant of edge_processing.c:1224 to run this frame. */
static bool guard_should_update(const calib_state_t *st, float presence_thresh, bool fixed_guard)
{
    if (fixed_guard) {
        /* This branch's edge_processing.c:1224. */
        return presence_thresh == 0.0f;
    }
    /* contrib/adaptive-floor@9cce83c6's edge_processing.c:1226. */
    return !st->calibrated && presence_thresh == 0.0f;
}

/* Drive EDGE_CALIB_FRAMES warm-up frames at `warmup_motion`, then
 * `settle_frames` post-warm-up frames at `settled_motion`, applying the
 * guard given by `fixed_guard` on every frame (matching process_frame()
 * calling calibration_update() once per processed frame). Returns the
 * floor immediately after warm-up via *floor_after_warmup, so callers can
 * compare against the exact seeded value (sum/count of 1200 float32
 * additions of a repeated constant is not bit-exact to the constant
 * itself, so comparing against a literal would be testing float rounding
 * rather than the guard). */
static void run_scenario(calib_state_t *st, bool fixed_guard,
                          float warmup_motion, float settled_motion,
                          uint32_t settle_frames, float *floor_after_warmup)
{
    calib_state_reset(st);

    for (uint32_t i = 0; i < EDGE_CALIB_FRAMES; i++) {
        if (guard_should_update(st, 0.0f, fixed_guard)) {
            mirror_calibration_update(st, warmup_motion);
        }
    }
    CHECK(st->calibrated, "warm-up reaches s_calibrated after EDGE_CALIB_FRAMES");
    if (floor_after_warmup) {
        *floor_after_warmup = st->floor;
    }

    for (uint32_t i = 0; i < settle_frames; i++) {
        if (guard_should_update(st, 0.0f, fixed_guard)) {
            mirror_calibration_update(st, settled_motion);
        }
    }
}

/* --- Positive case: fixed guard, floor descends after warm-up when
 * motion drops. This is the property PR #1821 shipped without. --- */
static void test_fixed_guard_floor_descends(void)
{
    printf("test_fixed_guard_floor_descends:\n");
    calib_state_t st;
    float seeded_floor = 0.0f;

    /* Seed a floor around 0.20 during warm-up (a "noisy room" baseline),
     * then hold at a genuinely quieter 0.02 for a while post-warm-up. */
    run_scenario(&st, /*fixed_guard=*/true, 0.20f, 0.02f, 5000, &seeded_floor);

    CHECK(st.floor < seeded_floor - 1e-6f,
          "fixed guard: floor descends below the warm-up seed once motion drops");
    CHECK(st.floor < 0.05f,
          "fixed guard: floor tracks close to the new quiet level, not frozen at seed");
    printf("  seeded floor ~0.20, settled floor = %.6f\n", (double)st.floor);
}

/* --- Negative control: the OLD (pre-fix) guard on the identical
 * scenario. The floor must stay frozen at the warm-up seed, because
 * `!s_calibrated` makes calibration_update() unreachable forever once
 * s_calibrated flips true -- reproducing the exact dead-code-floor bug
 * this branch fixes. If this assertion ever fails, the mirror above has
 * drifted from the real bug and needs re-checking against
 * edge_processing.c, not "fixed". --- */
static void test_buggy_guard_floor_stays_frozen(void)
{
    printf("test_buggy_guard_floor_stays_frozen (negative control):\n");
    calib_state_t st;
    float seeded_floor = 0.0f;

    run_scenario(&st, /*fixed_guard=*/false, 0.20f, 0.02f, 5000, &seeded_floor);

    /* Under the buggy guard, calibration_update() is unreachable once
     * s_calibrated flips true, so the settle loop must be a complete
     * no-op: the floor after "settling" must be bit-identical to the
     * floor captured immediately after warm-up. */
    CHECK(st.floor == seeded_floor,
          "buggy guard: floor stays latched at the warm-up seed (dead code, as shipped in #1821)");
    printf("  seeded floor ~0.20, buggy-guard \"settled\" floor = %.6f (unchanged)\n", (double)st.floor);
}

int main(void)
{
    test_fixed_guard_floor_descends();
    test_buggy_guard_floor_stays_frozen();

    printf("\n%d passed, %d failed\n", g_pass, g_fail);
    return g_fail == 0 ? 0 : 1;
}
