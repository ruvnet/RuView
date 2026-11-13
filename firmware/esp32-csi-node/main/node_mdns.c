/**
 * @file node_mdns.c
 * @brief mDNS advertisement of a CSI node. See node_mdns.h.
 */
#include "node_mdns.h"

#include <stdio.h>
#include <string.h>

size_t node_mdns_format_hostname(const uint8_t mac[6], char *out, size_t out_len)
{
    if (!mac || !out) return 0;
    int n = snprintf(out, out_len, "ruview-node-%02x%02x%02x", mac[3], mac[4], mac[5]);
    if (n <= 0 || (size_t)n >= out_len) {
        if (out_len) out[0] = '\0';
        return 0;
    }
    return (size_t)n;
}

size_t node_mdns_format_target(const char *ip, uint16_t port, char *out, size_t out_len)
{
    if (!out || out_len == 0) return 0;
    out[0] = '\0';
    if (!ip || ip[0] == '\0' || port == 0) return 0;
    int n = snprintf(out, out_len, "%s:%u", ip, (unsigned)port);
    if (n <= 0 || (size_t)n >= out_len) {
        out[0] = '\0';
        return 0;
    }
    return (size_t)n;
}

#ifndef NODE_MDNS_HOST_TEST  /* host tests exercise only the pure formatters */

#include "sdkconfig.h"
#include "esp_log.h"
#include "esp_mac.h"
#include "esp_app_desc.h"
#include "mdns.h"

static const char *TAG = "node_mdns";

esp_err_t node_mdns_start(uint8_t node_id, const char *target_ip, uint16_t target_port,
                          uint16_t service_port)
{
    uint8_t mac[6];
    esp_err_t err = esp_read_mac(mac, ESP_MAC_WIFI_STA);
    if (err != ESP_OK) return err;

    char host[32];
    if (node_mdns_format_hostname(mac, host, sizeof(host)) == 0) return ESP_ERR_INVALID_SIZE;

    err = mdns_init();
    if (err != ESP_OK) {
        ESP_LOGW(TAG, "mdns_init failed: %s (node stays undiscoverable)", esp_err_to_name(err));
        return err;
    }
    mdns_hostname_set(host);

    char instance[64]; /* "RuView node 255 (" + 31-char host + ")" fits */
    snprintf(instance, sizeof(instance), "RuView node %u (%s)", (unsigned)node_id, host);
    mdns_instance_name_set(instance);

    char id_s[4], target_s[48];
    snprintf(id_s, sizeof(id_s), "%u", (unsigned)node_id);
    if (node_mdns_format_target(target_ip, target_port, target_s, sizeof(target_s)) == 0) {
        strcpy(target_s, "unset");
    }
    const esp_app_desc_t *app = esp_app_get_description();

    mdns_txt_item_t txt[] = {
        {"node_id",    id_s},
        {"fw",         app->version},
        {"chip",       CONFIG_IDF_TARGET},
        {"csi_target", target_s},
    };
    err = mdns_service_add(instance, NODE_MDNS_SERVICE, NODE_MDNS_PROTO, service_port,
                           txt, sizeof(txt) / sizeof(txt[0]));
    if (err != ESP_OK) {
        ESP_LOGW(TAG, "mdns_service_add failed: %s", esp_err_to_name(err));
        return err;
    }
    ESP_LOGI(TAG, "advertising %s.local %s.%s port %u (node_id=%u csi_target=%s)",
             host, NODE_MDNS_SERVICE, NODE_MDNS_PROTO, (unsigned)service_port,
             (unsigned)node_id, target_s);
    return ESP_OK;
}

#endif /* NODE_MDNS_HOST_TEST */
