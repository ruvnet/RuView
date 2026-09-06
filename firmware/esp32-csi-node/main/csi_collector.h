/**
 * @file csi_collector.h
 * @brief CSI data collection and ADR-018 binary frame serialization.
 */

#ifndef CSI_COLLECTOR_H
#define CSI_COLLECTOR_H

#include <stdint.h>
#include <stddef.h>
#include "esp_err.h"
#include "esp_wifi_types.h"

/** ADR-018 magic number. */
/* Wire v1: 20-byte header, no transmitter identity. Retained so a mixed fleet
 * and older sinks keep parsing. */
#define CSI_MAGIC 0xC5110001

/* Wire v2: v1 header plus the 6-byte transmitter MAC (addr2) at bytes 20..25.
 *
 * Why: a node in promiscuous MGMT+DATA mode with no filter_mac produces CSI
 * for every transmitter on the channel — measured 2026-08-28 at ~75% non-AP in
 * a normal home. Each of those frames is a valid measurement of a *different*
 * link, but without the transmitter on the wire the sink cannot separate them,
 * so they interleave into one history with mixed geometry. Carrying addr2 lets
 * the sink split one stream into per-link streams, which is also what any
 * node-to-node ranging needs.
 *
 * 0xC5110002..0xC5110007 are already claimed by the vitals, feature and other
 * edge packets, so v2 takes the next free value rather than an adjacent one. */
#define CSI_MAGIC_V2 0xC5110008

/** ADR-018 header size in bytes (wire v1). */
#define CSI_HEADER_SIZE 20

/** Wire v2 header size: v1 plus the 6-byte transmitter MAC. */
#define CSI_HEADER_SIZE_V2 26

/** Wire v3 CSI frame magic: v2 plus the 802.11 receive sequence number.
 *
 * Why: v2 made frames attributable to a *link* (which transmitter), but not to
 * a *transmission*. Byte 12..15 carries `s_sequence`, a counter private to each
 * node, so two nodes that captured the same packet off the air report unrelated
 * numbers and the sink cannot tell it was one event observed twice. Fusing on
 * arrival timestamps instead fails because the sink sees network jitter, not
 * capture time.
 *
 * `wifi_csi_info_t.rx_seq` is the 802.11 sequence control field of the
 * overheard frame -- assigned by the TRANSMITTER, identical at every receiver.
 * (mac, rx_seq) is therefore a coordination-free name for "this specific
 * transmission", which is exactly the join key cross-node fusion needs.
 *
 * MEASURED 2026-08-30, two boards side by side logging (mac, rx_seq): 4099
 * distinct values over ~44,000 frames with both nodes reporting the identical
 * range, and 72% of frames heard by one were heard by the other. This settles
 * the open question in ADR-345 -- rx_seq is populated and usable.
 *
 * 0xC5110009 is the per-slot vitals packet, so v3 takes 0xC511000A. */
#define CSI_MAGIC_V3 0xC511000A

/** Wire v3 header size: v2 plus `rx_seq` (u16 LE) at bytes 26..27. */
#define CSI_HEADER_SIZE_V3 28

/** Maximum frame buffer size (largest header + 4 antennas * 256 subcarriers * 2 bytes). */
#define CSI_MAX_FRAME_SIZE (CSI_HEADER_SIZE_V3 + 4 * 256 * 2)

/** Maximum number of channels in the hop table (ADR-029). */
#define CSI_HOP_CHANNELS_MAX 6

/**
 * Initialize CSI collection.
 * Registers the WiFi CSI callback.
 */
void csi_collector_init(void);

/**
 * Capture node_id BEFORE wifi_init_sta() or any other heavy init.
 *
 * Must be called from app_main() immediately after nvs_config_load().
 * WiFi driver initialization can corrupt g_nvs_config.node_id (confirmed
 * on device 80:b5:4e:c1:be:b8, NVS=3 but post-WiFi reads as 1).
 * This early capture shields s_node_id from that corruption window.
 *
 * @param node_id Value from g_nvs_config.node_id, read right after NVS load.
 */
void csi_collector_set_node_id(uint8_t node_id);

/**
 * Get the runtime node_id (early capture if available, otherwise init-time).
 *
 * Other modules (edge_processing, wasm_runtime, display_ui) should prefer
 * this accessor over reading g_nvs_config.node_id directly.
 *
 * @return Node ID (0-255) as loaded from NVS at boot.
 */
uint8_t csi_collector_get_node_id(void);

/**
 * Serialize CSI data into ADR-018 binary frame format.
 *
 * @param info   WiFi CSI info from the ESP-IDF callback.
 * @param buf    Output buffer (must be at least CSI_MAX_FRAME_SIZE bytes).
 * @param buf_len Size of the output buffer.
 * @return Number of bytes written, or 0 on error.
 */
size_t csi_serialize_frame(const wifi_csi_info_t *info, uint8_t *buf, size_t buf_len);

/**
 * Configure the channel-hop table for multi-band sensing (ADR-029).
 *
 * When hop_count == 1 the collector stays on the single configured channel
 * (backward-compatible with the original single-channel mode).
 *
 * @param channels  Array of WiFi channel numbers (1-14 for 2.4 GHz, 36-177 for 5 GHz).
 * @param hop_count Number of entries in the channels array (1..CSI_HOP_CHANNELS_MAX).
 * @param dwell_ms  Dwell time per channel in milliseconds (>= 10).
 */
void csi_collector_set_hop_table(const uint8_t *channels, uint8_t hop_count, uint32_t dwell_ms);

/**
 * Advance to the next channel in the hop table.
 *
 * Called by the hop timer callback. If hop_count <= 1 this is a no-op.
 * Calls esp_wifi_set_channel() internally.
 */
void csi_hop_next_channel(void);

/**
 * Start the channel-hop timer.
 *
 * Creates an esp_timer periodic callback that fires every dwell_ms
 * milliseconds, calling csi_hop_next_channel(). If hop_count <= 1
 * the timer is not started (single-channel backward-compatible mode).
 */
void csi_collector_start_hop_timer(void);

/**
 * Upgrade the promiscuous filter to capture DATA frames in addition to MGMT
 * (RuView#893/#521).
 *
 * Called on display-less boards: the MGMT-only filter (the #396 display-crash
 * workaround set in csi_collector_init) only fires the CSI callback on sparse
 * management frames, so yield collapses to 0 pps under real traffic and the
 * node looks dead. A board with no AMOLED panel has no QSPI/SPI-flash cache
 * contention, so it can safely capture DATA frames — restoring abundant CSI.
 * Display boards keep MGMT-only to avoid the #396 crash.
 */
void csi_collector_enable_data_capture(void);

/**
 * Inject an NDP (Null Data Packet) frame for sensing.
 *
 * Uses esp_wifi_80211_tx() to send a preamble-only frame (~24 us airtime)
 * that triggers CSI measurement at all receivers. This is the "sensing-first"
 * TX mechanism described in ADR-029.
 *
 * @return ESP_OK on success, or an error code.
 *
 * @note TODO: Full NDP frame construction. Currently sends a minimal
 *       null-data frame as a placeholder.
 */
esp_err_t csi_inject_ndp_frame(void);

/**
 * Get the recent CSI callback rate (per second).
 *
 * Computed as a sliding 1-second window over the internal s_cb_count
 * counter. Used by the ADR-081 radio abstraction layer to fill the
 * pkt_yield_per_sec field of rv_radio_health_t.
 *
 * @return Callbacks observed in the trailing ~1 second.
 */
uint16_t csi_collector_get_pkt_yield_per_sec(void);

/**
 * Get the cumulative UDP send-failure counter since boot.
 *
 * @return Number of stream_sender_send() failures recorded by the
 *         CSI callback path.
 */
uint16_t csi_collector_get_send_fail_count(void);

#endif /* CSI_COLLECTOR_H */
