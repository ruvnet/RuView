/* Host harness: drives the firmware's own C5L8/v1 builder (c5_lltf8.c, compiled unchanged)
 * and prints one line per case: "<name> <status> <hex-or-dash>". SYNTHETIC IQ throughout. */
#include "c5_lltf8.h"
#include <stdio.h>
#include <string.h>

static uint8_t iq[C5L8_IQ_BYTES];

static void emit(const char *name, const c5l8_input_t *in)
{
    uint8_t buf[C5L8_FRAME_BYTES + 8];
    size_t len = 0;
    c5l8_status_t st = c5l8_build(in, buf, sizeof(buf), &len);
    printf("%s %s ", name, c5l8_status_str(st));
    if (st == C5L8_OK) { for (size_t i = 0; i < len; i++) printf("%02x", buf[i]); } else { printf("-"); }
    printf("\n");
}

static c5l8_input_t base(void)
{
    c5l8_input_t in = {
        .node_id = 23, .sequence = 1685, .primary_channel = 48, .rssi = -51, .noise_floor = -92,
        .bb_format = 1, .secondary_channel = 0, .estimate_valid = true, .first_word_invalid = false,
        .sync_valid = true, .format_readback_ok = true, .sta_bandwidth_mhz = 20,
        .iq = iq, .iq_len = C5L8_IQ_BYTES,
    };
    return in;
}

int main(void)
{
    for (unsigned n = 0; n < C5L8_SLOTS; n++) { iq[2 * n] = (uint8_t)(int8_t)(-7 + (int)n % 5); iq[2 * n + 1] = (uint8_t)(int8_t)(11 - (int)n % 7); }
    c5l8_input_t in;
    /* accepted */
    in = base();                                   emit("ok_5240_sync", &in);
    in = base(); in.first_word_invalid = true;     emit("ok_5240_sync_sanitized", &in);
    in = base(); in.sync_valid = false;            emit("ok_5240_nosync", &in);
    in = base(); in.sync_valid = false; in.first_word_invalid = true; emit("ok_5240_nosync_sanitized", &in);
    static const uint8_t chans[] = {36, 64, 100, 112, 144, 149, 165, 177};
    for (unsigned k = 0; k < sizeof(chans); k++) {
        char nm[32]; snprintf(nm, sizeof nm, "ok_ch%u", chans[k]);
        in = base(); in.primary_channel = chans[k]; emit(nm, &in);
    }
    /* withheld by the producer */
    in = base(); in.format_readback_ok = false;    emit("no_readback", &in);
    in = base(); in.sta_bandwidth_mhz = 40;        emit("bw40", &in);
    in = base(); in.bb_format = 2;                 emit("ht", &in);
    in = base(); in.bb_format = 4;                 emit("he_su", &in);
    in = base(); in.bb_format = 0;                 emit("dsss_11b", &in);
    in = base(); in.secondary_channel = 1;         emit("secondary", &in);
    in = base(); in.estimate_valid = false;        emit("estimate_invalid", &in);
    in = base(); in.iq_len = 128;                  emit("len128", &in);
    in = base(); in.iq_len = 104;                  emit("len104", &in);
    in = base(); in.primary_channel = 6;           emit("ch6_2g4", &in);
    in = base(); in.primary_channel = 38;          emit("ch38_offgrid", &in);
    in = base(); in.primary_channel = 181;         emit("ch181", &in);
    /* the firmware's serial self-test frames */
    for (unsigned k = 0; k < C5L8_SELFTEST_CASES; k++) {
        uint8_t buf[C5L8_FRAME_BYTES]; size_t len = 0;
        c5l8_status_t st = c5l8_selftest_frame(k, 23, buf, sizeof buf, &len);
        printf("selftest_%u %s ", k, c5l8_status_str(st));
        for (size_t i = 0; i < len; i++) printf("%02x", buf[i]);
        printf("\n");
    }
    return 0;
}
