/*
 * Host unit test for the C5L8/v1 builder (main/c5_lltf8.c): exact 142-byte layout,
 * header flags, suffix bytes, first-word sanitising, the supported 5 GHz channel set and
 * every refusal reason. SYNTHETIC IQ only. The server-side validator check of the same
 * frames lives in tests/c5l8v1-validator (it needs the room_learner validator source).
 */
#include "c5_lltf8.h"

#include <stdio.h>
#include <string.h>

static int failures;
#define CHECK(cond, ...) do { if (!(cond)) { failures++; printf("FAIL %s:%d: ", __FILE__, __LINE__); \
    printf(__VA_ARGS__); printf("\n"); } } while (0)

static uint8_t iq[C5L8_IQ_BYTES];

static c5l8_input_t base(void)
{
    c5l8_input_t in = {
        .node_id = 23, .sequence = 1685, .primary_channel = 48, .rssi = -51, .noise_floor = -92,
        .bb_format = C5L8_BB_FORMAT_NON_HT_OFDM, .secondary_channel = 0, .estimate_valid = true,
        .first_word_invalid = false, .sync_valid = true, .format_readback_ok = true,
        .sta_bandwidth_mhz = 20, .iq = iq, .iq_len = C5L8_IQ_BYTES,
    };
    return in;
}

static uint32_t le32(const uint8_t *p) { return p[0] | p[1] << 8 | p[2] << 16 | (uint32_t)p[3] << 24; }

static void check_frame(const char *name, const c5l8_input_t *in)
{
    uint8_t buf[C5L8_FRAME_BYTES + 8];
    memset(buf, 0xAA, sizeof buf);
    size_t len = 0;
    c5l8_status_t st = c5l8_build(in, buf, sizeof buf, &len);
    CHECK(st == C5L8_OK, "%s: status %s", name, c5l8_status_str(st));
    if (st != C5L8_OK) return;
    CHECK(len == 142, "%s: len %zu", name, len);
    CHECK(le32(buf) == C5L8_MAGIC, "%s: magic", name);
    CHECK(buf[4] == in->node_id && buf[5] == 1 && buf[6] == 53 && buf[7] == 0, "%s: node/ant/slots", name);
    CHECK(le32(&buf[8]) == 5000u + 5u * in->primary_channel, "%s: freq %u", name, (unsigned)le32(&buf[8]));
    CHECK(le32(&buf[12]) == in->sequence, "%s: sequence", name);
    CHECK((int8_t)buf[16] == in->rssi && (int8_t)buf[17] == in->noise_floor, "%s: rssi/noise", name);
    uint8_t flags = (in->sync_valid ? 0x10 : 0) | (in->first_word_invalid ? 0x20 : 0);
    CHECK(buf[18] == 0 && buf[19] == flags, "%s: ppdu/flags 0x%02x", name, buf[19]);
    if (in->first_word_invalid) {
        static const uint8_t z[4];
        CHECK(memcmp(&buf[20], z, 4) == 0, "%s: first word not zeroed", name);
        CHECK(memcmp(&buf[24], &iq[4], C5L8_IQ_BYTES - 4) == 0, "%s: IQ tail changed", name);
    } else {
        CHECK(memcmp(&buf[20], iq, C5L8_IQ_BYTES) == 0, "%s: IQ not copied verbatim", name);
    }
    const uint8_t suffix[16] = {'C', '5', 'L', '8', 1, 8, 1, 1, 1, 1, 0, 20, 1,
                                in->first_word_invalid ? 1 : 0, 106, 0};
    CHECK(memcmp(&buf[126], suffix, 16) == 0, "%s: suffix", name);
    CHECK(buf[142] == 0xAA, "%s: wrote past 142 bytes", name);
}

static void check_refused(const char *name, const c5l8_input_t *in, c5l8_status_t want)
{
    uint8_t buf[C5L8_FRAME_BYTES];
    size_t len = 99;
    c5l8_status_t st = c5l8_build(in, buf, sizeof buf, &len);
    CHECK(st == want, "%s: got %s want %s", name, c5l8_status_str(st), c5l8_status_str(want));
    CHECK(len == 0, "%s: out_len %zu on refusal", name, len);
}

int main(void)
{
    for (unsigned n = 0; n < C5L8_SLOTS; n++) {
        iq[2 * n] = (uint8_t)(int8_t)(-7 + (int)n % 5);
        iq[2 * n + 1] = (uint8_t)(int8_t)(11 - (int)n % 7);
    }
    c5l8_input_t in;
    in = base();                                                  check_frame("sync", &in);
    in = base(); in.first_word_invalid = true;                    check_frame("sync_sanitized", &in);
    in = base(); in.sync_valid = false;                           check_frame("nosync", &in);
    in = base(); in.sync_valid = false; in.first_word_invalid = true; check_frame("nosync_sanitized", &in);

    for (unsigned ch = 0; ch < 200; ch++) {
        bool want = (ch >= 36 && ch <= 64 && ch % 4 == 0) || (ch >= 100 && ch <= 144 && ch % 4 == 0)
                 || (ch >= 149 && ch <= 177 && (ch - 149) % 4 == 0);
        CHECK(c5l8_channel_supported((uint8_t)ch) == want, "channel %u", ch);
        if (want) { char nm[16]; snprintf(nm, sizeof nm, "ch%u", ch); in = base(); in.primary_channel = (uint8_t)ch; check_frame(nm, &in); }
    }

    in = base(); in.format_readback_ok = false;  check_refused("no_readback", &in, C5L8_REJECT_NO_READBACK);
    in = base(); in.sta_bandwidth_mhz = 40;      check_refused("bw40", &in, C5L8_REJECT_STA_BANDWIDTH);
    in = base(); in.bb_format = 0;               check_refused("dsss", &in, C5L8_REJECT_BB_FORMAT);
    in = base(); in.bb_format = 2;               check_refused("ht", &in, C5L8_REJECT_BB_FORMAT);
    in = base(); in.bb_format = 4;               check_refused("he_su", &in, C5L8_REJECT_BB_FORMAT);
    in = base(); in.secondary_channel = 1;       check_refused("secondary", &in, C5L8_REJECT_SECONDARY);
    in = base(); in.estimate_valid = false;      check_refused("estimate", &in, C5L8_REJECT_ESTIMATE_INVALID);
    in = base(); in.iq_len = 104;                check_refused("len104", &in, C5L8_REJECT_LENGTH);
    in = base(); in.iq_len = 128;                check_refused("len128", &in, C5L8_REJECT_LENGTH);
    in = base(); in.primary_channel = 6;         check_refused("ch6", &in, C5L8_REJECT_CHANNEL);
    in = base(); in.primary_channel = 38;        check_refused("ch38", &in, C5L8_REJECT_CHANNEL);
    in = base(); in.iq = NULL;                   check_refused("null_iq", &in, C5L8_REJECT_BUFFER);
    {
        uint8_t small[141]; size_t len = 7; in = base();
        CHECK(c5l8_build(&in, small, sizeof small, &len) == C5L8_REJECT_BUFFER && len == 0, "short buffer");
    }

    for (unsigned k = 0; k < C5L8_SELFTEST_CASES; k++) {
        uint8_t buf[C5L8_FRAME_BYTES]; size_t len = 0;
        CHECK(c5l8_selftest_frame(k, 23, buf, sizeof buf, &len) == C5L8_OK && len == 142, "selftest %u", k);
        CHECK(buf[19] == (k == 0 ? 0x10 : 0x30) && le32(&buf[8]) == 5240, "selftest %u flags/freq", k);
    }
    {
        uint8_t buf[C5L8_FRAME_BYTES]; size_t len = 5;
        CHECK(c5l8_selftest_frame(C5L8_SELFTEST_CASES, 23, buf, sizeof buf, &len) == C5L8_REJECT_BUFFER && len == 0,
              "selftest out of range");
    }

    if (failures) { printf("c5_lltf8: %d failure(s)\n", failures); return 1; }
    printf("c5_lltf8: all checks passed\n");
    return 0;
}
