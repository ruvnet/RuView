/**
 * @file test_edge_subcarrier_grid.c
 * @brief Pins EDGE_MAX_SUBCARRIERS to the radio's actual CSI grid.
 *
 * Regression test for the HE20 truncation bug: `EDGE_MAX_SUBCARRIERS` was a
 * flat 128, an ESP32-S3 assumption. A C6/C5 associated to an HE-capable AP
 * delivers HE20 frames with 256 bins, and `process_frame()` rejects anything
 * larger than the constant:
 *
 *     if (n_subcarriers == 0 || n_subcarriers > EDGE_MAX_SUBCARRIERS) return;
 *
 * so on those parts every frame was dropped and the entire edge pipeline --
 * vitals, presence, fall detection -- silently did nothing while the task
 * logged a healthy banner.
 *
 * WHAT THIS TEST DELIBERATELY DOES NOT DO: it does not re-implement the guard.
 * `fuzz_edge_enqueue.c` carried its own private copy of `EDGE_MAX_SUBCARRIERS`
 * that could drift from the real one without anything failing, which is how a
 * constant this load-bearing went wrong unnoticed. Re-stating the predicate
 * here would recreate exactly that hazard. Instead this pulls the REAL constant
 * from ../main/edge_processing.h and asserts the properties the guard derives
 * from it, so the test and the firmware cannot disagree.
 *
 * Built twice, because the constant is target-conditional and one compilation
 * can only ever prove one branch:
 *
 *     test_edge_grid_he      -Istubs_he (defines the macro)  expects 256
 *     test_edge_grid_pre_he  -Istubs    (macro absent)       expects 128
 *
 * The HE macro is supplied by a stub `sdkconfig.h` and NEVER by -D on the
 * command line. That is the point: `edge_processing.h` must include
 * sdkconfig.h itself. If it does not, C evaluates the undefined identifier in
 * `#if` as 0, silently picks the 128-bin grid on a 256-bin part, and the edge
 * pipeline goes quiet exactly as it did before this fix -- with no warning and
 * no failing build. Defining the macro on the command line would compile the
 * right branch regardless and mask that dependency completely.
 */

#include <stdint.h>
#include <stdio.h>

#include "edge_processing.h"

static int g_failures = 0;

#define CHECK(cond, ...)                                                       \
    do {                                                                       \
        if (!(cond)) {                                                         \
            printf("  FAIL: " __VA_ARGS__);                                    \
            printf("\n        (%s, line %d)\n", #cond, __LINE__);              \
            g_failures++;                                                      \
        }                                                                      \
    } while (0)

/** Bins an HE20 frame carries on an HE-capable ESP32 (C6/C5). */
#define HE20_SUBCARRIERS      256

/** Bins a pre-HE part (S3 etc) reports at most. */
#define PRE_HE_SUBCARRIERS    128

/** Bytes per subcarrier on the wire: one int8 I and one int8 Q. */
#define BYTES_PER_SUBCARRIER  2

/* EXPECT_HE is a TEST-ONLY marker set by the Makefile, deliberately spelled
 * differently from the IDF macro. It records what the build INTENDS; the
 * assertions below then check what the header actually CONCLUDED. Deciding the
 * expectation with `#if CONFIG_SOC_WIFI_HE_SUPPORT` would be circular: when the
 * header fails to pull in sdkconfig.h, the macro is invisible to the test too,
 * so it would quietly assert the pre-HE case and pass while the bug was live.
 * That exact mistake was made writing this test and caught by running it
 * against the broken header first. */
#ifndef EXPECT_HE
#define EXPECT_HE 0
#endif

int main(void)
{
#if EXPECT_HE
    const char *build = "HE-capable (C6/C5)";
    const unsigned expect = HE20_SUBCARRIERS;
#else
    const char *build = "pre-HE (S3)";
    const unsigned expect = PRE_HE_SUBCARRIERS;
#endif

    printf("edge subcarrier grid: %s build\n", build);
    printf("  EDGE_MAX_SUBCARRIERS = %u (expect %u)\n",
           (unsigned)EDGE_MAX_SUBCARRIERS, expect);

#if EXPECT_HE
    /* 0. The header must reach the SoC capability macro on its own. sdkconfig.h
     *    defines it; if edge_processing.h does not include sdkconfig.h, C reads
     *    the undefined identifier in `#if` as 0 and silently selects the pre-HE
     *    grid on a 256-bin part -- no error, no warning, no failing build, and
     *    a dead edge pipeline. This is the assertion that catches a missing
     *    `#include "sdkconfig.h"`. */
#ifndef CONFIG_SOC_WIFI_HE_SUPPORT
    CHECK(0, "CONFIG_SOC_WIFI_HE_SUPPORT is not visible after including "
             "edge_processing.h -- the header is not including sdkconfig.h, so "
             "the subcarrier grid silently falls back to the pre-HE size");
#endif
#endif

    /* 1. The constant matches the radio this build targets. This is the
     *    assertion that would have failed before the fix on a C6. */
    CHECK((unsigned)EDGE_MAX_SUBCARRIERS == expect,
          "EDGE_MAX_SUBCARRIERS is %u, expected %u for a %s build",
          (unsigned)EDGE_MAX_SUBCARRIERS, expect, build);

#if EXPECT_HE
    /* 2. An HE20 frame survives the guard rather than being discarded. The
     *    guard rejects `n_subcarriers > EDGE_MAX_SUBCARRIERS`, so accepting a
     *    256-bin frame is exactly `256 <= EDGE_MAX_SUBCARRIERS`. */
    CHECK(HE20_SUBCARRIERS <= EDGE_MAX_SUBCARRIERS,
          "a %u-bin HE20 frame is rejected by the guard on an HE part",
          (unsigned)HE20_SUBCARRIERS);
#else
    /* 2'. Pre-HE parts keep the smaller grid. Sizing these buffers for 256 on
     *     an S3 would spend ~3.5 KB of .bss the part can never fill. */
    CHECK((unsigned)EDGE_MAX_SUBCARRIERS == PRE_HE_SUBCARRIERS,
          "pre-HE grid changed from %u; that is a .bss regression on S3",
          (unsigned)PRE_HE_SUBCARRIERS);
#endif

    /* 3. A full-width frame still fits one ring slot. Raising the subcarrier
     *    grid without the I/Q budget to carry it would move the truncation
     *    from process_frame() into ring_push()'s memcpy clamp, which is
     *    quieter still -- the frame would arrive, be silently shortened, and
     *    be processed as though complete. */
    const unsigned widest_iq_bytes =
        (unsigned)EDGE_MAX_SUBCARRIERS * BYTES_PER_SUBCARRIER;
    printf("  widest frame = %u bytes of I/Q (EDGE_MAX_IQ_BYTES = %u)\n",
           widest_iq_bytes, (unsigned)EDGE_MAX_IQ_BYTES);
    CHECK(widest_iq_bytes <= (unsigned)EDGE_MAX_IQ_BYTES,
          "a full-width frame needs %u B but a ring slot holds %u B",
          widest_iq_bytes, (unsigned)EDGE_MAX_IQ_BYTES);

    /* 4. Top-K selection has to fit inside the grid it selects from. */
    CHECK((unsigned)EDGE_TOP_K <= (unsigned)EDGE_MAX_SUBCARRIERS,
          "EDGE_TOP_K (%u) exceeds the subcarrier grid (%u)",
          (unsigned)EDGE_TOP_K, (unsigned)EDGE_MAX_SUBCARRIERS);

    if (g_failures == 0) {
        printf("  PASS (%s)\n", build);
        return 0;
    }
    printf("  %d check(s) FAILED\n", g_failures);
    return 1;
}
