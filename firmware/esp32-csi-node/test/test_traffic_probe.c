/* SYNTHETIC: the #2287 UDP DNS probe -- query bytes, and the ICMP/UDP switch
 * in csi_start_self_ping() with CONFIG_CSI_TRAFFIC_SOURCE_UDP_DNS enabled. */
#define CONFIG_CSI_TRAFFIC_SOURCE_UDP_DNS 1
#define CONFIG_CSI_TRAFFIC_DNS_QNAME "localhost"

#include "esp_netif.h"
#include "ping/ping_sock.h"
#include <assert.h>
#include <stdbool.h>
#include <stdint.h>
#include <string.h>

/* ---- fakes for the switch ---- */
static esp_ping_config_t observed;
static unsigned sessions, starts, probe_calls;
static bool probe_result;
static uint32_t probe_gw, probe_interval;
static const char *probe_name;

static esp_netif_t *test_netif(const char *key)
{
    assert(strcmp(key, "WIFI_STA_DEF") == 0);
    return (esp_netif_t *)&observed;
}
static esp_err_t test_ip(esp_netif_t *netif, esp_netif_ip_info_t *ip)
{
    assert(netif != NULL);
    ip->gw.addr = 0x0101a8c0;
    return ESP_OK;
}
static esp_err_t test_session(const esp_ping_config_t *config,
                             const esp_ping_callbacks_t *callbacks,
                             esp_ping_handle_t *handle)
{
    (void)callbacks;
    observed = *config;
    sessions++;
    *handle = &observed;
    return ESP_OK;
}
static esp_err_t test_start(esp_ping_handle_t handle)
{
    assert(handle == &observed);
    starts++;
    return ESP_OK;
}
bool traffic_probe_start(uint32_t gw_addr, uint32_t interval_ms, const char *qname)
{
    probe_calls++;
    probe_gw = gw_addr;
    probe_interval = interval_ms;
    probe_name = qname;
    return probe_result;
}
#define esp_netif_get_handle_from_ifkey test_netif
#define esp_netif_get_ip_info test_ip
#define esp_ping_new_session test_session
#define esp_ping_start test_start
#include "../main/csi_collector.c"

nvs_config_t g_nvs_config;

static void test_query_bytes(void)
{
    uint8_t buf[TRAFFIC_PROBE_QUERY_MAX];
    static const uint8_t want[] = {
        0x12, 0x34,             /* id */
        0x00, 0x00,             /* standard query, RD clear */
        0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        9, 'l', 'o', 'c', 'a', 'l', 'h', 'o', 's', 't', 0,
        0x00, 0x01, 0x00, 0x01, /* A, IN */
    };
    size_t n = traffic_probe_build_dns_query(buf, sizeof(buf), 0x1234, "localhost");
    assert(n == sizeof(want) && memcmp(buf, want, n) == 0);

    /* Trailing dot is the same name; multi-label encodes each label. */
    uint8_t buf2[TRAFFIC_PROBE_QUERY_MAX];
    assert(traffic_probe_build_dns_query(buf2, sizeof(buf2), 0x1234, "localhost.") == n);
    assert(memcmp(buf, buf2, n) == 0);
    static const uint8_t ab[] = { 1, 'a', 2, 'b', '_', 0 };
    n = traffic_probe_build_dns_query(buf, sizeof(buf), 0, "a.b_");
    assert(n == 12 + sizeof(ab) + 4 && memcmp(buf + 12, ab, sizeof(ab)) == 0);
    /* Exact fit, and one byte short. */
    assert(traffic_probe_build_dns_query(buf, n, 0, "a.b_") == n);
    assert(traffic_probe_build_dns_query(buf, n - 1, 0, "a.b_") == 0);
}

static void test_query_rejects(void)
{
    uint8_t buf[TRAFFIC_PROBE_QUERY_MAX];
    char name[300];
    assert(traffic_probe_build_dns_query(buf, sizeof(buf), 0, "") == 0);
    assert(traffic_probe_build_dns_query(buf, sizeof(buf), 0, ".") == 0);
    assert(traffic_probe_build_dns_query(buf, sizeof(buf), 0, "a..b") == 0);
    assert(traffic_probe_build_dns_query(buf, sizeof(buf), 0, ".a") == 0);
    assert(traffic_probe_build_dns_query(buf, sizeof(buf), 0, "a b") == 0);
    assert(traffic_probe_build_dns_query(buf, sizeof(buf), 0, "a/b") == 0);
    assert(traffic_probe_build_dns_query(buf, sizeof(buf), 0, NULL) == 0);
    assert(traffic_probe_build_dns_query(NULL, sizeof(buf), 0, "a") == 0);

    memset(name, 'x', 63); name[63] = '\0';
    assert(traffic_probe_build_dns_query(buf, sizeof(buf), 0, name) != 0);
    memset(name, 'x', 64); name[64] = '\0';
    assert(traffic_probe_build_dns_query(buf, sizeof(buf), 0, name) == 0);

    /* 253 bytes is the longest name; 254 is refused. Labels of 49 + dots. */
    for (int i = 0; i < 253; i++) name[i] = (i % 50 == 49) ? '.' : 'x';
    name[253] = '\0';
    size_t n = traffic_probe_build_dns_query(buf, sizeof(buf), 0, name);
    assert(n == 12 + 253 + 2 + 4 && n <= sizeof(buf));
    name[253] = 'x'; name[254] = '\0';
    assert(traffic_probe_build_dns_query(buf, sizeof(buf), 0, name) == 0);
}

static void test_switch(void)
{
    /* Probe starts: no ICMP session. */
    probe_result = true;
    csi_start_self_ping();
    assert(probe_calls == 1 && sessions == 0 && starts == 0);
    assert(probe_gw == 0x0101a8c0);
    assert(probe_interval == CSI_SELF_PING_INTERVAL_MS);
    assert(strcmp(probe_name, "localhost") == 0);

    /* Probe refuses: falls back to the ICMP session, once. */
    probe_result = false;
    csi_start_self_ping();
    assert(probe_calls == 2 && sessions == 1 && starts == 1);
    assert(observed.interval_ms == CSI_SELF_PING_INTERVAL_MS);
    csi_start_self_ping();
    assert(probe_calls == 2 && sessions == 1 && starts == 1);
}

int main(void)
{
    test_query_bytes();
    test_query_rejects();
    test_switch();
    puts("PASS: UDP DNS probe query and ICMP fallback (#2287)");
    return 0;
}
