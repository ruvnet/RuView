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
 * True iff a frame type is specific enough to identify the supported Seeed
 * MR60BHA2. The broader 0x0A family is shared by other Hi-Link sensors:
 * 0x0A14/0x0A13/0x0A04 can mean work-mode, low-power, and target data on
 * LD6002B/LD6004, so those types must never establish an MR60BHA2 identity.
 */
static inline bool mmwave_mr60_identifying_type(uint16_t type)
{
    return type == 0x0A15 || type == 0x0A16 || type == 0x0F09;
}

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

#endif /* MMWAVE_DETECT_H */
