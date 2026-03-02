/**
 * @file nvs_config.c
 * @brief Runtime configuration via NVS (Non-Volatile Storage).
 *
 * Checks NVS namespace "csi_cfg" for keys: ssid, password, target_ip,
 * target_port, node_id.  Falls back to Kconfig defaults when absent.
 */

#include "nvs_config.h"

#include <string.h>
#include "esp_log.h"
#include "nvs_flash.h"
#include "nvs.h"
#include "sdkconfig.h"

static const char *TAG = "nvs_config";

void nvs_config_load(nvs_config_t *cfg)
{
    if (cfg == NULL) {
        ESP_LOGE(TAG, "nvs_config_load: cfg is NULL");
        return;
    }

    /* Start with Kconfig compiled defaults */
    strncpy(cfg->wifi_ssid, CONFIG_CSI_WIFI_SSID, NVS_CFG_SSID_MAX - 1);
    cfg->wifi_ssid[NVS_CFG_SSID_MAX - 1] = '\0';

#ifdef CONFIG_CSI_WIFI_PASSWORD
    strncpy(cfg->wifi_password, CONFIG_CSI_WIFI_PASSWORD, NVS_CFG_PASS_MAX - 1);
    cfg->wifi_password[NVS_CFG_PASS_MAX - 1] = '\0';
#else
    cfg->wifi_password[0] = '\0';
#endif

    strncpy(cfg->target_ip, CONFIG_CSI_TARGET_IP, NVS_CFG_IP_MAX - 1);
    cfg->target_ip[NVS_CFG_IP_MAX - 1] = '\0';

    cfg->target_port = (uint16_t)CONFIG_CSI_TARGET_PORT;
    cfg->node_id     = (uint8_t)CONFIG_CSI_NODE_ID;

    /* ADR-029: Defaults for channel hopping and TDM.
     * hop_count=1 means single-channel (backward-compatible). */
    cfg->channel_hop_count = 1;
    cfg->channel_list[0]   = (uint8_t)CONFIG_CSI_WIFI_CHANNEL;
    for (uint8_t i = 1; i < NVS_CFG_HOP_MAX; i++) {
        cfg->channel_list[i] = 0;
    }
    cfg->dwell_ms       = 50;
    cfg->tdm_slot_index = 0;
    cfg->tdm_node_count = 1;

    /* Try to override from NVS */
    nvs_handle_t handle;
    esp_err_t err = nvs_open("csi_cfg", NVS_READONLY, &handle);
    if (err != ESP_OK) {
        ESP_LOGI(TAG, "No NVS config found, using compiled defaults");
        return;
    }

    size_t len;
    char buf[NVS_CFG_PASS_MAX];

    /* WiFi SSID */
    len = sizeof(buf);
    if (nvs_get_str(handle, "ssid", buf, &len) == ESP_OK && len > 1) {
        strncpy(cfg->wifi_ssid, buf, NVS_CFG_SSID_MAX - 1);
        cfg->wifi_ssid[NVS_CFG_SSID_MAX - 1] = '\0';
        ESP_LOGI(TAG, "NVS override: ssid=%s", cfg->wifi_ssid);
    }

    /* WiFi password */
    len = sizeof(buf);
    if (nvs_get_str(handle, "password", buf, &len) == ESP_OK) {
        strncpy(cfg->wifi_password, buf, NVS_CFG_PASS_MAX - 1);
        cfg->wifi_password[NVS_CFG_PASS_MAX - 1] = '\0';
        ESP_LOGI(TAG, "NVS override: password=***");
    }

    /* Target IP */
    len = sizeof(buf);
    if (nvs_get_str(handle, "target_ip", buf, &len) == ESP_OK && len > 1) {
        strncpy(cfg->target_ip, buf, NVS_CFG_IP_MAX - 1);
        cfg->target_ip[NVS_CFG_IP_MAX - 1] = '\0';
        ESP_LOGI(TAG, "NVS override: target_ip=%s", cfg->target_ip);
    }

    /* Target port */
    uint16_t port_val;
    if (nvs_get_u16(handle, "target_port", &port_val) == ESP_OK) {
        cfg->target_port = port_val;
        ESP_LOGI(TAG, "NVS override: target_port=%u", cfg->target_port);
    }

    /* Node ID */
    uint8_t node_val;
    if (nvs_get_u8(handle, "node_id", &node_val) == ESP_OK) {
        cfg->node_id = node_val;
        ESP_LOGI(TAG, "NVS override: node_id=%u", cfg->node_id);
    }

    /* ADR-029: Channel hop count */
    uint8_t hop_count_val;
    if (nvs_get_u8(handle, "hop_count", &hop_count_val) == ESP_OK) {
        if (hop_count_val >= 1 && hop_count_val <= NVS_CFG_HOP_MAX) {
            cfg->channel_hop_count = hop_count_val;
            ESP_LOGI(TAG, "NVS override: hop_count=%u", (unsigned)cfg->channel_hop_count);
        } else {
            ESP_LOGW(TAG, "NVS hop_count=%u out of range [1..%u], ignored",
                     (unsigned)hop_count_val, (unsigned)NVS_CFG_HOP_MAX);
        }
    }

    /* ADR-029: Channel list (stored as a blob of up to NVS_CFG_HOP_MAX bytes) */
    len = NVS_CFG_HOP_MAX;
    uint8_t ch_blob[NVS_CFG_HOP_MAX];
    if (nvs_get_blob(handle, "chan_list", ch_blob, &len) == ESP_OK && len > 0) {
        uint8_t count = (len < cfg->channel_hop_count) ? (uint8_t)len : cfg->channel_hop_count;
        for (uint8_t i = 0; i < count; i++) {
            cfg->channel_list[i] = ch_blob[i];
        }
        ESP_LOGI(TAG, "NVS override: chan_list loaded (%u channels)", (unsigned)count);
    }

    /* ADR-029: Dwell time */
    uint32_t dwell_val;
    if (nvs_get_u32(handle, "dwell_ms", &dwell_val) == ESP_OK) {
        if (dwell_val >= 10) {
            cfg->dwell_ms = dwell_val;
            ESP_LOGI(TAG, "NVS override: dwell_ms=%lu", (unsigned long)cfg->dwell_ms);
        } else {
            ESP_LOGW(TAG, "NVS dwell_ms=%lu too small, ignored", (unsigned long)dwell_val);
        }
    }

    /* ADR-029/031: TDM slot index */
    uint8_t slot_val;
    if (nvs_get_u8(handle, "tdm_slot", &slot_val) == ESP_OK) {
        cfg->tdm_slot_index = slot_val;
        ESP_LOGI(TAG, "NVS override: tdm_slot_index=%u", (unsigned)cfg->tdm_slot_index);
    }

    /* ADR-029/031: TDM node count */
    uint8_t tdm_nodes_val;
    if (nvs_get_u8(handle, "tdm_nodes", &tdm_nodes_val) == ESP_OK) {
        if (tdm_nodes_val >= 1) {
            cfg->tdm_node_count = tdm_nodes_val;
            ESP_LOGI(TAG, "NVS override: tdm_node_count=%u", (unsigned)cfg->tdm_node_count);
        } else {
            ESP_LOGW(TAG, "NVS tdm_nodes=%u invalid, ignored", (unsigned)tdm_nodes_val);
        }
    }

    /* Validate tdm_slot_index < tdm_node_count */
    if (cfg->tdm_slot_index >= cfg->tdm_node_count) {
        ESP_LOGW(TAG, "tdm_slot_index=%u >= tdm_node_count=%u, clamping to 0",
                 (unsigned)cfg->tdm_slot_index, (unsigned)cfg->tdm_node_count);
        cfg->tdm_slot_index = 0;
    }

    nvs_close(handle);
}
