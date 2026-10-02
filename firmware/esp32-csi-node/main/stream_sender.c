/**
 * @file stream_sender.c
 * @brief UDP stream sender for CSI frames.
 *
 * Opens a UDP socket and sends serialized ADR-018 frames to the aggregator.
 */

#include "stream_sender.h"

#include <string.h>
#include "esp_log.h"
#include "esp_timer.h"
#include "lwip/sockets.h"
#include "lwip/netdb.h"
#include "sdkconfig.h"

static const char *TAG = "stream_sender";

static int s_sock = -1;
static struct sockaddr_in s_dest_addr;

/**
 * Send-failure backoff state.
 * When sendto fails for any reason, we suppress further sends for a cooldown
 * period to let lwIP/WiFi drain whatever is causing the failure. Originally
 * ENOMEM-only; broadened to any errno (docs/ADR-360-crash-fix-plan-2026-09-22.md)
 * after fleet nodes at low CSI_UPLINK_BATCH_SIZE hit a sustained EIO flood
 * (errno 5) that fell through this gate entirely and crashed on an unthrottled
 * retry path. Without this, rapid-fire CSI callbacks can exhaust the pbuf pool
 * or hammer a failing socket and crash the device.
 */
static int64_t s_backoff_until_us = 0;       /* esp_timer timestamp to resume */
#define SEND_FAIL_COOLDOWN_MS  100           /* base backoff; doubles per streak */
#define SEND_FAIL_COOLDOWN_MAX_MS 2000       /* cap on the exponential backoff */
#define SEND_FAIL_LOG_INTERVAL 50            /* log every Nth suppressed send */
static uint32_t s_send_failure_suppressed = 0;
/* Consecutive send-failure episodes without an intervening successful send. A
 * fixed 100 ms backoff is too short to drain sustained lwIP/WiFi buffer
 * pressure (#1135 bug #1: tier-2 + concurrent TX keeps the node stuck), so the
 * backoff grows 100→200→400→…→2000 ms per streak and resets on the first send
 * that succeeds. */
static uint32_t s_send_failure_streak = 0;

/* Timestamp of the last sendto() the stack accepted, on either path; 0 until
 * the node has ever delivered anything. Read by the uplink watchdog in main.c
 * -- see the rationale there for why "accepted by the stack" is deliberately
 * not "received by the server". */
static int64_t s_last_success_us = 0;

static int sender_init_internal(const char *ip, uint16_t port)
{
    s_sock = socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
    if (s_sock < 0) {
        ESP_LOGE(TAG, "Failed to create socket: errno %d", errno);
        return -1;
    }

    memset(&s_dest_addr, 0, sizeof(s_dest_addr));
    s_dest_addr.sin_family = AF_INET;
    s_dest_addr.sin_port = htons(port);

    if (inet_pton(AF_INET, ip, &s_dest_addr.sin_addr) <= 0) {
        ESP_LOGE(TAG, "Invalid target IP: %s", ip);
        close(s_sock);
        s_sock = -1;
        return -1;
    }

    ESP_LOGI(TAG, "UDP sender initialized: %s:%d", ip, port);
    return 0;
}

int stream_sender_init(void)
{
    return sender_init_internal(CONFIG_CSI_TARGET_IP, CONFIG_CSI_TARGET_PORT);
}

int stream_sender_init_with(const char *ip, uint16_t port)
{
    return sender_init_internal(ip, port);
}

int stream_sender_send(const uint8_t *data, size_t len)
{
    if (s_sock < 0) {
        return -1;
    }

    /* Send-failure backoff: if a recent sendto failed, skip sends until the
     * cooldown expires. This prevents the cascade of failed sendto calls
     * that leads to a guru meditation crash. */
    if (s_backoff_until_us > 0) {
        int64_t now = esp_timer_get_time();
        if (now < s_backoff_until_us) {
            s_send_failure_suppressed++;
            if ((s_send_failure_suppressed % SEND_FAIL_LOG_INTERVAL) == 1) {
                ESP_LOGW(TAG, "sendto suppressed (failure backoff, %lu dropped)",
                         (unsigned long)s_send_failure_suppressed);
            }
            return -1;
        }
        /* Cooldown expired — resume sending */
        ESP_LOGI(TAG, "send-failure backoff expired, resuming sends (%lu were suppressed)",
                 (unsigned long)s_send_failure_suppressed);
        s_backoff_until_us = 0;
        s_send_failure_suppressed = 0;
    }

    int sent = sendto(s_sock, data, len, 0,
                      (struct sockaddr *)&s_dest_addr, sizeof(s_dest_addr));
    if (sent < 0) {
        /* Exponential backoff on ANY sendto failure, not just ENOMEM: double
         * the cooldown each consecutive failure (capped) so sustained
         * pressure actually drains instead of the node re-failing every
         * 100 ms forever (#1135 bug #1), regardless of which errno is behind
         * it (docs/ADR-360-crash-fix-plan-2026-09-22.md — an EIO flood fell
         * through the old ENOMEM-only gate entirely and crashed on an
         * unthrottled retry path). */
        uint32_t shift = s_send_failure_streak < 5 ? s_send_failure_streak : 5;
        uint32_t cooldown = SEND_FAIL_COOLDOWN_MS << shift;
        if (cooldown > SEND_FAIL_COOLDOWN_MAX_MS) cooldown = SEND_FAIL_COOLDOWN_MAX_MS;
        s_send_failure_streak++;
        s_backoff_until_us = esp_timer_get_time() + (int64_t)cooldown * 1000;
        ESP_LOGW(TAG, "sendto failed: errno %d — backing off for %lu ms (streak %lu)",
                 errno, (unsigned long)cooldown, (unsigned long)s_send_failure_streak);
        return -1;
    }

    /* A send got through — pressure cleared; reset the backoff streak. */
    s_send_failure_streak = 0;
    s_last_success_us = esp_timer_get_time();
    return sent;
}

int stream_sender_send_priority(const uint8_t *data, size_t len)
{
    if (s_sock < 0) {
        return -1;
    }

    /* Priority path (#1183): low-rate control packets (feature_state, HEALTH,
     * mesh sync) bypass the global ENOMEM backoff gate so the high-rate CSI
     * stream cannot starve them. These are ≤48 B at ≤1 Hz — negligible pbuf
     * pressure, so they won't re-trigger the crash cascade that the backoff
     * (driven by the 50 Hz CSI flood) exists to prevent.
     *
     * Crucially, an ENOMEM here is reported quietly and does NOT extend the
     * global streak/backoff: a tiny control packet failing is a symptom of
     * the bulk-stream pressure, not a cause, so it must not feed the cooldown
     * that suppresses the next CSI frame. Likewise a success does not reset
     * the streak — the bulk path owns that signal. */
    int sent = sendto(s_sock, data, len, 0,
                      (struct sockaddr *)&s_dest_addr, sizeof(s_dest_addr));
    if (sent < 0) {
        if (errno != ENOMEM) {
            ESP_LOGW(TAG, "priority sendto failed: errno %d", errno);
        }
        return -1;
    }
    /* Counts for the watchdog: the mesh sync packet rides this path at ~1 Hz,
     * and its getting through proves the node's network path is intact even
     * when the bulk CSI stream is being suppressed by the ENOMEM backoff. */
    s_last_success_us = esp_timer_get_time();
    return sent;
}

int64_t stream_sender_last_success_us(void)
{
    return s_last_success_us;
}

/* Consecutive send failures since the last accepted send, on the bulk path
 * only (stream_sender_send_priority() deliberately does not feed this — see
 * its own comment). Exposed so the OTA rollback confirmation gate
 * (docs/ADR-360-crash-fix-plan-2026-09-22.md) can require actual send-health,
 * not just wall-clock-plus-network-presence, before cancelling rollback. */
uint32_t stream_sender_failure_streak(void)
{
    return s_send_failure_streak;
}

void stream_sender_deinit(void)
{
    if (s_sock >= 0) {
        close(s_sock);
        s_sock = -1;
        ESP_LOGI(TAG, "UDP sender closed");
    }
}
