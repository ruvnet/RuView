/**
 * @file main.c
 * @brief ESP32-S3 CSI Node — ADR-018 compliant firmware.
 *
 * Initializes NVS, WiFi STA mode, CSI collection, and UDP streaming.
 * CSI frames are serialized in ADR-018 binary format and sent to the
 * aggregator over UDP.
 */

#include <string.h>
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "freertos/event_groups.h"
#include "esp_system.h"
#include "esp_wifi.h"
#include "esp_event.h"
#include "esp_log.h"
#include "nvs_flash.h"
#include "esp_app_desc.h"
#include "sdkconfig.h"
#include "led_strip.h"

#include "csi_collector.h"
#include "thermal.h"
#include "stream_sender.h"
#include "nvs_config.h"
#include "config_api.h"
#include "edge_processing.h"
#include "ota_update.h"
#include "power_mgmt.h"
#include "wasm_runtime.h"
#include "wasm_upload.h"
#include "display_task.h"
#include "mmwave_sensor.h"
#include "swarm_bridge.h"
#include "rv_radio_ops.h"          /* ADR-081 Layer 1 — Radio Abstraction Layer. */
#include "adaptive_controller.h"   /* ADR-081 Layer 2 — Adaptive controller. */
#include "c6_twt.h"                /* ADR-110: TWT (no-op stub on S3) */
#include "c6_timesync.h"           /* ADR-110: 802.15.4 mesh time-sync (no-op on S3) */
#include "c6_lp_core.h"            /* ADR-110: LP-core hibernation (no-op on S3) */
#include "c6_sync_espnow.h"        /* ADR-110 D1 workaround: ESP-NOW sync */
#include "c6_softap_he.h"          /* ADR-110 B1/B2: HE/TWT soft-AP (no-op when disabled) */
#ifdef CONFIG_CSI_MOCK_ENABLED
#include "mock_csi.h"
#endif

#include "esp_timer.h"

static const char *TAG = "main";

/* ADR-040: WASM timer handle (calls on_timer at configurable interval). */
static esp_timer_handle_t s_wasm_timer;

/* Runtime configuration (loaded from NVS or Kconfig defaults).
 * Global so other modules (wasm_upload.c) can access pubkey, etc. */
nvs_config_t g_nvs_config;

/* Event group bits */
#define WIFI_CONNECTED_BIT BIT0
#define WIFI_FAIL_BIT      BIT1

static EventGroupHandle_t s_wifi_event_group;
static int s_retry_num = 0;

/* WiFi reconnection.
 *
 * This used to be `#define MAX_RETRY 10` with the disconnect handler giving up
 * permanently once s_retry_num reached it: it set WIFI_FAIL_BIT and never
 * called esp_wifi_connect() again. s_retry_num is only cleared on a successful
 * IP_EVENT_STA_GOT_IP, so it accumulated over the node's entire uptime -- ten
 * disconnects spread across days left the node permanently off the network
 * with nothing but a power cycle to clear it.
 *
 * MEASURED 2026-08-30: a single AP-side event at 03:37:03 EDT disconnected all
 * three wall-mounted nodes within three minutes of each other. Every one of
 * them latched off and stayed off for 5.5-7.5 hours, until it was physically
 * unplugged. CSI capture kept running throughout (it is driver-level and needs
 * no IP), so the boards looked alive on the bench and dead to the server.
 *
 * Retry is now unbounded with exponential backoff. Two things that were
 * conflated are now separate: WIFI_BOOT_WAIT_ATTEMPTS only releases app_main
 * from its startup wait so the rest of the node can boot; it does not stop the
 * node trying to reconnect. Nothing stops the node trying to reconnect. */
#define WIFI_BOOT_WAIT_ATTEMPTS 10
#define WIFI_RETRY_BASE_MS       500
#define WIFI_RETRY_MAX_MS      30000

static esp_timer_handle_t s_reconnect_timer;

/* 500 ms doubling to a 30 s ceiling. Backoff matters because the failure this
 * guards against is an AP that is down or rebooting: hammering
 * esp_wifi_connect() at full rate for the minutes an AP takes to come back
 * wastes power and floods the log without reconnecting any sooner. */
static uint32_t wifi_retry_delay_ms(int attempt)
{
    int shift = attempt - 1;
    if (shift < 0)  shift = 0;
    if (shift > 6)  shift = 6;
    uint32_t d = (uint32_t)WIFI_RETRY_BASE_MS << shift;
    return d > WIFI_RETRY_MAX_MS ? WIFI_RETRY_MAX_MS : d;
}

/* Runs on the esp_timer task, not the event loop task -- esp_wifi_connect()
 * must not be reached through a delay inside the event handler itself. */
static void wifi_reconnect_cb(void *arg)
{
    (void)arg;
    esp_wifi_connect();
}

static void event_handler(void *arg, esp_event_base_t event_base,
                          int32_t event_id, void *event_data)
{
    if (event_base == WIFI_EVENT && event_id == WIFI_EVENT_STA_START) {
        esp_wifi_connect();
    } else if (event_base == WIFI_EVENT && event_id == WIFI_EVENT_STA_DISCONNECTED) {
        wifi_event_sta_disconnected_t *disc = (wifi_event_sta_disconnected_t *)event_data;
        ESP_LOGW(TAG, "WiFi disconnected, reason=%d rssi=%d", disc->reason, disc->rssi);
        s_retry_num++;
        /* Release the boot wait once, so a node that comes up while the AP is
         * down still starts CSI capture and the mesh instead of blocking in
         * app_main. Retrying continues regardless. */
        if (s_retry_num == WIFI_BOOT_WAIT_ATTEMPTS) {
            xEventGroupSetBits(s_wifi_event_group, WIFI_FAIL_BIT);
        }
        uint32_t delay_ms = wifi_retry_delay_ms(s_retry_num);
        if (s_reconnect_timer != NULL) {
            esp_timer_stop(s_reconnect_timer);   /* may not be running; harmless */
            esp_timer_start_once(s_reconnect_timer, (uint64_t)delay_ms * 1000);
        } else {
            esp_wifi_connect();                  /* timer unavailable: retry now */
        }
        ESP_LOGI(TAG, "Reconnecting in %lu ms (attempt %d)",
                 (unsigned long)delay_ms, s_retry_num);
    } else if (event_base == IP_EVENT && event_id == IP_EVENT_STA_GOT_IP) {
        ip_event_got_ip_t *event = (ip_event_got_ip_t *)event_data;
        ESP_LOGI(TAG, "Got IP: " IPSTR, IP2STR(&event->ip_info.ip));
        s_retry_num = 0;
        xEventGroupSetBits(s_wifi_event_group, WIFI_CONNECTED_BIT);
        /* Proof that a pending config trial produced a usable network. */
        config_trial_notify_connected();
        /* Same evidence starts the soak that confirms firmware on trial. */
        ota_rollback_notify_connected();
    }
}

#ifdef CONFIG_UPLINK_WATCHDOG
/* Uplink supervision.
 *
 * The reconnect fix above addresses the one wedge whose mechanism we have
 * proven. This catches the ones we have not: any state in which the node is
 * powered, capturing, and unable to put a packet on the wire. It bounds such a
 * state to CONFIG_UPLINK_WATCHDOG_TIMEOUT_S instead of the 7.5 hours measured
 * on 2026-08-30.
 *
 * The signal is a successful sendto(), which on UDP means the stack accepted
 * the datagram -- NOT that the server received it. That distinction is what
 * makes this safe: stopping the aggregator does not reboot the fleet, because
 * sendto keeps succeeding into a socket whose peer is gone. It fires only when
 * the node's own network path is broken.
 *
 * It arms on the first successful send rather than at boot, so a node that has
 * never reached the network does not reboot-loop while the AP is down; the
 * unbounded reconnect above owns that case. */
static void uplink_watchdog_tick(void)
{
    int64_t last = stream_sender_last_success_us();
    if (last == 0) {
        return;                      /* never delivered -- not armed yet */
    }
    int64_t idle_s = (esp_timer_get_time() - last) / 1000000;
    if (idle_s < CONFIG_UPLINK_WATCHDOG_TIMEOUT_S) {
        return;
    }
    ESP_LOGE(TAG, "uplink watchdog: no successful send for %lld s "
                  "(limit %d s) -- restarting",
             (long long)idle_s, CONFIG_UPLINK_WATCHDOG_TIMEOUT_S);
    esp_restart();
}
#endif /* CONFIG_UPLINK_WATCHDOG */

static void wifi_init_sta(void)
{
    s_wifi_event_group = xEventGroupCreate();

    ESP_ERROR_CHECK(esp_netif_init());
    ESP_ERROR_CHECK(esp_event_loop_create_default());
    esp_netif_create_default_wifi_sta();

    wifi_init_config_t cfg = WIFI_INIT_CONFIG_DEFAULT();
    ESP_ERROR_CHECK(esp_wifi_init(&cfg));

    esp_event_handler_instance_t instance_any_id;
    esp_event_handler_instance_t instance_got_ip;
    ESP_ERROR_CHECK(esp_event_handler_instance_register(
        WIFI_EVENT, ESP_EVENT_ANY_ID, &event_handler, NULL, &instance_any_id));
    ESP_ERROR_CHECK(esp_event_handler_instance_register(
        IP_EVENT, IP_EVENT_STA_GOT_IP, &event_handler, NULL, &instance_got_ip));

    wifi_config_t wifi_config = {
        .sta = {
            /* WPA_PSK (not WPA2_PSK) so routers running WPA/WPA2-mixed
             * compatibility mode aren't rejected with
             * WIFI_REASON_NO_AP_FOUND_IN_AUTHMODE_THRESHOLD (#1050). */
            .threshold.authmode = WIFI_AUTH_WPA_PSK,
        },
    };

    /* Copy runtime SSID/password from NVS config */
    strlcpy((char *)wifi_config.sta.ssid, g_nvs_config.wifi_ssid,
            sizeof(wifi_config.sta.ssid));
    strlcpy((char *)wifi_config.sta.password, g_nvs_config.wifi_password,
            sizeof(wifi_config.sta.password));

    /* If password is empty, use open auth */
    if (strlen((char *)wifi_config.sta.password) == 0) {
        wifi_config.sta.threshold.authmode = WIFI_AUTH_OPEN;
    }

    ESP_ERROR_CHECK(esp_wifi_set_mode(WIFI_MODE_STA));
    ESP_ERROR_CHECK(esp_wifi_set_config(WIFI_IF_STA, &wifi_config));

#if defined(CONFIG_IDF_TARGET_ESP32C6) && defined(CONFIG_C6_SOFTAP_HE_ENABLE)
    /* ADR-110 B1/B2 cheap-unblock: bring up a soft-AP that advertises HE +
     * TWT Responder=1 so a second C6 board can negotiate iTWT against
     * this node. c6_softap_he_start() switches the mode to AP+STA. */
    uint8_t softap_chan = 0;
    if (c6_softap_he_start(&softap_chan) == ESP_OK) {
        ESP_LOGI(TAG, "C6 soft-AP HE armed on channel %u (ADR-110 B1/B2)", softap_chan);
    }
#endif

    const esp_timer_create_args_t reconnect_args = {
        .callback = &wifi_reconnect_cb,
        .name     = "wifi_reconnect",
    };
    esp_err_t rc_ret = esp_timer_create(&reconnect_args, &s_reconnect_timer);
    if (rc_ret != ESP_OK) {
        ESP_LOGW(TAG, "reconnect timer create failed: %s (falling back to "
                      "immediate retry)", esp_err_to_name(rc_ret));
        s_reconnect_timer = NULL;
    }

    ESP_ERROR_CHECK(esp_wifi_start());

    ESP_LOGI(TAG, "WiFi STA initialized, connecting to SSID: %s", g_nvs_config.wifi_ssid);

    /* Wait for connection */
    EventBits_t bits = xEventGroupWaitBits(s_wifi_event_group,
        WIFI_CONNECTED_BIT | WIFI_FAIL_BIT,
        pdFALSE, pdFALSE, portMAX_DELAY);

    if (bits & WIFI_CONNECTED_BIT) {
        ESP_LOGI(TAG, "Connected to WiFi");
    } else if (bits & WIFI_FAIL_BIT) {
        ESP_LOGW(TAG, "Not connected after %d attempts -- continuing boot; "
                      "reconnection keeps retrying in the background",
                 WIFI_BOOT_WAIT_ATTEMPTS);
    }
}

#if CONFIG_LED_GAMMA_VIZ
/* Viridis colormap (60 steps), generated from ruv-neural-viz::ColorMap::viridis()
 * — the rUv-Neural brain-topology colormap, now no_std (ruvnet/ruv-neural#3 /
 * RuView#1126). Used as the ON-phase colour of the 40 Hz gamma flicker below:
 * dark-purple (still) -> teal -> green -> yellow (strong motion). */
static const uint8_t VIRIDIS_LUT[60][3] = {
    { 68,  1, 84},{ 67,  6, 88},{ 67, 12, 91},{ 66, 17, 95},{ 66, 23, 99},
    { 65, 28,103},{ 64, 34,106},{ 64, 39,110},{ 63, 45,114},{ 63, 50,118},
    { 62, 56,121},{ 61, 61,125},{ 61, 67,129},{ 60, 72,132},{ 59, 78,136},
    { 59, 83,139},{ 57, 87,139},{ 55, 92,139},{ 53, 96,139},{ 52,100,139},
    { 50,104,139},{ 48,109,139},{ 46,113,139},{ 44,117,140},{ 43,122,140},
    { 41,126,140},{ 39,130,140},{ 37,134,140},{ 36,139,140},{ 34,143,140},
    { 35,147,139},{ 39,151,136},{ 43,154,133},{ 47,158,130},{ 52,162,127},
    { 56,166,124},{ 60,170,121},{ 64,173,119},{ 68,177,116},{ 72,181,113},
    { 76,185,110},{ 81,189,107},{ 85,192,104},{ 89,196,102},{ 93,200, 99},
    {102,203, 95},{113,205, 91},{124,207, 87},{134,209, 82},{145,211, 78},
    {156,213, 74},{167,215, 70},{178,217, 66},{188,219, 62},{199,221, 58},
    {210,223, 54},{221,225, 49},{231,227, 45},{242,229, 41},{253,231, 37},
};
static led_strip_handle_t s_viz_led;

/* motion_energy that saturates the colormap to yellow (CONFIG, milli-units). */
#define LED_MOTION_FULLSCALE ((float)CONFIG_LED_MOTION_FULLSCALE_MILLI / 1000.0f)

/* GENUS-style 40 Hz gamma flicker: full on/off square wave, 50% duty (toggled
 * every 12.5 ms → 40 Hz). The ON colour is live CSI motion (edge motion_energy)
 * mapped through the ruv-neural-viz viridis LUT — still=purple, moving=yellow.
 * So the LED is a real 40 Hz gamma stimulus whose hue tracks sensed motion. */
static void led_gamma_40hz_cb(void *arg)
{
    static bool on = false;
    static uint8_t last_mode = 0xFF;

    /* Read live rather than latching at boot, so turning the LED down from
     * config_push.py takes effect immediately -- the point of the knob is
     * being able to darken a bedroom without waiting for a reboot. */
    uint8_t mode = g_nvs_config.led_mode;
    uint8_t bright = g_nvs_config.led_brightness;
    if (bright > 100) bright = 100;

    if (mode == LED_MODE_OFF) {
        /* Blank once on the transition, then leave the RMT idle instead of
         * clocking out zeros eighty times a second for the node's lifetime. */
        if (last_mode != LED_MODE_OFF) {
            led_strip_set_pixel(s_viz_led, 0, 0, 0, 0);
            led_strip_refresh(s_viz_led);
            last_mode = mode;
        }
        return;
    }
    last_mode = mode;

    on = !on;
    /* STEADY keeps the motion colour but drops the square wave: same
     * information, no 40 Hz strobe. */
    if (on || mode == LED_MODE_STEADY) {
        edge_vitals_pkt_t v;
        float m = edge_get_vitals(&v) ? v.motion_energy : 0.0f;
        float norm = m / LED_MOTION_FULLSCALE;
        if (norm < 0.0f) norm = 0.0f;
        if (norm > 1.0f) norm = 1.0f;
        int idx = (int)(norm * 59.0f + 0.5f);
        const uint8_t *c = VIRIDIS_LUT[idx];
        led_strip_set_pixel(s_viz_led, 0,                    /* R,G,B (driver maps to GRB) */
                            (uint32_t)c[0] * bright / 100,
                            (uint32_t)c[1] * bright / 100,
                            (uint32_t)c[2] * bright / 100);
    } else {
        led_strip_set_pixel(s_viz_led, 0, 0, 0, 0);          /* off phase */
    }
    led_strip_refresh(s_viz_led);
}
#endif /* CONFIG_LED_GAMMA_VIZ */

void app_main(void)
{
    /* Initialize NVS */
    esp_err_t ret = nvs_flash_init();
    if (ret == ESP_ERR_NVS_NO_FREE_PAGES || ret == ESP_ERR_NVS_NEW_VERSION_FOUND) {
        ESP_ERROR_CHECK(nvs_flash_erase());
        ret = nvs_flash_init();
    }
    ESP_ERROR_CHECK(ret);

    /* Load runtime config (NVS overrides Kconfig defaults) */
    /* Before loading config: if the last change is still on trial this arms
     * the revert timer, so settings that cannot associate undo themselves. */
    config_trial_boot_check();

    /* And report whether the bootloader just reverted a failed firmware image. */
    ota_rollback_boot_check();

    nvs_config_load(&g_nvs_config);

    /* Capture node_id IMMEDIATELY — before wifi_init_sta() can corrupt
     * g_nvs_config. See #232/#375/#390: WiFi driver init clobbers the struct
     * on some devices, reverting node_id to the Kconfig default of 1. */
    csi_collector_set_node_id(g_nvs_config.node_id);

    const esp_app_desc_t *app_desc = esp_app_get_description();
#if defined(CONFIG_IDF_TARGET_ESP32C6)
    const char *target_name = "ESP32-C6";
#elif defined(CONFIG_IDF_TARGET_ESP32S3)
    const char *target_name = "ESP32-S3";
#else
    const char *target_name = "ESP32";
#endif
    ESP_LOGI(TAG, "%s CSI Node (ADR-018 / ADR-110) — v%s — Node ID: %d",
             target_name, app_desc->version, g_nvs_config.node_id);

    /* Onboard WS2812. C6 wires the LED to GPIO 8; S3 to GPIO 38 (DevKitC-1 v1.0)
     * or GPIO 48 (DevKitC-1 v1.1 / N16R8 — see #962). On S3 we drive 48 (the
     * common module). On C6, GPIO 38/48 don't exist (only 0-30) — gate by target.
     * Behaviour is set by CONFIG_LED_GAMMA_VIZ (ADR-183): on = 40 Hz gamma flicker
     * coloured by CSI motion; off = clear the LED at boot. */
#if defined(CONFIG_IDF_TARGET_ESP32C6)
    const int led_gpio = 8;
#else
    const int led_gpio = 48;
#endif
    led_strip_config_t strip_config = {
        .strip_gpio_num = led_gpio,
        .max_leds = 1,
        .led_model = LED_MODEL_WS2812,
        .color_component_format = LED_STRIP_COLOR_COMPONENT_FMT_GRB,
        .flags.invert_out = false,
    };
    led_strip_rmt_config_t rmt_config = {
        .resolution_hz = 10 * 1000 * 1000, // 10MHz
        .flags.with_dma = false,
    };
#if CONFIG_LED_GAMMA_VIZ
    if (led_strip_new_rmt_device(&strip_config, &rmt_config, &s_viz_led) == ESP_OK) {
        const esp_timer_create_args_t viz_args = {
            .callback = &led_gamma_40hz_cb,
            .name = "led_gamma_40hz",
        };
        esp_timer_handle_t viz_timer;
        if (esp_timer_create(&viz_args, &viz_timer) == ESP_OK) {
            esp_timer_start_periodic(viz_timer, 12500); // 12.5 ms toggle → 40 Hz square wave
            ESP_LOGI(TAG, "Onboard WS2812: 40 Hz gamma flicker (GENUS), colour=CSI motion via ruv-neural-viz, GPIO %d", led_gpio);
        }
    }
#else
    /* Viz disabled — clear the onboard LED at boot and release the RMT channel. */
    led_strip_handle_t led_strip;
    if (led_strip_new_rmt_device(&strip_config, &rmt_config, &led_strip) == ESP_OK) {
        led_strip_clear(led_strip);
        led_strip_del(led_strip);
    }
#endif /* CONFIG_LED_GAMMA_VIZ */

    /* ADR-110 P4: 802.15.4 mesh time-sync (C6 only).
     * Initialized BEFORE WiFi so it's available even when WiFi STA can't
     * connect — the radios are physically independent on the C6.
     * No-op on S3 (the helper compiles to an empty inline stub). */
#if defined(CONFIG_IDF_TARGET_ESP32C6) && defined(CONFIG_C6_TIMESYNC_ENABLE)
    esp_err_t ts_ret = c6_timesync_init(CONFIG_C6_TIMESYNC_CHANNEL);
    if (ts_ret != ESP_OK) {
        ESP_LOGW(TAG, "c6_timesync_init failed: %s (continuing without 15.4 sync)",
                 esp_err_to_name(ts_ret));
    }
#endif

    /* ADR-110 P5: Optionally arm LP-core wake-on-motion (C6 only, opt-in).
     * Default off — only nodes flashed for battery-powered seed duty enable
     * this in menuconfig. */
#if defined(CONFIG_IDF_TARGET_ESP32C6) && defined(CONFIG_C6_LP_CORE_ENABLE)
    if (c6_lp_core_was_motion_wake()) {
        ESP_LOGI(TAG, "boot cause: LP-core motion wake (running CSI burst)");
    }
#endif

    /* Initialize WiFi STA (skip entirely under QEMU mock — no RF hardware) */
#ifndef CONFIG_CSI_MOCK_SKIP_WIFI_CONNECT
    wifi_init_sta();
#else
    ESP_LOGI(TAG, "Mock CSI mode: skipping WiFi init (CONFIG_CSI_MOCK_SKIP_WIFI_CONNECT)");
#endif

    /* Initialize UDP sender with runtime target */
#ifdef CONFIG_CSI_MOCK_SKIP_WIFI_CONNECT
    ESP_LOGI(TAG, "Mock CSI mode: skipping UDP sender init (no network)");
#else
    if (stream_sender_init_with(g_nvs_config.target_ip, g_nvs_config.target_port) != 0) {
        ESP_LOGE(TAG, "Failed to initialize UDP sender");
        return;
    }
#endif

    /* Initialize CSI collection */
#ifdef CONFIG_CSI_MOCK_ENABLED
    /* ADR-061: Start mock CSI generator (replaces real WiFi CSI in QEMU) */
    esp_err_t mock_ret = mock_csi_init(CONFIG_CSI_MOCK_SCENARIO);
    if (mock_ret != ESP_OK) {
        ESP_LOGE(TAG, "Mock CSI init failed: %s", esp_err_to_name(mock_ret));
    } else {
        ESP_LOGI(TAG, "Mock CSI active (scenario=%d)", CONFIG_CSI_MOCK_SCENARIO);
    }
#else
    /* Why did we just start? Nothing recorded this before, and it is the
     * single most useful line in a fleet log: a node that reboots tells you
     * almost nothing, but a node that reboots with ESP_RST_BROWNOUT tells you
     * its supply sagged, which on shared outlets is a wiring problem rather
     * than a firmware one. Three nodes dropping together on 2026-08-28 would
     * have been answered by this line. */
    {
        esp_reset_reason_t rr = esp_reset_reason();
        const char *why =
            rr == ESP_RST_POWERON  ? "power-on" :
            rr == ESP_RST_EXT      ? "external reset" :
            rr == ESP_RST_SW       ? "software restart" :
            rr == ESP_RST_PANIC    ? "PANIC (crash)" :
            rr == ESP_RST_INT_WDT  ? "interrupt watchdog" :
            rr == ESP_RST_TASK_WDT ? "task watchdog" :
            rr == ESP_RST_WDT      ? "other watchdog" :
            rr == ESP_RST_BROWNOUT ? "BROWNOUT (supply sagged)" :
            rr == ESP_RST_DEEPSLEEP? "deep-sleep wake" : "unknown";
        if (rr == ESP_RST_PANIC || rr == ESP_RST_BROWNOUT ||
            rr == ESP_RST_INT_WDT || rr == ESP_RST_TASK_WDT) {
            ESP_LOGW(TAG, "reset reason: %s (%d) <-- not a clean start", why, (int)rr);
        } else {
            ESP_LOGI(TAG, "reset reason: %s (%d)", why, (int)rr);
        }
    }

    /* Bring thermal monitoring up before the radio is loaded, so the first
     * reading is taken against a known-idle baseline rather than mid-burst. */
    thermal_init();

    csi_collector_init();

    /* ADR-073: Start multi-frequency channel hopping if configured in NVS. */
    if (g_nvs_config.channel_hop_count > 1) {
        ESP_LOGI(TAG, "Starting channel hopping: %u channels, dwell=%lu ms",
                 (unsigned)g_nvs_config.channel_hop_count,
                 (unsigned long)g_nvs_config.dwell_ms);
        csi_collector_set_hop_table(
            g_nvs_config.channel_list,
            g_nvs_config.channel_hop_count,
            g_nvs_config.dwell_ms);
    }
#endif

    /* ADR-110 P3: Request TWT from the AP for deterministic CSI cadence.
     * No-op on S3 (the helper compiles to an empty inline stub). On C6
     * the AP may NACK — the helper logs and falls back to opportunistic.
     * Called only after WiFi STA connect (wifi_init_sta blocks until then). */
#if defined(CONFIG_IDF_TARGET_ESP32C6) && defined(CONFIG_C6_TWT_ENABLE)
    c6_twt_setup_default();
#endif

    /* ADR-110 D1 workaround: ESP-NOW cross-node sync. Initialized after
     * WiFi STA connects (ESP-NOW needs the WiFi driver up). Works on
     * both S3 and C6 — replaces the broken 802.15.4 RX path in c6_timesync.
     * Skip on QEMU mock (no real WiFi → no ESP-NOW). */
#ifndef CONFIG_CSI_MOCK_SKIP_WIFI_CONNECT
    esp_err_t espnow_ret = c6_sync_espnow_init();
    if (espnow_ret != ESP_OK) {
        ESP_LOGW(TAG, "c6_sync_espnow_init failed: %s (continuing without ESP-NOW sync)",
                 esp_err_to_name(espnow_ret));
    }
#endif

    /* ADR-039: Initialize edge processing pipeline. */
    edge_config_t edge_cfg = {
        .tier              = g_nvs_config.edge_tier,
        .presence_thresh   = g_nvs_config.presence_thresh,
        .fall_thresh       = g_nvs_config.fall_thresh,
        .vital_window      = g_nvs_config.vital_window,
        .vital_interval_ms = g_nvs_config.vital_interval_ms,
        .top_k_count       = g_nvs_config.top_k_count,
        .power_duty        = g_nvs_config.power_duty,
    };
    esp_err_t edge_ret = edge_processing_init(&edge_cfg);
    if (edge_ret != ESP_OK) {
        ESP_LOGW(TAG, "Edge processing init failed: %s (continuing without edge DSP)",
                 esp_err_to_name(edge_ret));
    }

    /* Initialize OTA update HTTP server (requires network). */
    httpd_handle_t ota_server = NULL;
#ifndef CONFIG_CSI_MOCK_SKIP_WIFI_CONNECT
    esp_err_t ota_ret = ota_update_init_ex(&ota_server);
    if (ota_ret != ESP_OK) {
        ESP_LOGW(TAG, "OTA server init failed: %s", esp_err_to_name(ota_ret));
    }
#else
    esp_err_t ota_ret = ESP_ERR_NOT_SUPPORTED;
    ESP_LOGI(TAG, "Mock CSI mode: skipping OTA server (no network)");
#endif

    /* ADR-040: Initialize WASM programmable sensing runtime. */
    esp_err_t wasm_ret = wasm_runtime_init();
    if (wasm_ret != ESP_OK) {
        ESP_LOGW(TAG, "WASM runtime init failed: %s", esp_err_to_name(wasm_ret));
    } else {
        /* Register WASM upload endpoints on the OTA HTTP server. */
        if (ota_server != NULL) {
            wasm_upload_register(ota_server);
        }

        /* Start periodic timer for wasm_runtime_on_timer(). */
        esp_timer_create_args_t timer_args = {
            .callback = (void (*)(void *))wasm_runtime_on_timer,
            .arg = NULL,
            .dispatch_method = ESP_TIMER_TASK,
            .name = "wasm_timer",
        };
        esp_err_t timer_ret = esp_timer_create(&timer_args, &s_wasm_timer);
        if (timer_ret == ESP_OK) {
#ifdef CONFIG_WASM_TIMER_INTERVAL_MS
            uint64_t interval_us = (uint64_t)CONFIG_WASM_TIMER_INTERVAL_MS * 1000ULL;
#else
            uint64_t interval_us = 1000000ULL;  /* Default: 1 second. */
#endif
            esp_timer_start_periodic(s_wasm_timer, interval_us);
            ESP_LOGI(TAG, "WASM on_timer() periodic: %llu ms",
                     (unsigned long long)(interval_us / 1000));
        } else {
            ESP_LOGW(TAG, "WASM timer create failed: %s", esp_err_to_name(timer_ret));
        }
    }

    /* ADR-063: Initialize mmWave sensor (auto-detect on UART). */
    esp_err_t mmwave_ret = mmwave_sensor_init(-1, -1);  /* -1 = use default GPIO pins */
    if (mmwave_ret == ESP_OK) {
        mmwave_state_t mw;
        if (mmwave_sensor_get_state(&mw)) {
            ESP_LOGI(TAG, "mmWave sensor: %s (caps=0x%04x)",
                     mmwave_type_name(mw.type), mw.capabilities);
        }
    } else {
        ESP_LOGI(TAG, "No mmWave sensor detected (CSI-only mode)");
    }

    /* ADR-066: Initialize swarm bridge to Cognitum Seed (if configured). */
    esp_err_t swarm_ret = ESP_ERR_INVALID_ARG;
#ifndef CONFIG_CSI_MOCK_SKIP_WIFI_CONNECT
    if (g_nvs_config.seed_url[0] != '\0') {
        swarm_config_t swarm_cfg = {
            .heartbeat_sec = g_nvs_config.swarm_heartbeat_sec,
            .ingest_sec    = g_nvs_config.swarm_ingest_sec,
            .enabled       = 1,
        };
        strlcpy(swarm_cfg.seed_url, g_nvs_config.seed_url,
                sizeof(swarm_cfg.seed_url));
        strlcpy(swarm_cfg.seed_token, g_nvs_config.seed_token,
                sizeof(swarm_cfg.seed_token));
        strlcpy(swarm_cfg.zone_name, g_nvs_config.zone_name,
                sizeof(swarm_cfg.zone_name));
        swarm_ret = swarm_bridge_init(&swarm_cfg, csi_collector_get_node_id());
        if (swarm_ret != ESP_OK) {
            ESP_LOGW(TAG, "Swarm bridge init failed: %s", esp_err_to_name(swarm_ret));
        }
    } else {
        ESP_LOGI(TAG, "Swarm bridge disabled (no seed_url configured)");
    }
#else
    ESP_LOGI(TAG, "Mock CSI mode: skipping swarm bridge");
#endif

    /* ADR-081 Layer 1: register the active radio ops binding.
     * - Real hardware: ESP32 binding wrapping csi_collector + esp_wifi.
     * - QEMU / offline: mock binding wrapping mock_csi.c.
     * Either way, the layers above (adaptive controller, mesh plane,
     * feature extraction) address the radio through the same vtable —
     * this is the portability acceptance test in ADR-081. */
#ifdef CONFIG_CSI_MOCK_ENABLED
    rv_radio_ops_mock_register();
#else
    rv_radio_ops_esp32_register();
#endif
    const rv_radio_ops_t *radio_ops = rv_radio_ops_get();
    if (radio_ops != NULL && radio_ops->init != NULL) {
        radio_ops->init();
    }

    /* ADR-081 Layer 2: start the adaptive controller. NULL config → use
     * Kconfig defaults. Default policy is conservative: no channel
     * switching, no role change. Operators opt in via menuconfig. */
    esp_err_t adapt_ret = adaptive_controller_init(NULL);
    if (adapt_ret != ESP_OK) {
        ESP_LOGW(TAG, "Adaptive controller init failed: %s",
                 esp_err_to_name(adapt_ret));
    }

    /* Initialize power management. */
    power_mgmt_init(g_nvs_config.power_duty);

    /* ADR-045: Start AMOLED display task (gracefully skips if no display). */
#ifdef CONFIG_DISPLAY_ENABLE
    esp_err_t disp_ret = display_task_start();
    if (disp_ret != ESP_OK) {
        ESP_LOGW(TAG, "Display init returned: %s", esp_err_to_name(disp_ret));
    }
#endif

    /* RuView#893/#521: the MGMT-only promiscuous filter (set in
     * csi_collector_init as the #396 display-crash workaround) starves the CSI
     * callback on display-less boards — yield collapses to 0 pps and the node
     * looks dead despite being on the network. Now that the display probe has
     * run, boards with no AMOLED panel (no QSPI/SPI-flash cache contention)
     * upgrade the filter to capture DATA frames too, restoring CSI yield. */
#ifdef CONFIG_DISPLAY_ENABLE
    bool has_display = display_is_active();   /* runtime panel probe result */
#else
    bool has_display = false;                 /* display support not compiled in */
#endif
    if (!has_display) {
        csi_collector_enable_data_capture();
    }

    ESP_LOGI(TAG, "CSI streaming active → %s:%d (edge_tier=%u, OTA=%s, WASM=%s, mmWave=%s, swarm=%s, adapt=%s)",
             g_nvs_config.target_ip, g_nvs_config.target_port,
             g_nvs_config.edge_tier,
             (ota_ret == ESP_OK) ? "ready" : "off",
             (wasm_ret == ESP_OK) ? "ready" : "off",
             (mmwave_ret == ESP_OK) ? "active" : "off",
             (swarm_ret == ESP_OK) ? g_nvs_config.seed_url : "off",
             (adapt_ret == ESP_OK) ? "on" : "off");

    /* Main loop — keep alive, and supervise the uplink. */
    while (1) {
        vTaskDelay(pdMS_TO_TICKS(10000));
#ifdef CONFIG_UPLINK_WATCHDOG
        uplink_watchdog_tick();
#endif
    }
}
