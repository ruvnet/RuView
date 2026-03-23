/**
 * @file nvs_config.h
 * @brief Runtime configuration via NVS (Non-Volatile Storage).
 *
 * Reads WiFi credentials and aggregator target from NVS.
 * Falls back to compile-time Kconfig defaults if NVS keys are absent.
 * This allows a single firmware binary to be shipped and configured
 * per-device using the provisioning script.
 */

#ifndef NVS_CONFIG_H
#define NVS_CONFIG_H

#include <stdint.h>

/** Maximum lengths for NVS string fields. */
#define NVS_CFG_SSID_MAX     33
#define NVS_CFG_PASS_MAX     65
#define NVS_CFG_IP_MAX       16

/** Maximum channels in the hop list (must match CSI_HOP_CHANNELS_MAX). */
#define NVS_CFG_HOP_MAX      6

/** Runtime configuration loaded from NVS or Kconfig defaults. */
typedef struct {
    char     wifi_ssid[NVS_CFG_SSID_MAX];
    char     wifi_password[NVS_CFG_PASS_MAX];
    char     target_ip[NVS_CFG_IP_MAX];
    uint16_t target_port;
    uint8_t  node_id;

    /* ADR-029: Channel hopping and TDM configuration */
    uint8_t  channel_hop_count;               /**< Number of channels to hop (1 = no hop). */
    uint8_t  channel_list[NVS_CFG_HOP_MAX];   /**< Channel numbers for hopping. */
    uint32_t dwell_ms;                        /**< Dwell time per channel in ms. */
    uint8_t  tdm_slot_index;                  /**< This node's TDM slot index (0-based). */
    uint8_t  tdm_node_count;                  /**< Total nodes in the TDM schedule. */

    /* ADR-039: Edge intelligence configuration */
    uint8_t  edge_tier;                       /**< Processing tier (0=raw, 1=basic, 2=full). */
    float    presence_thresh;                 /**< Presence threshold (0 = auto-calibrate). */
    float    fall_thresh;                     /**< Fall detection threshold (rad/s^2). */
    uint16_t vital_window;                    /**< Phase history window for BPM. */
    uint16_t vital_interval_ms;              /**< Vitals packet interval (ms). */
    uint8_t  top_k_count;                    /**< Number of top subcarriers to track. */
    uint8_t  power_duty;                     /**< Power duty cycle (10-100%). */

    /* ADR-040: WASM programmable sensing configuration */
    uint8_t  wasm_max_modules;               /**< Max concurrent WASM modules (1-8). */
    uint8_t  wasm_verify;                    /**< Require Ed25519 signature for uploads. */
    uint8_t  wasm_pubkey[32];               /**< Ed25519 public key for WASM signature. */
    uint8_t  wasm_pubkey_valid;             /**< 1 if pubkey was loaded from NVS. */

    /* ADR-060: Channel override and MAC address filtering */
    uint8_t  csi_channel;                    /**< Explicit CSI channel override (0 = auto-detect). */
    uint8_t  filter_mac[6];                  /**< MAC address to filter CSI frames. */
    uint8_t  filter_mac_set;                 /**< 1 if filter_mac was loaded from NVS. */

    /* Generic Utility Settings */
    uint8_t  status_led;                     /**< 1 to enable the RGB status LED, 0 to disable. */

    /* ADR-066: Swarm bridge configuration */
    char     seed_url[64];                /**< Cognitum Seed base URL (empty = disabled). */
    char     seed_token[64];             /**< Seed Bearer token (from pairing). */
    char     zone_name[16];              /**< Zone name for this node (e.g. "lobby"). */
    uint16_t swarm_heartbeat_sec;        /**< Heartbeat interval (seconds, default 30). */
    uint16_t swarm_ingest_sec;           /**< Vector ingest interval (seconds, default 5). */
} nvs_config_t;

/**
 * Load configuration from NVS, falling back to Kconfig defaults.
 *
 * Must be called after nvs_flash_init().
 *
 * @param cfg  Output configuration struct.
 */
void nvs_config_load(nvs_config_t *cfg);

#endif /* NVS_CONFIG_H */
