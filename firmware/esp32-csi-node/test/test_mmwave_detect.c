/**
 * @file test_mmwave_detect.c
 * @brief Host-side unit tests for the LD2410 frame-validation predicate (#1135).
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

    /* 0x0Axx is not an identity: Hi-Link LD6002B/LD6004 use overlapping
     * frame structure, with these IDs assigned to work-mode/low-power/targets. */
    CHECK(!mmwave_mr60_identifying_type(0x0A14), "ambiguous LD6002B work-mode type rejected (#2136)");
    CHECK(!mmwave_mr60_identifying_type(0x0A13), "ambiguous LD6002B low-power type rejected (#2136)");
    CHECK(!mmwave_mr60_identifying_type(0x0A04), "ambiguous LD6002B target type rejected (#2136)");
    CHECK(!mmwave_mr60_identifying_type(0x0A99), "unknown 0x0A family type rejected");
    CHECK(mmwave_mr60_identifying_type(0x0A15), "MR60BHA2 heart-rate type identifies sensor");
    CHECK(mmwave_mr60_identifying_type(0x0A16), "MR60BHA2 distance type identifies sensor");
    CHECK(mmwave_mr60_identifying_type(0x0F09), "MR60BHA2 presence type identifies sensor");

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

    printf("\n%s (%d failures)\n", failures ? "FAILED" : "ALL PASS", failures);
    return failures ? 1 : 0;
}
