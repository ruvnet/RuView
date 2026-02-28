/**
 * @file stream_sender.c
 * @brief UDP stream sender for CSI frames.
 *
 * Opens a UDP socket and sends serialized ADR-018 frames to the aggregator.
 */

#include "stream_sender.h"

#include <string.h>
#include "esp_log.h"
#include "lwip/sockets.h"
#include "lwip/netdb.h"
#include "sdkconfig.h"

static const char *TAG = "stream_sender";

static int s_sock = -1;
static struct sockaddr_in s_dest_addr;

int stream_sender_init(void)
{
    s_sock = socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
    if (s_sock < 0) {
        ESP_LOGE(TAG, "Failed to create socket: errno %d", errno);
        return -1;
    }

    memset(&s_dest_addr, 0, sizeof(s_dest_addr));
    s_dest_addr.sin_family = AF_INET;
    s_dest_addr.sin_port = htons(CONFIG_CSI_TARGET_PORT);

    if (inet_pton(AF_INET, CONFIG_CSI_TARGET_IP, &s_dest_addr.sin_addr) <= 0) {
        ESP_LOGE(TAG, "Invalid target IP: %s", CONFIG_CSI_TARGET_IP);
        close(s_sock);
        s_sock = -1;
        return -1;
    }

    ESP_LOGI(TAG, "UDP sender initialized: %s:%d", CONFIG_CSI_TARGET_IP, CONFIG_CSI_TARGET_PORT);
    return 0;
}

int stream_sender_send(const uint8_t *data, size_t len)
{
    if (s_sock < 0) {
        return -1;
    }

    int sent = sendto(s_sock, data, len, 0,
                      (struct sockaddr *)&s_dest_addr, sizeof(s_dest_addr));
    if (sent < 0) {
        ESP_LOGW(TAG, "sendto failed: errno %d", errno);
        return -1;
    }

    return sent;
}

void stream_sender_deinit(void)
{
    if (s_sock >= 0) {
        close(s_sock);
        s_sock = -1;
        ESP_LOGI(TAG, "UDP sender closed");
    }
}
