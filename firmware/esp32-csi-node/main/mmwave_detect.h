/**
 * @file mmwave_detect.h
 * @brief Pure (host-testable) mmWave frame-validation predicates for probe-time
 *        sensor detection. No ESP-IDF deps — safe to #include in a host unit test.
 *
 * Detection must validate a *full* frame, never a bare header byte/pattern: a
 * floating UART with no sensor reads line noise that can contain header-looking
 * bytes, which the old loose checks mistook for a real sensor (#1107 MR60,
 * #1135 LD2410). These predicates are the validate-before-trust gate.
 */
#ifndef MMWAVE_DETECT_H
#define MMWAVE_DETECT_H

#include <stdint.h>
#include <stdbool.h>

/**
 * True iff buf[i..] begins a *validated* LD2410 report frame within [0,len):
 *   F4 F3 F2 F1 | len(LE,2) | data[len] | F8 F7 F6 F5
 * Requires the head magic, a sane intra-frame length, AND the matching tail at
 * head+6+len. Pure noise that merely contains 0xF4F3F2F1 fails the tail check.
 */
static inline bool mmwave_ld2410_valid_at(const uint8_t *buf, int i, int len)
{
    if (i < 0 || i + 5 >= len) return false;
    if (!(buf[i] == 0xF4 && buf[i+1] == 0xF3 && buf[i+2] == 0xF2 && buf[i+3] == 0xF1))
        return false;
    uint16_t flen = (uint16_t)buf[i+4] | ((uint16_t)buf[i+5] << 8);
    /* Real LD2410 report frames are small (basic=13, engineering=35). */
    if (flen < 1 || flen > 64) return false;
    int tail = i + 6 + (int)flen;
    if (tail + 3 >= len) return false;
    return buf[tail] == 0xF8 && buf[tail+1] == 0xF7
        && buf[tail+2] == 0xF6 && buf[tail+3] == 0xF5;
}

/* ---- HLK-LD2450 (24 GHz, multi-target x/y tracking, 256000 baud) ----
 *
 * One fixed 30-byte report frame per update (~10 Hz):
 *   AA FF 03 00 | target[3] x 8 bytes | 55 CC
 * Each target is four LE uint16: x (mm), y (mm), speed (cm/s), distance
 * resolution (mm). x, y and speed are SIGN-MAGNITUDE with the top bit meaning
 * POSITIVE (Hi-Link manual's worked example: 0x030E -> -782 mm, 0x86B1 ->
 * +1713 mm). An all-zero slot means "no target". y points away from the
 * radar face; x is lateral.
 */
#define MMWAVE_LD2450_FRAME_LEN   30
#define MMWAVE_LD2450_MAX_TARGETS 3

typedef struct {
    int16_t  x_mm;
    int16_t  y_mm;
    int16_t  speed_cms;
    uint16_t res_mm;
} mmwave_ld2450_target_t;

/** True iff buf[i..i+30) is a whole LD2450 frame (head AND tail). */
static inline bool mmwave_ld2450_valid_at(const uint8_t *buf, int i, int len)
{
    if (i < 0 || i + MMWAVE_LD2450_FRAME_LEN > len) return false;
    const uint8_t *f = &buf[i];
    return f[0] == 0xAA && f[1] == 0xFF && f[2] == 0x03 && f[3] == 0x00
        && f[28] == 0x55 && f[29] == 0xCC;
}

/** Sign-magnitude, top bit = positive. */
static inline int16_t mmwave_ld2450_sm(uint16_t raw)
{
    return (raw & 0x8000) ? (int16_t)(raw & 0x7FFF) : (int16_t)-(int16_t)(raw & 0x7FFF);
}

/**
 * Decode a validated 30-byte frame into up to 3 targets, packed to the front
 * of `out` (empty slots skipped). Returns the target count.
 */
static inline int mmwave_ld2450_decode(const uint8_t *frame,
                                       mmwave_ld2450_target_t out[MMWAVE_LD2450_MAX_TARGETS])
{
    int n = 0;
    for (int t = 0; t < MMWAVE_LD2450_MAX_TARGETS; t++) {
        const uint8_t *p = &frame[4 + 8 * t];
        uint16_t x = (uint16_t)p[0] | ((uint16_t)p[1] << 8);
        uint16_t y = (uint16_t)p[2] | ((uint16_t)p[3] << 8);
        uint16_t v = (uint16_t)p[4] | ((uint16_t)p[5] << 8);
        uint16_t r = (uint16_t)p[6] | ((uint16_t)p[7] << 8);
        if (x == 0 && y == 0 && v == 0 && r == 0) continue;
        out[n].x_mm = mmwave_ld2450_sm(x);
        out[n].y_mm = mmwave_ld2450_sm(y);
        out[n].speed_cms = mmwave_ld2450_sm(v);
        out[n].res_mm = r;
        n++;
    }
    return n;
}

#endif /* MMWAVE_DETECT_H */
