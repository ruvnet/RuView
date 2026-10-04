/**
 * @file mmwave_mr60.h
 * @brief Pure (host-testable) helpers for the Seeed/Hi-Link 0x0A-family frame
 *        protocol: probe classification, payload bounds and 0x0A04 target
 *        decoding. No ESP-IDF deps — safe to #include in a host unit test.
 *
 * Frame layout (shared by MR60BHA2, LD6002, LD6002B, LD6004):
 *   [0] SOF=0x01 | [1-2] id BE | [3-4] len BE | [5-6] type BE | [7] ~XOR(0..6)
 *   [8..] payload[len] | [8+len] ~XOR(payload)
 *
 * The frame layout alone does not identify the sensor. Hi-Link's LD6002B and
 * LD6004 reuse type IDs the MR60BHA2 uses for vitals (0x0A14 is the LD6002B
 * work-mode report, 0x0A13 its low-power report), so a probe that accepts any
 * 0x0Axx frame would decode those reports as breathing rate and phase (#2136).
 * The probe below only claims MR60BHA2 when it sees a heart-rate or phase
 * frame with the MR60BHA2 payload length, and refuses the sensor when a shared
 * type ID arrives with a different length.
 *
 * NEEDS HARDWARE VALIDATION: the lengths below come from the Seeed MR60BHA2
 * Arduino library and have not been checked against LD6002B/LD6004 captures.
 */
#ifndef MMWAVE_MR60_H
#define MMWAVE_MR60_H

#include <stdint.h>
#include <stdbool.h>
#include <string.h>
#include <math.h>

#define MR60_SOF                0x01
#define MR60_HEADER_LEN         8

/* Frame types (big-endian uint16 at offset 5-6). */
#define MR60_TYPE_POINTCLOUD    0x0A04
#define MR60_TYPE_PHASE         0x0A13
#define MR60_TYPE_BREATHING     0x0A14
#define MR60_TYPE_HEARTRATE     0x0A15
#define MR60_TYPE_DISTANCE      0x0A16
#define MR60_TYPE_PRESENCE      0x0F09

/* MR60BHA2 payload lengths for the vitals types. */
#define MR60_LEN_PHASE          12  /* total, breath, heart phase: 3 x f32 LE */
#define MR60_LEN_BREATHING      4   /* f32 LE */
#define MR60_LEN_HEARTRATE      4   /* f32 LE */
#define MR60_LEN_DISTANCE       8   /* u32 range flag + f32 cm */

/* Largest payload the parser accepts. The old limit of 30 bytes dropped every
 * 0x0A04 frame with two or more targets (4 + 16n bytes). 252 holds 15 targets
 * and still fits the parser's 256-byte buffer. */
#define MMWAVE_MR60_MAX_PAYLOAD 252

static inline uint8_t mmwave_mr60_checksum(const uint8_t *data, uint16_t len)
{
    uint8_t cksum = 0;
    for (uint16_t i = 0; i < len; i++) {
        cksum ^= data[i];
    }
    return (uint8_t)~cksum;
}

static inline bool mmwave_mr60_payload_len_ok(uint16_t len)
{
    return len <= MMWAVE_MR60_MAX_PAYLOAD;
}

/**
 * True iff buf[i..i+7] is a checksum-valid 0x0A-family header (type high byte
 * 0x0A, or 0x0F09). Writes the type and payload length on success.
 */
static inline bool mmwave_mr60_header_at(const uint8_t *buf, int i, int len,
                                         uint16_t *type, uint16_t *data_len)
{
    if (i < 0 || i + MR60_HEADER_LEN > len) return false;
    const uint8_t *h = &buf[i];
    if (h[0] != MR60_SOF) return false;
    if (mmwave_mr60_checksum(h, 7) != h[7]) return false;
    uint16_t t = (uint16_t)((h[5] << 8) | h[6]);
    if ((t >> 8) != 0x0A && t != MR60_TYPE_PRESENCE) return false;
    *type = t;
    *data_len = (uint16_t)((h[3] << 8) | h[4]);
    return true;
}

/* ---- Probe classification (#2136) ---- */

typedef enum {
    MMWAVE_MR60_PROBE_UNDECIDED = 0, /**< Keep reading. */
    MMWAVE_MR60_PROBE_MATCH,         /**< MR60BHA2 vitals signature seen. */
    MMWAVE_MR60_PROBE_AMBIGUOUS,     /**< 0x0A family, not an MR60BHA2: refuse. */
    MMWAVE_MR60_PROBE_NONE,          /**< Too few frames to say anything. */
} mmwave_mr60_verdict_t;

typedef struct {
    uint16_t family_frames;    /**< Valid 0x0A-family headers. */
    uint16_t signature_frames; /**< HR/phase frames with the MR60BHA2 length. */
    uint16_t conflict_frames;  /**< Shared vitals type IDs with another length. */
} mmwave_mr60_probe_t;

#define MMWAVE_MR60_PROBE_MIN_FRAMES 3

/*
 * Which frames count as what:
 *  - signature: 0x0A15 heart rate (4 bytes) or 0x0A13 phase (12 bytes). Neither
 *    is known to be sent with these lengths by a non-vitals sensor. 0x0A14 is
 *    deliberately not a signature, because the LD6002B work-mode report shares
 *    the type and its length is not confirmed.
 *  - conflict: 0x0A13 or 0x0A14 with any other length. That is a sensor which
 *    uses the shared type IDs for something else.
 * Other types (0x0A04, 0x0A16, 0x0F09, ...) count only toward family_frames, so
 * an unexpected length there cannot refuse a real MR60BHA2.
 */
static inline void mmwave_mr60_probe_note(mmwave_mr60_probe_t *p,
                                          uint16_t type, uint16_t data_len)
{
    p->family_frames++;
    switch (type) {
    case MR60_TYPE_HEARTRATE:
        if (data_len == MR60_LEN_HEARTRATE) p->signature_frames++;
        break;
    case MR60_TYPE_PHASE:
        if (data_len == MR60_LEN_PHASE) p->signature_frames++;
        else p->conflict_frames++;
        break;
    case MR60_TYPE_BREATHING:
        if (data_len != MR60_LEN_BREATHING) p->conflict_frames++;
        break;
    default:
        break;
    }
}

/**
 * Decide what the probe has seen. A conflicting frame refuses the sensor at
 * once. A match needs MMWAVE_MR60_PROBE_MIN_FRAMES family frames and at least
 * one signature frame. With @p final set (the probe
 * window has closed), enough family frames without a signature is ambiguous.
 */
static inline mmwave_mr60_verdict_t mmwave_mr60_probe_verdict(
    const mmwave_mr60_probe_t *p, bool final)
{
    if (p->conflict_frames > 0) return MMWAVE_MR60_PROBE_AMBIGUOUS;
    if (p->family_frames >= MMWAVE_MR60_PROBE_MIN_FRAMES && p->signature_frames > 0) {
        return MMWAVE_MR60_PROBE_MATCH;
    }
    if (!final) return MMWAVE_MR60_PROBE_UNDECIDED;
    return (p->family_frames >= MMWAVE_MR60_PROBE_MIN_FRAMES)
           ? MMWAVE_MR60_PROBE_AMBIGUOUS : MMWAVE_MR60_PROBE_NONE;
}

/* ---- 0x0A04 target records ---- */

/*
 * LAYOUT UNCONFIRMED ON MR60BHA2 HARDWARE. Assumed layout, after the Seeed
 * MR60BHA2 Arduino library:
 *
 *   n:u32 LE, then n records of { x:f32 LE, y:f32 LE, word3:u32 LE, word4:u32 LE }
 *
 * x/y are metres in the radar's frame. The library names word3/word4
 * dop_index and cluster_index; they are passed through raw and not
 * interpreted. The decoder accepts a payload only when its length is exactly
 * 4 + 16n for the declared n and every x/y is finite; anything else decodes
 * to nothing, never to partial targets.
 */
#define MMWAVE_MR60_TARGET_REC_LEN 16
#define MMWAVE_MAX_TARGETS         8

typedef struct {
    float    x_m;
    float    y_m;
    uint32_t word3;  /**< "dop_index" in the Seeed library; meaning unconfirmed. */
    uint32_t word4;  /**< "cluster_index" in the Seeed library; meaning unconfirmed. */
} mmwave_target_t;

static inline uint32_t mmwave_mr60_u32le(const uint8_t *p)
{
    return (uint32_t)p[0] | ((uint32_t)p[1] << 8)
         | ((uint32_t)p[2] << 16) | ((uint32_t)p[3] << 24);
}

static inline float mmwave_mr60_f32le(const uint8_t *p)
{
    uint32_t bits = mmwave_mr60_u32le(p);
    float f;
    memcpy(&f, &bits, sizeof(f));
    return f;
}

/**
 * Decode a 0x0A04 payload. Returns the declared target count n (which may be
 * larger than @p max_out; only the first max_out are written), or -1 when the
 * payload does not match the assumed layout.
 */
static inline int mmwave_mr60_decode_targets(const uint8_t *p, uint16_t len,
                                             mmwave_target_t *out, int max_out)
{
    if (len < 4) return -1;
    uint32_t n = mmwave_mr60_u32le(p);
    if (n > (uint32_t)(MMWAVE_MR60_MAX_PAYLOAD / MMWAVE_MR60_TARGET_REC_LEN)) return -1;
    if (4u + n * MMWAVE_MR60_TARGET_REC_LEN != len) return -1;

    for (uint32_t k = 0; k < n; k++) {
        const uint8_t *r = p + 4 + k * MMWAVE_MR60_TARGET_REC_LEN;
        float x = mmwave_mr60_f32le(r);
        float y = mmwave_mr60_f32le(r + 4);
        if (!isfinite(x) || !isfinite(y)) return -1;
    }
    for (uint32_t k = 0; k < n && (int)k < max_out; k++) {
        const uint8_t *r = p + 4 + k * MMWAVE_MR60_TARGET_REC_LEN;
        out[k].x_m   = mmwave_mr60_f32le(r);
        out[k].y_m   = mmwave_mr60_f32le(r + 4);
        out[k].word3 = mmwave_mr60_u32le(r + 8);
        out[k].word4 = mmwave_mr60_u32le(r + 12);
    }
    return (int)n;
}

#endif /* MMWAVE_MR60_H */
