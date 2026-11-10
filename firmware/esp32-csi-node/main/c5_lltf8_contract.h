/* Local C5 L-LTF8 format claim v1. Not hardware attestation or accuracy. */
#ifndef RUVIEW_C5_LLTF8_CONTRACT_H
#define RUVIEW_C5_LLTF8_CONTRACT_H
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#define C5_LLTF8_V1_BYTES 16u
#define C5_LLTF8_V1_IQ_BYTES 106u

typedef struct {
    bool config_readback;
    unsigned bb_format, secondary, bandwidth, estimate_valid;
    bool first_word_invalid;
    size_t iq_bytes;
} c5_lltf8_v1_metadata_t;

/* Native order is the SDK-documented imag/real signed int8 stream, not
 * a padded 64-point FFT. Only non-HT OFDM (11g/11a), primary 20 MHz and
 * the driver's valid-estimate indication may make this narrow claim. */
static inline bool c5_lltf8_v1_encode(const c5_lltf8_v1_metadata_t *m,
                                    uint8_t out[C5_LLTF8_V1_BYTES])
{
    if (!m || !out || !m->config_readback || m->bb_format != 1u ||
        m->secondary != 0u || m->bandwidth != 0u || m->estimate_valid != 1u ||
        m->iq_bytes != C5_LLTF8_V1_IQ_BYTES) return false;
    const uint8_t claim[C5_LLTF8_V1_BYTES] = {
        'C', '5', 'L', '8', 1, 8, 1, 1, 1,
        1, 0, 0, 1, m->first_word_invalid ? 1u : 0u, 106, 0
    };
    memcpy(out, claim, sizeof(claim));
    return true;
}

/* Append after the declared 106-byte ADR-018 IQ payload. The caller keeps
 * raw_frame_bytes separately so the extension can never enter edge DSP. */
static inline size_t c5_lltf8_v1_append(uint8_t *frame, size_t capacity,
                                      size_t raw_frame_bytes,
                                      const uint8_t claim[C5_LLTF8_V1_BYTES])
{
    if (!frame || !claim || raw_frame_bytes != 20u + C5_LLTF8_V1_IQ_BYTES ||
        capacity < raw_frame_bytes + C5_LLTF8_V1_BYTES) return 0;
    memcpy(frame + raw_frame_bytes, claim, C5_LLTF8_V1_BYTES);
    return raw_frame_bytes + C5_LLTF8_V1_BYTES;
}
#endif
