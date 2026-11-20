/* SYNTHETIC: exercise the production ping configuration with a fake AP. */
#include "esp_netif.h"
#include "ping/ping_sock.h"
#include <assert.h>

static esp_ping_config_t observed;
static unsigned sessions, starts;
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
    assert(callbacks->on_ping_timeout != NULL);
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
#define esp_netif_get_handle_from_ifkey test_netif
#define esp_netif_get_ip_info test_ip
#define esp_ping_new_session test_session
#define esp_ping_start test_start
#include "../main/csi_collector.c"

nvs_config_t g_nvs_config;
/* csi_start_self_ping() hands RTTs to the link watchdog (ADR-427); this test
 * only checks the session config, so the callbacks are no-ops. */
void link_watchdog_ping_success_cb(esp_ping_handle_t hdl, void *args) { (void)hdl; (void)args; }
void link_watchdog_ping_timeout_cb(esp_ping_handle_t hdl, void *args) { (void)hdl; (void)args; }

int main(void)
{
    csi_start_self_ping();
    assert(sessions == 1 && starts == 1);
    assert(observed.interval_ms == CSI_SELF_PING_INTERVAL_MS);
    /* CSI_SELF_PING_TIMEOUT_MS (200 ms default) floored at one interval:
     * long enough to measure a slow-but-working gateway, short enough that a
     * lost reply does not stall the probes for the IDF default 1 s. */
    assert(observed.timeout_ms ==
           ping_timeout_resolve(observed.interval_ms, CONFIG_CSI_SELF_PING_TIMEOUT_MS));
    assert(observed.timeout_ms >= observed.interval_ms);
    assert(observed.timeout_ms < 1000);
    assert(observed.count == ESP_PING_COUNT_INFINITE);
    csi_start_self_ping();
    assert(sessions == 1 && starts == 1);
    puts("PASS: production ping timeout is bounded (200 ms default, floored at one interval)");
    return 0;
}
