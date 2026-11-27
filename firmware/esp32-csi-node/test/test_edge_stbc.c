/* SYNTHETIC: execute the production DSP, including STBC frames. */
#define _USE_MATH_DEFINES
#ifndef M_PI
#define M_PI 3.14159265358979323846
#endif
#define TickType_t unsigned
#define portNUM_PROCESSORS 2
#include "../main/edge_processing.c"
#include <assert.h>
#include <stdio.h>

nvs_config_t g_nvs_config;
uint8_t csi_collector_get_node_id(void) { return 1; }

int main(void)
{
    edge_config_t config = {.top_k_count = EDGE_TOP_K, .vital_interval_ms = 1};
    assert(edge_processing_init(&config) == ESP_OK);
    edge_ring_slot_t slot = {0};
    process_frame(&slot);
    assert(s_frame_count == 0);
    /* Alternate ordinary HT20 and STBC shapes. Every complete frame must
     * advance the production DSP clock; the oversized suffix is bounded. */
    const uint16_t lengths[] = {256, 384, 256, 384, EDGE_MAX_IQ_BYTES};
    for (unsigned i = 0; i < sizeof(lengths) / sizeof(lengths[0]); i++) {
        slot.iq_len = lengths[i];
        slot.timestamp_us = (i + 1) * 50000;
        slot.rssi = -45;
        memset(slot.iq_data, 1, sizeof(slot.iq_data));
        process_frame(&slot);
        assert(s_frame_count == i + 1);
        unsigned bins = lengths[i] / 2;
        if (bins > EDGE_MAX_SUBCARRIERS) bins = EDGE_MAX_SUBCARRIERS;
        assert(s_subcarrier_var[bins - 1].count > 0);
        assert(s_pkt_valid);
    }
    puts("PASS: production DSP accepts HT20/STBC and bounds oversized input");
    return 0;
}
