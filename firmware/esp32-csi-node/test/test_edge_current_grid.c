/* SYNTHETIC: execute production DSP across changing CSI payload lengths.
 * This verifies index bounds, not equivalence between radio grids or accuracy. */
#define _USE_MATH_DEFINES
#ifndef M_PI
#define M_PI 3.14159265358979323846
#endif
#define TickType_t unsigned
#define portNUM_PROCESSORS 2
#include "../main/edge_processing.c"
#include <assert.h>
#include <math.h>
#include <stdio.h>

nvs_config_t g_nvs_config;
uint8_t csi_collector_get_node_id(void) { return 1; }

static void check_current_grid(const edge_ring_slot_t *slot)
{
    unsigned bins = slot->iq_len / 2;
    if (bins > EDGE_MAX_SUBCARRIERS) bins = EDGE_MAX_SUBCARRIERS;
    assert(s_top_k_count <= bins);
    for (unsigned k = 0; k < s_top_k_count; k++) {
        assert(s_top_k[k] < bins);
    }
    unsigned latest = (s_history_idx + EDGE_PHASE_HISTORY_LEN - 1)
                      % EDGE_PHASE_HISTORY_LEN;
    assert(isfinite(s_phase_history[latest]));
    assert(fabsf(s_phase_history[latest]) < 10.0f);
    assert(isfinite(s_motion_energy));
}

static void exercise_transition(bool invalid_primary)
{
    edge_config_t cfg = {.top_k_count = EDGE_TOP_K, .vital_interval_ms = 1000};
    assert(edge_processing_init(&cfg) == ESP_OK);
    edge_ring_slot_t slot = {0};
    slot.iq_len = 256;
    slot.rssi = -69;
    /* Establish a real selection outside a subsequent 64-bin frame. Check
     * both the primary and a secondary retained index, without seeding state. */
    for (unsigned frame = 1; frame <= 101; frame++) {
        for (unsigned sc = 0; sc < 128; sc++) {
            slot.iq_data[2 * sc] = 100;
            slot.iq_data[2 * sc + 1] = 0;
        }
        int sign = (frame % 2) ? 1 : -1;
        slot.iq_data[255] = (uint8_t)(int8_t)(sign * (invalid_primary ? 100 : 50));
        if (!invalid_primary) slot.iq_data[1] = (uint8_t)(int8_t)(sign * 100);
        slot.timestamp_us = frame * 50000;
        process_frame(&slot);
        check_current_grid(&slot);
    }
    assert(s_top_k[invalid_primary ? 0 : 1] == 127);
    slot.iq_len = 128;
    slot.timestamp_us = 102 * 50000;
    process_frame(&slot);
    check_current_grid(&slot);

    /* Repeated shrink/growth, bounded STBC prefix, and a single IQ pair. */
    const uint16_t lengths[] = {256, 128, 384, 128, 2, 128, 256, 128};
    for (unsigned f = 0; f < sizeof(lengths) / sizeof(lengths[0]); f++) {
        slot.iq_len = lengths[f];
        slot.timestamp_us = (103 + f) * 50000;
        process_frame(&slot);
        check_current_grid(&slot);
    }
    unsigned before = s_frame_count;
    slot.iq_len = 0;
    process_frame(&slot);
    assert(s_frame_count == before);
}

int main(void)
{
    exercise_transition(true);
    exercise_transition(false);
    puts("PASS: production DSP keeps all selected bins within the current CSI grid");
    return 0;
}
