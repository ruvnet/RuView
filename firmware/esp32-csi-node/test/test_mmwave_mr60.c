/**
 * @file test_mmwave_mr60.c
 * @brief Host-side unit tests for the 0x0A-family helpers in mmwave_mr60.h:
 *        probe classification (#2136), the payload cap, and 0x0A04 decoding.
 *
 * These pin the decision logic only. The frame lengths they assume come from
 * the Seeed MR60BHA2 library; LD6002B/LD6004 behaviour still needs a hardware
 * capture to confirm.
 *
 *   cc -std=c99 -Wall -I../main -o test_mmwave_mr60 test_mmwave_mr60.c -lm && ./test_mmwave_mr60
 */
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "mmwave_mr60.h"

static int failures = 0;
#define CHECK(cond, msg) do { \
    if (!(cond)) { printf("FAIL: %s\n", msg); failures++; } \
    else        { printf("ok:   %s\n", msg); } \
} while (0)

/* Encode a full frame; returns its length. */
static int make_frame(uint8_t *out, uint16_t type, const uint8_t *payload, uint16_t plen)
{
    out[0] = MR60_SOF;
    out[1] = 0x00; out[2] = 0x01;
    out[3] = (uint8_t)(plen >> 8); out[4] = (uint8_t)plen;
    out[5] = (uint8_t)(type >> 8); out[6] = (uint8_t)type;
    out[7] = mmwave_mr60_checksum(out, 7);
    memcpy(out + 8, payload, plen);
    out[8 + plen] = mmwave_mr60_checksum(payload, plen);
    return 9 + plen;
}

/* Feed a frame's header through the probe classifier. */
static void note_frame(mmwave_mr60_probe_t *p, uint16_t type, uint16_t plen)
{
    uint8_t buf[300];
    uint8_t payload[260] = {0};
    int n = make_frame(buf, type, payload, plen);
    uint16_t t, l;
    if (mmwave_mr60_header_at(buf, 0, n, &t, &l)) mmwave_mr60_probe_note(p, t, l);
}

static void put_f32(uint8_t *p, float f) { memcpy(p, &f, 4); }
static void put_u32(uint8_t *p, uint32_t v)
{
    p[0] = (uint8_t)v; p[1] = (uint8_t)(v >> 8); p[2] = (uint8_t)(v >> 16); p[3] = (uint8_t)(v >> 24);
}

static int make_targets(uint8_t *out, uint32_t n, const float (*xy)[2])
{
    put_u32(out, n);
    for (uint32_t k = 0; k < n; k++) {
        uint8_t *r = out + 4 + k * 16;
        put_f32(r, xy[k][0]);
        put_f32(r + 4, xy[k][1]);
        put_u32(r + 8, 0x11110000u + k);
        put_u32(r + 12, 0x22220000u + k);
    }
    return (int)(4 + n * 16);
}

int main(void)
{
    uint8_t buf[300];

    /* ---- header predicate ---- */
    uint8_t hr[4]; put_f32(hr, 72.0f);
    int n = make_frame(buf, MR60_TYPE_HEARTRATE, hr, 4);
    uint16_t t = 0, l = 0;
    CHECK(mmwave_mr60_header_at(buf, 0, n, &t, &l) && t == 0x0A15 && l == 4,
          "valid heart-rate header parsed");
    buf[7] ^= 0xFF;
    CHECK(!mmwave_mr60_header_at(buf, 0, n, &t, &l), "bad header checksum REJECTED");
    n = make_frame(buf, 0x0B01, hr, 4);
    CHECK(!mmwave_mr60_header_at(buf, 0, n, &t, &l), "non-0x0A type REJECTED");
    n = make_frame(buf, MR60_TYPE_HEARTRATE, hr, 4);
    CHECK(!mmwave_mr60_header_at(buf, 0, 7, &t, &l), "truncated header REJECTED");

    /* ---- probe classification (#2136) ---- */
    mmwave_mr60_probe_t p;

    memset(&p, 0, sizeof(p));
    note_frame(&p, MR60_TYPE_PRESENCE, 1);
    note_frame(&p, MR60_TYPE_PHASE, 12);
    note_frame(&p, MR60_TYPE_BREATHING, 4);
    CHECK(mmwave_mr60_probe_verdict(&p, false) == MMWAVE_MR60_PROBE_MATCH,
          "MR60BHA2 stream (presence, phase/12, breath/4) is a MATCH");

    memset(&p, 0, sizeof(p));
    note_frame(&p, MR60_TYPE_BREATHING, 4);
    note_frame(&p, MR60_TYPE_DISTANCE, 8);
    note_frame(&p, MR60_TYPE_HEARTRATE, 4);
    CHECK(mmwave_mr60_probe_verdict(&p, false) == MMWAVE_MR60_PROBE_MATCH,
          "MR60BHA2 stream with heart rate/4 is a MATCH");

    memset(&p, 0, sizeof(p));
    note_frame(&p, MR60_TYPE_BREATHING, 4);
    note_frame(&p, MR60_TYPE_BREATHING, 4);
    note_frame(&p, MR60_TYPE_PRESENCE, 1);
    CHECK(mmwave_mr60_probe_verdict(&p, false) == MMWAVE_MR60_PROBE_UNDECIDED,
          "0x0A14/4 alone is not a signature (could be LD6002B work mode)");
    CHECK(mmwave_mr60_probe_verdict(&p, true) == MMWAVE_MR60_PROBE_AMBIGUOUS,
          "family frames with no signature at window close are AMBIGUOUS");

    memset(&p, 0, sizeof(p));
    note_frame(&p, MR60_TYPE_BREATHING, 1);   /* LD6002B-style work-mode report */
    CHECK(mmwave_mr60_probe_verdict(&p, false) == MMWAVE_MR60_PROBE_AMBIGUOUS,
          "0x0A14 with a non-4 length refuses the sensor at once");

    memset(&p, 0, sizeof(p));
    note_frame(&p, MR60_TYPE_HEARTRATE, 4);
    note_frame(&p, MR60_TYPE_PHASE, 12);
    note_frame(&p, MR60_TYPE_PHASE, 2);       /* LD6002B-style low-power report */
    CHECK(mmwave_mr60_probe_verdict(&p, false) == MMWAVE_MR60_PROBE_AMBIGUOUS,
          "a conflicting 0x0A13 overrides earlier signature frames");

    memset(&p, 0, sizeof(p));
    note_frame(&p, MR60_TYPE_POINTCLOUD, 36);
    note_frame(&p, MR60_TYPE_DISTANCE, 5);
    note_frame(&p, 0x0A38, 1);
    CHECK(mmwave_mr60_probe_verdict(&p, false) == MMWAVE_MR60_PROBE_UNDECIDED,
          "odd lengths on non-shared types do not refuse a sensor");

    memset(&p, 0, sizeof(p));
    note_frame(&p, MR60_TYPE_HEARTRATE, 4);
    CHECK(mmwave_mr60_probe_verdict(&p, true) == MMWAVE_MR60_PROBE_NONE,
          "one frame is too few for any verdict");

    memset(&p, 0, sizeof(p));
    CHECK(mmwave_mr60_probe_verdict(&p, true) == MMWAVE_MR60_PROBE_NONE,
          "silence is NONE");

    /* ---- payload cap ---- */
    CHECK(mmwave_mr60_payload_len_ok(4 + 2 * 16), "two-target payload (36 B) accepted");
    CHECK(mmwave_mr60_payload_len_ok(4 + 15 * 16), "fifteen-target payload (244 B) accepted");
    CHECK(!mmwave_mr60_payload_len_ok(MMWAVE_MR60_MAX_PAYLOAD + 1), "payload over cap REJECTED");
    CHECK(MMWAVE_MR60_MAX_PAYLOAD + 1 <= 256, "cap plus checksum fits a 256-byte buffer");

    /* ---- 0x0A04 decode (layout unconfirmed) ---- */
    mmwave_target_t out[MMWAVE_MAX_TARGETS];
    const float xy3[3][2] = {{0.5f, 1.2f}, {-0.8f, 2.0f}, {0.0f, 3.5f}};
    n = make_targets(buf, 3, xy3);
    int got = mmwave_mr60_decode_targets(buf, (uint16_t)n, out, MMWAVE_MAX_TARGETS);
    CHECK(got == 3, "three targets decoded");
    CHECK(out[1].x_m == -0.8f && out[1].y_m == 2.0f, "target 2 x/y round-trip");
    CHECK(out[2].word3 == 0x11110002u && out[2].word4 == 0x22220002u,
          "words 3/4 passed through raw");

    n = make_targets(buf, 0, NULL);
    CHECK(mmwave_mr60_decode_targets(buf, (uint16_t)n, out, MMWAVE_MAX_TARGETS) == 0,
          "zero-target frame decodes to 0");

    n = make_targets(buf, 3, xy3);
    CHECK(mmwave_mr60_decode_targets(buf, (uint16_t)(n - 1), out, MMWAVE_MAX_TARGETS) == -1,
          "length not 4+16n REJECTED");
    put_u32(buf, 4);
    CHECK(mmwave_mr60_decode_targets(buf, (uint16_t)n, out, MMWAVE_MAX_TARGETS) == -1,
          "declared count disagreeing with length REJECTED");
    CHECK(mmwave_mr60_decode_targets(buf, 3, out, MMWAVE_MAX_TARGETS) == -1,
          "payload shorter than the count REJECTED");

    const float bad[1][2] = {{NAN, 1.0f}};
    n = make_targets(buf, 1, bad);
    CHECK(mmwave_mr60_decode_targets(buf, (uint16_t)n, out, MMWAVE_MAX_TARGETS) == -1,
          "non-finite x REJECTED");

    /* Bare 16n array without a count: 32 bytes, first word reads as a float. */
    memset(buf, 0, sizeof(buf));
    put_f32(buf, 1.0f); put_f32(buf + 4, 2.0f); put_f32(buf + 16, 3.0f); put_f32(buf + 20, 4.0f);
    CHECK(mmwave_mr60_decode_targets(buf, 32, out, MMWAVE_MAX_TARGETS) == -1,
          "count-less 16n array REJECTED, not guessed");

    float xy10[10][2];
    for (int k = 0; k < 10; k++) { xy10[k][0] = (float)k; xy10[k][1] = 1.0f; }
    n = make_targets(buf, 10, (const float (*)[2])xy10);
    mmwave_target_t few[2];
    got = mmwave_mr60_decode_targets(buf, (uint16_t)n, few, 2);
    CHECK(got == 10 && few[1].x_m == 1.0f, "count reported in full, output clamped to max_out");

    printf("\n%s (%d failures)\n", failures ? "FAILED" : "ALL PASS", failures);
    return failures ? 1 : 0;
}
