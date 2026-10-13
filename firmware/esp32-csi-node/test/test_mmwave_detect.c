/**
 * @file test_mmwave_detect.c
 * @brief Host-side unit tests for the LD2410 frame-validation predicate (#1135)
 *        and the LD2450 frame validator + target decoder.
 *
 * Proves the phantom-detection fix: a floating UART can emit the 4-byte head
 * 0xF4F3F2F1, but the predicate rejects it unless a sane length + matching tail
 * 0xF8F7F6F5 are also present. Tests the REAL predicate from mmwave_detect.h
 * (the same code the firmware's probe_at_baud calls).
 *
 *   cc -std=c99 -Wall -I../main -o test_mmwave_detect test_mmwave_detect.c && ./test_mmwave_detect
 *
 * Exits 0 on all-pass; prints the failing case otherwise.
 */
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "mmwave_detect.h"

static int failures = 0;
#define CHECK(cond, msg) do { \
    if (!(cond)) { printf("FAIL: %s\n", msg); failures++; } \
    else        { printf("ok:   %s\n", msg); } \
} while (0)

/* Build a valid LD2410 report frame: F4F3F2F1 | len(LE) | data[len] | F8F7F6F5 */
static int make_frame(uint8_t *out, uint16_t dlen)
{
    int n = 0;
    out[n++] = 0xF4; out[n++] = 0xF3; out[n++] = 0xF2; out[n++] = 0xF1;
    out[n++] = (uint8_t)(dlen & 0xFF); out[n++] = (uint8_t)(dlen >> 8);
    for (uint16_t k = 0; k < dlen; k++) out[n++] = (uint8_t)(0xAA ^ k);
    out[n++] = 0xF8; out[n++] = 0xF7; out[n++] = 0xF6; out[n++] = 0xF5;
    return n;
}

int main(void)
{
    uint8_t buf[256];

    /* 1. A real basic-report frame (data len 13) validates. */
    int n = make_frame(buf, 13);
    CHECK(mmwave_ld2410_valid_at(buf, 0, n), "valid basic frame (len=13) accepted");

    /* 2. A real engineering-report frame (data len 35) validates. */
    n = make_frame(buf, 35);
    CHECK(mmwave_ld2410_valid_at(buf, 0, n), "valid engineering frame (len=35) accepted");

    /* 3. Head magic present but NO valid tail — the #1135 phantom case. */
    memset(buf, 0x00, sizeof(buf));
    buf[0]=0xF4; buf[1]=0xF3; buf[2]=0xF2; buf[3]=0xF1; buf[4]=13; buf[5]=0;
    /* data present but tail is zeros, not F8F7F6F5 */
    CHECK(!mmwave_ld2410_valid_at(buf, 0, 64), "head magic without valid tail REJECTED (#1135)");

    /* 4. Head magic with insane length is rejected. */
    memset(buf, 0xFF, sizeof(buf));
    buf[0]=0xF4; buf[1]=0xF3; buf[2]=0xF2; buf[3]=0xF1; buf[4]=0xFF; buf[5]=0xFF; /* len=65535 */
    CHECK(!mmwave_ld2410_valid_at(buf, 0, 200), "head magic with oversized length REJECTED");

    /* 5. Pure noise (no head) is rejected. */
    for (int k = 0; k < 64; k++) buf[k] = (uint8_t)(0x5A + k);
    CHECK(!mmwave_ld2410_valid_at(buf, 0, 64), "non-header noise REJECTED");

    /* 6. Truncated frame (tail would run past the buffer) is rejected. */
    n = make_frame(buf, 13);
    CHECK(!mmwave_ld2410_valid_at(buf, 0, n - 2), "truncated frame (tail past buffer) REJECTED");

    /* 7. Valid frame at a non-zero offset still validates. */
    memset(buf, 0x00, sizeof(buf));
    n = make_frame(buf + 7, 13);
    CHECK(mmwave_ld2410_valid_at(buf, 7, 7 + n), "valid frame at offset 7 accepted");

    /* 8. Repeated head bytes without a frame (worst-case noise) rejected. */
    for (int k = 0; k + 3 < 64; k += 4) {
        buf[k]=0xF4; buf[k+1]=0xF3; buf[k+2]=0xF2; buf[k+3]=0xF1;
    }
    CHECK(!mmwave_ld2410_valid_at(buf, 0, 64), "repeated bare head bytes REJECTED");

    /* ---- LD2450 ---- */
    /* Frame from the Hi-Link manual's worked example: target 1 at
     * x=0x030E (-782 mm), y=0x86B1 (+1713 mm), speed=0x8010 (+16 cm/s),
     * res=0x0168 (360 mm); slots 2 and 3 empty. */
    static const uint8_t ld50[30] = {
        0xAA, 0xFF, 0x03, 0x00,
        0x0E, 0x03, 0xB1, 0x86, 0x10, 0x80, 0x68, 0x01,
        0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0,
        0x55, 0xCC,
    };
    mmwave_ld2450_target_t tg[3];

    /* 9. The manual's frame validates and decodes to its documented values. */
    CHECK(mmwave_ld2450_valid_at(ld50, 0, 30), "LD2450 manual frame accepted");
    memset(tg, 0, sizeof(tg));
    int nt = mmwave_ld2450_decode(ld50, tg);
    CHECK(nt == 1, "LD2450 manual frame: 1 target, empty slots skipped");
    CHECK(tg[0].x_mm == -782, "LD2450 x 0x030E -> -782 mm (top bit clear = negative)");
    CHECK(tg[0].y_mm == 1713, "LD2450 y 0x86B1 -> +1713 mm (top bit set = positive)");
    CHECK(tg[0].speed_cms == 16, "LD2450 speed 0x8010 -> +16 cm/s");
    CHECK(tg[0].res_mm == 360, "LD2450 resolution 0x0168 -> 360 mm (unsigned)");

    /* 10. An empty first slot does not hide a target in slot 3. */
    uint8_t f3[30];
    memcpy(f3, ld50, 30);
    memset(&f3[4], 0, 8);
    f3[20] = 0x0E; f3[21] = 0x03; f3[22] = 0xB1; f3[23] = 0x86;
    nt = mmwave_ld2450_decode(f3, tg);
    CHECK(nt == 1 && tg[0].x_mm == -782 && tg[0].y_mm == 1713,
          "LD2450 target in slot 3 packed to the front");

    /* 11. Head without the 55 CC tail is rejected (noise guard, as #1135). */
    memcpy(f3, ld50, 30);
    f3[29] = 0x00;
    CHECK(!mmwave_ld2450_valid_at(f3, 0, 30), "LD2450 head without tail REJECTED");

    /* 12. Truncated frame (fewer than 30 bytes available) is rejected. */
    CHECK(!mmwave_ld2450_valid_at(ld50, 0, 29), "LD2450 truncated frame REJECTED");

    /* 13. Frame at an offset inside a larger buffer validates. */
    memset(buf, 0x11, sizeof(buf));
    memcpy(buf + 13, ld50, 30);
    CHECK(mmwave_ld2450_valid_at(buf, 13, 43) && !mmwave_ld2450_valid_at(buf, 12, 43),
          "LD2450 frame at offset 13 found, off-by-one rejected");

    /* 14. LD2410 and LD2450 predicates never accept each other's frames. */
    n = make_frame(buf, 13);
    CHECK(!mmwave_ld2450_valid_at(buf, 0, n), "LD2410 frame is not an LD2450 frame");
    CHECK(!mmwave_ld2410_valid_at(ld50, 0, 30), "LD2450 frame is not an LD2410 frame");

    /* 15. Negative zero (0x0000 with other fields set) decodes as 0. */
    CHECK(mmwave_ld2450_sm(0x0000) == 0 && mmwave_ld2450_sm(0x8000) == 0,
          "LD2450 sign-magnitude zero both ways");

    printf("\n%s (%d failures)\n", failures ? "FAILED" : "ALL PASS", failures);
    return failures ? 1 : 0;
}
