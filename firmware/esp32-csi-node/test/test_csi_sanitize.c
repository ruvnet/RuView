/** Deterministic host test for ESP-IDF first_word_invalid sanitation. */

#include "esp_stubs.h"
#include "nvs_config.h"
#include "csi_collector.h"

#include <assert.h>
#include <stdio.h>
#include <string.h>

nvs_config_t g_nvs_config;

static void make_info(wifi_csi_info_t *info, int8_t *iq, uint16_t len, bool invalid)
{
    memset(info, 0, sizeof(*info));
    info->rx_ctrl.channel = 6;
    info->rx_ctrl.rssi = -42;
    info->rx_ctrl.noise_floor = -91;
    info->len = (int16_t)len;
    info->buf = iq;
    info->first_word_invalid = invalid;
}

int main(void)
{
    int8_t iq[] = {1, 2, 3, 4, 5, 6, 7, 8};
    uint8_t frame[CSI_MAX_FRAME_SIZE];
    wifi_csi_info_t info;

    make_info(&info, iq, sizeof(iq), true);
    size_t len = csi_serialize_frame(&info, frame, sizeof(frame));
    assert(len == CSI_HEADER_SIZE + sizeof(iq));
    assert(frame[CSI_HEADER_SIZE] == 0);
    assert(frame[CSI_HEADER_SIZE + 1] == 0);
    assert(frame[CSI_HEADER_SIZE + 2] == 0);
    assert(frame[CSI_HEADER_SIZE + 3] == 0);
    assert(frame[CSI_HEADER_SIZE + 4] == 5);
    assert((frame[19] & CSI_FLAG_FIRST_WORD_SANITIZED) != 0);

    make_info(&info, iq, sizeof(iq), false);
    len = csi_serialize_frame(&info, frame, sizeof(frame));
    assert(len == CSI_HEADER_SIZE + sizeof(iq));
    assert(memcmp(&frame[CSI_HEADER_SIZE], iq, sizeof(iq)) == 0);
    assert((frame[19] & CSI_FLAG_FIRST_WORD_SANITIZED) == 0);

    int8_t short_iq[] = {9, 10};
    make_info(&info, short_iq, sizeof(short_iq), true);
    len = csi_serialize_frame(&info, frame, sizeof(frame));
    assert(len == CSI_HEADER_SIZE + sizeof(short_iq));
    assert(frame[CSI_HEADER_SIZE] == 0);
    assert(frame[CSI_HEADER_SIZE + 1] == 0);
    assert((frame[19] & CSI_FLAG_FIRST_WORD_SANITIZED) != 0);

    puts("CSI invalid-prefix sanitation: PASS");
    return 0;
}
