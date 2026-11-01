/**
 * @file traffic_probe.c
 * @brief Unicast UDP CSI traffic source (#2287). See traffic_probe.h.
 *
 * One UDP socket, connect()ed to gateway:53 so the stack only delivers
 * datagrams from the gateway, and one low-priority task that sends a DNS
 * query every interval and discards the replies. The replies are the point:
 * each one is a unicast frame from the AP that the CSI engine can measure.
 */

#include "traffic_probe.h"

#include <errno.h>
#include <string.h>
#include "esp_log.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "lwip/sockets.h"
#include "sdkconfig.h"

static const char *TAG = "traffic_probe";

/* First report after this long, then every TRAFFIC_PROBE_REPORT_MS. The first
 * one is what a user debugging low CSI yield needs: does the gateway answer? */
#define TRAFFIC_PROBE_FIRST_REPORT_MS 10000U
#define TRAFFIC_PROBE_REPORT_MS       600000U

static int s_sock = -1;
static TaskHandle_t s_task = NULL;
static uint32_t s_interval_ms;
static uint8_t s_query[TRAFFIC_PROBE_QUERY_MAX];
static size_t s_query_len;
static uint32_t s_sent, s_send_err, s_replies;

/* Discard every reply already queued. Bounded by what the socket holds. */
static void traffic_probe_drain(void)
{
    uint8_t buf[64];   /* replies are discarded; truncating them is fine */
    while (recv(s_sock, buf, sizeof(buf), MSG_DONTWAIT) >= 0) {
        s_replies++;
    }
}

static void traffic_probe_task(void *arg)
{
    (void)arg;
    uint16_t id = 0;
    uint32_t since_report_ms = 0;
    uint32_t report_every_ms = TRAFFIC_PROBE_FIRST_REPORT_MS;
    /* At a FreeRTOS tick rate below 1000/interval Hz this would round to 0
     * and spin; never wait less than one tick. */
    TickType_t period = pdMS_TO_TICKS(s_interval_ms);
    if (period == 0) {
        period = 1;
    }
    TickType_t wake = xTaskGetTickCount();

    for (;;) {
        vTaskDelayUntil(&wake, period);
        traffic_probe_drain();

        s_query[0] = (uint8_t)(id >> 8);
        s_query[1] = (uint8_t)id;
        id++;
        if (send(s_sock, s_query, s_query_len, 0) < 0) {
            s_send_err++;
        } else {
            s_sent++;
        }

        since_report_ms += s_interval_ms;
        if (since_report_ms >= report_every_ms) {
            if (s_replies == 0) {
                ESP_LOGW(TAG, "no DNS replies from the gateway after %u queries: it may "
                              "not answer DNS, so this probe adds no CSI (#2287)",
                         (unsigned)s_sent);
            } else {
                ESP_LOGI(TAG, "sent=%u replies=%u send_err=%u",
                         (unsigned)s_sent, (unsigned)s_replies, (unsigned)s_send_err);
            }
            since_report_ms = 0;
            report_every_ms = TRAFFIC_PROBE_REPORT_MS;
        }
    }
}

bool traffic_probe_start(uint32_t gw_addr, uint32_t interval_ms, const char *qname)
{
    if (s_task != NULL) {
        return true;   /* already running */
    }
    if (gw_addr == 0 || interval_ms == 0) {
        return false;
    }

    s_query_len = traffic_probe_build_dns_query(s_query, sizeof(s_query), 0, qname);
    if (s_query_len == 0) {
        ESP_LOGE(TAG, "invalid DNS probe name \"%s\"", qname ? qname : "(null)");
        return false;
    }

    s_sock = socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
    if (s_sock < 0) {
        ESP_LOGE(TAG, "socket() failed: errno %d", errno);
        return false;
    }
    struct sockaddr_in gw = {
        .sin_family = AF_INET,
        .sin_port = htons(TRAFFIC_PROBE_DNS_PORT),
        .sin_addr.s_addr = gw_addr,
    };
    if (connect(s_sock, (struct sockaddr *)&gw, sizeof(gw)) != 0) {
        ESP_LOGE(TAG, "connect() failed: errno %d", errno);
        close(s_sock);
        s_sock = -1;
        return false;
    }

    s_interval_ms = interval_ms;
    if (xTaskCreate(traffic_probe_task, "traffic_probe", 3072, NULL,
                    tskIDLE_PRIORITY + 2, &s_task) != pdPASS) {
        ESP_LOGE(TAG, "task create failed");
        s_task = NULL;
        close(s_sock);
        s_sock = -1;
        return false;
    }

    char gw_str[16];
    inet_ntoa_r(gw.sin_addr, gw_str, sizeof(gw_str));
    ESP_LOGI(TAG, "UDP DNS probe started -> %s:%u every %u ms, name \"%s\" (#2287)",
             gw_str, (unsigned)TRAFFIC_PROBE_DNS_PORT, (unsigned)interval_ms, qname);
    return true;
}
