/*
 * Host unit test for rv_feature_state_* helpers.
 *
 * Validates:
 *   - Packet layout is exactly 80 bytes
 *   - IEEE CRC32 matches well-known reference vectors
 *   - finalize() populates magic/seq/ts/crc correctly
 *   - CRC32 throughput benchmark
 */

#include <assert.h>
#include <stdio.h>
#include <string.h>
#include <time.h>

#include "rv_feature_state.h"
#include "rv_radio_ops.h"

static int g_pass = 0, g_fail = 0;
#define CHECK(cond, msg) do {                                   \
    if (cond) { g_pass++; }                                     \
    else { g_fail++; printf("  FAIL: %s (line %d)\n", msg, __LINE__); } \
} while (0)

static void test_packet_size(void) {
    printf("test: rv_feature_state_t is 60 bytes on the wire\n");
    CHECK(sizeof(rv_feature_state_t) == 60, "sizeof == 60");
}

static void test_crc_known_vectors(void) {
    printf("test: IEEE CRC32 known vectors\n");
    /* IEEE CRC32 of "123456789" == 0xCBF43926 (well-known). */
    uint32_t c1 = rv_feature_state_crc32((const uint8_t *)"123456789", 9);
    CHECK(c1 == 0xCBF43926u, "CRC32('123456789') == 0xCBF43926");

    /* Empty input → 0x00000000 (before final inversion, 0xFFFFFFFF);
     * IEEE convention with post-invert → 0x00000000 reversed — but with
     * our implementation the empty-input CRC is 0x00000000 after post-
     * invert on ~0xFFFFFFFF = 0x00000000. */
    uint32_t c2 = rv_feature_state_crc32(NULL, 0);
    CHECK(c2 == 0x00000000u, "CRC32(empty) == 0");

    /* Single zero byte: IEEE CRC32 of 0x00 = 0xD202EF8D. */
    uint8_t zero = 0;
    uint32_t c3 = rv_feature_state_crc32(&zero, 1);
    CHECK(c3 == 0xD202EF8Du, "CRC32(0x00) == 0xD202EF8D");
}

static void test_finalize(void) {
    printf("test: finalize populates required fields\n");
    rv_feature_state_t pkt;
    memset(&pkt, 0, sizeof(pkt));
    pkt.motion_score    = 0.25f;
    pkt.presence_score  = 0.75f;
    pkt.respiration_bpm = 14.5f;
    pkt.quality_flags   = RV_QFLAG_PRESENCE_VALID | RV_QFLAG_RESPIRATION_VALID;

    rv_feature_state_finalize(&pkt, /*node*/ 7, /*seq*/ 42,
                              /*ts*/ 1234567ULL, RV_PROFILE_RESP_HIGH_SENS);

    CHECK(pkt.magic == RV_FEATURE_STATE_MAGIC, "magic");
    CHECK(pkt.node_id == 7, "node_id");
    CHECK(pkt.seq == 42, "seq");
    CHECK(pkt.ts_us == 1234567ULL, "ts_us");
    CHECK(pkt.mode == RV_PROFILE_RESP_HIGH_SENS, "mode");
    CHECK(pkt.reserved == 0, "reserved cleared");
    CHECK(pkt.crc32 != 0, "crc32 populated (non-trivial input)");

    /* Re-finalize must produce identical CRC (deterministic). */
    uint32_t crc1 = pkt.crc32;
    rv_feature_state_finalize(&pkt, 7, 42, 1234567ULL, RV_PROFILE_RESP_HIGH_SENS);
    CHECK(pkt.crc32 == crc1, "finalize is deterministic");

    /* Changing a payload byte must change the CRC. */
    pkt.motion_score = 0.26f;
    rv_feature_state_finalize(&pkt, 7, 42, 1234567ULL, RV_PROFILE_RESP_HIGH_SENS);
    CHECK(pkt.crc32 != crc1, "CRC changes when payload changes");
}

static void test_crc_verifiability(void) {
    printf("test: receiver can verify CRC\n");
    rv_feature_state_t pkt;
    memset(&pkt, 0, sizeof(pkt));
    pkt.motion_score   = 0.33f;
    pkt.presence_score = 0.66f;
    rv_feature_state_finalize(&pkt, 1, 100, 555ULL, RV_PROFILE_PASSIVE_LOW_RATE);

    /* Receiver recomputes CRC over all bytes except the trailing crc32. */
    uint32_t expected = rv_feature_state_crc32(
        (const uint8_t *)&pkt, sizeof(pkt) - sizeof(uint32_t));
    CHECK(pkt.crc32 == expected, "receiver-side CRC check matches");
}

static void benchmark_crc(void) {
    printf("bench: CRC32 over 60-byte packet (56 B hashed, excl trailing crc32)\n");
    rv_feature_state_t pkt;
    memset(&pkt, 0x5A, sizeof(pkt));

    const int N = 5000000;
    struct timespec a, b;
    clock_gettime(CLOCK_MONOTONIC, &a);
    volatile uint32_t sink = 0;
    for (int i = 0; i < N; i++) {
        pkt.seq = (uint16_t)i;  /* vary input so compiler can't fold */
        sink ^= rv_feature_state_crc32(
            (const uint8_t *)&pkt, sizeof(pkt) - sizeof(uint32_t));
    }
    clock_gettime(CLOCK_MONOTONIC, &b);
    (void)sink;
    double ns_per_call = ((b.tv_sec - a.tv_sec) * 1e9 +
                          (b.tv_nsec - a.tv_nsec)) / (double)N;
    double mb_per_sec = (double)(sizeof(pkt) - sizeof(uint32_t)) / ns_per_call
                        * 1e9 / (1024.0 * 1024.0);
    printf("  %d calls, %.1f ns/packet, %.1f MB/s\n",
           N, ns_per_call, mb_per_sec);
    /* At 10 Hz feature-state cadence, CRC budget is <100us/packet — we
     * expect bit-by-bit CRC32 to run ~1 MB/s on host, ~100-300 KB/s on
     * ESP32-S3 Xtensa LX7. 76-byte CRC takes <1 ms either way. */
    CHECK(ns_per_call < 50000.0, "CRC32(80B) must be under 50us/packet");
}

static void benchmark_finalize(void) {
    printf("bench: full finalize() cost\n");
    rv_feature_state_t pkt;
    memset(&pkt, 0x33, sizeof(pkt));

    const int N = 5000000;
    struct timespec a, b;
    clock_gettime(CLOCK_MONOTONIC, &a);
    for (int i = 0; i < N; i++) {
        rv_feature_state_finalize(&pkt, 1, (uint16_t)i, (uint64_t)i,
                                  RV_PROFILE_PASSIVE_LOW_RATE);
    }
    clock_gettime(CLOCK_MONOTONIC, &b);
    double ns_per_call = ((b.tv_sec - a.tv_sec) * 1e9 +
                          (b.tv_nsec - a.tv_nsec)) / (double)N;
    printf("  %d calls, %.1f ns/call (includes CRC)\n", N, ns_per_call);
}

int main(void) {
    printf("=== rv_feature_state_* host tests ===\n\n");

    test_packet_size();
    test_crc_known_vectors();
    test_finalize();
    test_crc_verifiability();
    benchmark_crc();
    benchmark_finalize();

    printf("\n=== result: %d pass, %d fail ===\n", g_pass, g_fail);
    return g_fail > 0 ? 1 : 0;
}
