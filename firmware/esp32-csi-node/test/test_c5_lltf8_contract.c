#include <assert.h>
#include <string.h>
#include "c5_lltf8_contract.h"

int main(void)
{
    const c5_lltf8_v1_metadata_t valid = {
        .config_readback = true, .bb_format = 1, .secondary = 0,
        .sta_bandwidth_mhz = 20, .estimate_valid = 1, .first_word_invalid = false,
        .iq_bytes = 106,
    };
    uint8_t claim[16], frame[142], before[142];
    assert(c5_lltf8_v1_encode(&valid, claim));
    const uint8_t expected[16] = {'C','5','L','8',1,8,1,1,1,1,0,20,1,0,106,0};
    assert(memcmp(claim, expected, sizeof(claim)) == 0);
    for (unsigned mutant = 0; mutant < 11; mutant++) {
        c5_lltf8_v1_metadata_t m = valid;
        switch (mutant) {
        case 0: m.config_readback = false; break;
        case 1: m.bb_format = 0; break;
        case 2: m.bb_format = 2; break;
        case 3: m.bb_format = 4; break;
        case 4: m.secondary = 1; break;
        case 5: m.secondary = 2; break;
        case 6: m.sta_bandwidth_mhz = 40; break;
        case 7: m.sta_bandwidth_mhz = 0; break;
        case 8: m.estimate_valid = 0; break;
        case 9: m.iq_bytes = 105; break;
        default: m.iq_bytes = 108; break;
        }
        memset(claim, 0xa5, sizeof(claim));
        assert(!c5_lltf8_v1_encode(&m, claim));
        for (unsigned i = 0; i < sizeof(claim); i++) assert(claim[i] == 0xa5);
    }
    assert(!c5_lltf8_v1_encode(NULL, claim));
    assert(!c5_lltf8_v1_encode(&valid, NULL));
    assert(c5_lltf8_v1_encode(&valid, claim));
    for (unsigned i = 0; i < sizeof(frame); i++) frame[i] = (uint8_t)i;
    memcpy(before, frame, sizeof(frame));
    assert(c5_lltf8_v1_append(frame, 141, 126, claim) == 0);
    assert(memcmp(frame, before, sizeof(frame)) == 0);
    assert(c5_lltf8_v1_append(frame, sizeof(frame), 125, claim) == 0);
    assert(memcmp(frame, before, sizeof(frame)) == 0);
    assert(c5_lltf8_v1_append(frame, sizeof(frame), 126, claim) == 142);
    assert(memcmp(frame, before, 126) == 0);
    assert(memcmp(frame + 126, expected, 16) == 0);
    c5_lltf8_v1_metadata_t sanitized = valid;
    sanitized.first_word_invalid = true;
    assert(c5_lltf8_v1_encode(&sanitized, claim));
    assert(claim[13] == 1);
    return 0;
}
