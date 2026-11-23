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

int main(void)
{
    csi_start_self_ping();
    assert(sessions == 1 && starts == 1);
    assert(observed.interval_ms == CSI_SELF_PING_INTERVAL_MS);
    assert(observed.timeout_ms == observed.interval_ms);
    assert(observed.count == ESP_PING_COUNT_INFINITE);
    csi_start_self_ping();
    assert(sessions == 1 && starts == 1);
    puts("PASS: production ping timeout is bounded to one interval");
    return 0;
}
