/**
 * @file esp_stubs.h
 * @brief Minimal ESP-IDF type stubs for host-based fuzz testing.
 *
 * Provides just enough type definitions and macros to compile
 * csi_collector.c and edge_processing.c on a Linux/macOS host
 * without the full ESP-IDF SDK.
 */

#ifndef ESP_STUBS_H
#define ESP_STUBS_H

#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>
#include <stdio.h>
#include <string.h>

/* ---- esp_err.h ---- */
typedef int esp_err_t;
#define ESP_OK          0
#define ESP_FAIL        (-1)
#define ESP_ERR_NO_MEM      0x101
#define ESP_ERR_INVALID_ARG 0x102
#define ESP_ERR_NOT_FOUND   0x105

/* ---- esp_log.h ---- */
#define ESP_LOGI(tag, fmt, ...)  ((void)0)
#define ESP_LOGW(tag, fmt, ...)  ((void)0)
#define ESP_LOGE(tag, fmt, ...)  ((void)0)
#define ESP_LOGD(tag, fmt, ...)  ((void)0)
#define ESP_ERROR_CHECK(x)       ((void)(x))

/* ---- esp_timer.h ---- */
typedef void *esp_timer_handle_t;

/** Timer callback type (matches ESP-IDF signature). */
typedef void (*esp_timer_cb_t)(void *arg);

/** Timer creation arguments (matches ESP-IDF esp_timer_create_args_t). */
typedef struct {
    esp_timer_cb_t callback;
    void          *arg;
    const char    *name;
} esp_timer_create_args_t;

/**
 * Stub: returns a monotonically increasing microsecond counter.
 * Declared here, defined in esp_stubs.c.
 */
int64_t esp_timer_get_time(void);

/** Stub: timer lifecycle (no-ops for fuzz testing). */
static inline esp_err_t esp_timer_create(const esp_timer_create_args_t *args, esp_timer_handle_t *h) {
    (void)args; if (h) *h = (void *)1; return ESP_OK;
}
static inline esp_err_t esp_timer_start_periodic(esp_timer_handle_t h, uint64_t period) {
    (void)h; (void)period; return ESP_OK;
}
static inline esp_err_t esp_timer_stop(esp_timer_handle_t h) { (void)h; return ESP_OK; }
static inline esp_err_t esp_timer_delete(esp_timer_handle_t h) { (void)h; return ESP_OK; }

/* ---- esp_wifi_types.h ---- */

/** Minimal rx_ctrl fields needed by csi_serialize_frame. */
typedef struct {
    signed   rssi        : 8;
    unsigned channel     : 4;
    unsigned noise_floor : 8;
    unsigned rx_ant      : 2;
    /* Padding to fill out the struct so it compiles. */
    unsigned _pad        : 10;
} wifi_pkt_rx_ctrl_t;

/** Minimal wifi_csi_info_t needed by csi_serialize_frame. */
typedef struct {
    wifi_pkt_rx_ctrl_t rx_ctrl;
    uint8_t            mac[6];
    int16_t            len;     /**< Length of the I/Q buffer in bytes. */
    int8_t            *buf;     /**< Pointer to I/Q data. */
} wifi_csi_info_t;

/* ---- Kconfig defaults ---- */
#ifndef CONFIG_CSI_NODE_ID
#define CONFIG_CSI_NODE_ID  1
#endif

#ifndef CONFIG_CSI_WIFI_CHANNEL
#define CONFIG_CSI_WIFI_CHANNEL  6
#endif

#ifndef CONFIG_CSI_WIFI_SSID
#define CONFIG_CSI_WIFI_SSID  "test_ssid"
#endif

#ifndef CONFIG_CSI_TARGET_IP
#define CONFIG_CSI_TARGET_IP  "192.168.1.1"
#endif

#ifndef CONFIG_CSI_TARGET_PORT
#define CONFIG_CSI_TARGET_PORT  5500
#endif

/* Suppress the build-time guard in csi_collector.c */
#ifndef CONFIG_ESP_WIFI_CSI_ENABLED
#define CONFIG_ESP_WIFI_CSI_ENABLED 1
#endif

/* ---- sdkconfig.h stub ---- */
/* (empty — all needed CONFIG_ macros are above) */

/* ---- FreeRTOS stubs ---- */
#define pdMS_TO_TICKS(x) ((x))
#define pdPASS  1
typedef int BaseType_t;

static inline int xPortGetCoreID(void) { return 0; }
static inline void vTaskDelay(uint32_t ticks) { (void)ticks; }
static inline BaseType_t xTaskCreatePinnedToCore(
    void (*fn)(void *), const char *name, uint32_t stack,
    void *arg, int prio, void *handle, int core)
{
    (void)fn; (void)name; (void)stack; (void)arg;
    (void)prio; (void)handle; (void)core;
    return pdPASS;
}

/* ---- WiFi API stubs (no-ops) ---- */
typedef int wifi_interface_t;
typedef int wifi_second_chan_t;
#define WIFI_IF_STA  0
#define WIFI_SECOND_CHAN_NONE  0

typedef struct {
    unsigned filter_mask;
} wifi_promiscuous_filter_t;

typedef int wifi_promiscuous_pkt_type_t;
#define WIFI_PROMIS_FILTER_MASK_MGMT 1
#define WIFI_PROMIS_FILTER_MASK_DATA 2

typedef struct {
    int lltf_en;
    int htltf_en;
    int stbc_htltf2_en;
    int ltf_merge_en;
    int channel_filter_en;
    int manu_scale;
    int shift;
} wifi_csi_config_t;

typedef struct {
    uint8_t primary;
} wifi_ap_record_t;

typedef enum {
    WIFI_PS_NONE = 0,
    WIFI_PS_MIN_MODEM = 1,
    WIFI_PS_MAX_MODEM = 2,
} wifi_ps_type_t;

static inline esp_err_t esp_wifi_set_ps(wifi_ps_type_t type) { (void)type; return ESP_OK; }
static inline esp_err_t esp_wifi_set_promiscuous(bool en) { (void)en; return ESP_OK; }
static inline esp_err_t esp_wifi_set_promiscuous_rx_cb(void *cb) { (void)cb; return ESP_OK; }
static inline esp_err_t esp_wifi_set_promiscuous_filter(wifi_promiscuous_filter_t *f) { (void)f; return ESP_OK; }
static inline esp_err_t esp_wifi_set_csi_config(wifi_csi_config_t *c) { (void)c; return ESP_OK; }
static inline esp_err_t esp_wifi_set_csi_rx_cb(void *cb, void *ctx) { (void)cb; (void)ctx; return ESP_OK; }
static inline esp_err_t esp_wifi_set_csi(bool en) { (void)en; return ESP_OK; }
static inline esp_err_t esp_wifi_set_channel(uint8_t ch, wifi_second_chan_t sc) { (void)ch; (void)sc; return ESP_OK; }
static inline esp_err_t esp_wifi_80211_tx(wifi_interface_t ifx, const void *b, int len, bool en) { (void)ifx; (void)b; (void)len; (void)en; return ESP_OK; }
static inline esp_err_t esp_wifi_sta_get_ap_info(wifi_ap_record_t *ap) { (void)ap; return ESP_FAIL; }
static inline const char *esp_err_to_name(esp_err_t code) { (void)code; return "STUB"; }

/* ---- NVS stubs ---- */
typedef uint32_t nvs_handle_t;
#define NVS_READONLY 0
static inline esp_err_t nvs_open(const char *ns, int mode, nvs_handle_t *h) { (void)ns; (void)mode; (void)h; return ESP_FAIL; }
static inline void nvs_close(nvs_handle_t h) { (void)h; }
static inline esp_err_t nvs_get_str(nvs_handle_t h, const char *k, char *v, size_t *l) { (void)h; (void)k; (void)v; (void)l; return ESP_FAIL; }
static inline esp_err_t nvs_get_u8(nvs_handle_t h, const char *k, uint8_t *v) { (void)h; (void)k; (void)v; return ESP_FAIL; }
static inline esp_err_t nvs_get_u16(nvs_handle_t h, const char *k, uint16_t *v) { (void)h; (void)k; (void)v; return ESP_FAIL; }
static inline esp_err_t nvs_get_u32(nvs_handle_t h, const char *k, uint32_t *v) { (void)h; (void)k; (void)v; return ESP_FAIL; }
static inline esp_err_t nvs_get_blob(nvs_handle_t h, const char *k, void *v, size_t *l) { (void)h; (void)k; (void)v; (void)l; return ESP_FAIL; }

/* ---- stream_sender stubs (defined in esp_stubs.c) ---- */
int stream_sender_send(const uint8_t *data, size_t len);
int stream_sender_init(void);
int stream_sender_init_with(const char *ip, uint16_t port);
void stream_sender_deinit(void);

/*
 * wasm_runtime stubs: defined in esp_stubs.c.
 * The actual prototype comes from ../main/wasm_runtime.h (via csi_collector.c).
 * We just need the definition in esp_stubs.c to link.
 */

#endif /* ESP_STUBS_H */
