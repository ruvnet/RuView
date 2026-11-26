/**
 * @file c5_lltf8.c
 * @brief C5L8/v1 datagram builder. Pure C, shared by the firmware and the host test.
 */
#include "c5_lltf8.h"

#include <string.h>

bool c5l8_channel_supported(uint8_t ch)
{
    if (ch >= 36 && ch <= 64)   return (ch % 4) == 0;
    if (ch >= 100 && ch <= 144) return (ch % 4) == 0;
    if (ch >= 149 && ch <= 177) return ((ch - 149) % 4) == 0;
    return false;
}

static void put_le16(uint8_t *p, uint16_t v) { p[0] = (uint8_t)v; p[1] = (uint8_t)(v >> 8); }
static void put_le32(uint8_t *p, uint32_t v)
{
    p[0] = (uint8_t)v; p[1] = (uint8_t)(v >> 8); p[2] = (uint8_t)(v >> 16); p[3] = (uint8_t)(v >> 24);
}

c5l8_status_t c5l8_build(const c5l8_input_t *in, uint8_t *buf, size_t buf_len, size_t *out_len)
{
    if (out_len != NULL) *out_len = 0;
    if (in == NULL || buf == NULL || out_len == NULL || in->iq == NULL || buf_len < C5L8_FRAME_BYTES) {
        return C5L8_REJECT_BUFFER;
    }
    if (!in->format_readback_ok)                        return C5L8_REJECT_NO_READBACK;
    if (in->sta_bandwidth_mhz != 20)                    return C5L8_REJECT_STA_BANDWIDTH;
    if (in->bb_format != C5L8_BB_FORMAT_NON_HT_OFDM)    return C5L8_REJECT_BB_FORMAT;
    if (in->secondary_channel != 0)                     return C5L8_REJECT_SECONDARY;
    if (!in->estimate_valid)                            return C5L8_REJECT_ESTIMATE_INVALID;
    if (in->iq_len != C5L8_IQ_BYTES)                    return C5L8_REJECT_LENGTH;
    if (!c5l8_channel_supported(in->primary_channel))   return C5L8_REJECT_CHANNEL;

    uint8_t flags = 0;
    if (in->sync_valid)         flags |= C5L8_FLAG_SYNC_VALID;
    if (in->first_word_invalid) flags |= C5L8_FLAG_SANITIZED;

    put_le32(&buf[0], C5L8_MAGIC);
    buf[4] = in->node_id;
    buf[5] = 1;                                   /* one antenna */
    put_le16(&buf[6], C5L8_SLOTS);
    put_le32(&buf[8], 5000u + 5u * in->primary_channel);
    put_le32(&buf[12], in->sequence);
    buf[16] = (uint8_t)in->rssi;
    buf[17] = (uint8_t)in->noise_floor;
    buf[18] = 0;                                  /* PPDU: non-HT bucket */
    buf[19] = flags;

    uint8_t *iq = &buf[C5L8_HEADER_BYTES];
    memcpy(iq, in->iq, C5L8_IQ_BYTES);            /* original SDK imag/real byte order */
    if (in->first_word_invalid) memset(iq, 0, 4); /* zero, never remove: geometry stays 53 slots */

    uint8_t *s = &buf[C5L8_HEADER_BYTES + C5L8_IQ_BYTES];
    s[0] = 'C'; s[1] = '5'; s[2] = 'L'; s[3] = '8';
    s[4] = 1;   /* format version */
    s[5] = 8;   /* component bits */
    s[6] = 1;   /* SDK-documented imaginary/real byte order */
    s[7] = 1;   /* L-LTF 53-tone order [0..26, -26..-1] */
    s[8] = 1;   /* matched-format readback */
    s[9] = C5L8_BB_FORMAT_NON_HT_OFDM;
    s[10] = 0;  /* original secondary channel */
    s[11] = 20; /* STA bandwidth readback, MHz */
    s[12] = 1;  /* original estimate valid */
    s[13] = in->first_word_invalid ? 1 : 0;
    put_le16(&s[14], C5L8_IQ_BYTES);

    *out_len = C5L8_FRAME_BYTES;
    return C5L8_OK;
}

c5l8_status_t c5l8_selftest_frame(unsigned which, uint8_t node_id, uint8_t *buf, size_t buf_len,
                                  size_t *out_len)
{
    if (which >= C5L8_SELFTEST_CASES) {
        if (out_len != NULL) *out_len = 0;
        return C5L8_REJECT_BUFFER;
    }
    uint8_t iq[C5L8_IQ_BYTES];
    for (unsigned n = 0; n < C5L8_SLOTS; n++) {
        iq[2 * n]     = (uint8_t)(int8_t)((int)n - 30);   /* imaginary (SDK byte 0) */
        iq[2 * n + 1] = (uint8_t)(int8_t)(53 - (int)n);   /* real (SDK byte 1) */
    }
    c5l8_input_t in = {
        .node_id = node_id, .sequence = 0x5E1F0000u + which,
        .primary_channel = C5L8_DEFAULT_CHANNEL, .rssi = -40, .noise_floor = -95,
        .bb_format = C5L8_BB_FORMAT_NON_HT_OFDM, .secondary_channel = 0,
        .estimate_valid = true, .first_word_invalid = (which == 1), .sync_valid = true,
        .format_readback_ok = true, .sta_bandwidth_mhz = 20,
        .iq = iq, .iq_len = sizeof(iq),
    };
    return c5l8_build(&in, buf, buf_len, out_len);
}

const char *c5l8_status_str(c5l8_status_t status)
{
    switch (status) {
    case C5L8_OK:                      return "ok";
    case C5L8_REJECT_NO_READBACK:      return "csi_config_readback_mismatch";
    case C5L8_REJECT_STA_BANDWIDTH:    return "sta_bandwidth_not_20mhz";
    case C5L8_REJECT_BB_FORMAT:        return "not_non_ht_ofdm";
    case C5L8_REJECT_SECONDARY:        return "secondary_channel_present";
    case C5L8_REJECT_ESTIMATE_INVALID: return "estimate_invalid";
    case C5L8_REJECT_LENGTH:           return "callback_len_not_106";
    case C5L8_REJECT_CHANNEL:          return "unsupported_5ghz_primary";
    case C5L8_REJECT_BUFFER:           return "buffer";
    default:                           return "unknown";
    }
}
