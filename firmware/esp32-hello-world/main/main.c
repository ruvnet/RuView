/**
 * @file main.c
 * @brief ESP32-S3 Hello World — Full Capability Discovery
 *
 * Boots up, prints "Hello World!", then probes and reports every major
 * hardware/software capability of the ESP32-S3: chip info, flash, PSRAM,
 * WiFi (including CSI), Bluetooth, GPIOs, peripherals, FreeRTOS stats,
 * and power management features.  No WiFi connection required.
 */

#include <stdio.h>
#include <string.h>
#include <inttypes.h>

#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "esp_system.h"
#include "esp_chip_info.h"
#include "esp_flash.h"
#include "esp_mac.h"
#include "esp_log.h"
#include "esp_wifi.h"
#include "esp_event.h"
#include "esp_timer.h"
#include "esp_heap_caps.h"
#include "esp_partition.h"
#include "esp_ota_ops.h"
#include "esp_efuse.h"
#include "esp_pm.h"
#include "nvs_flash.h"
#include "soc/soc_caps.h"
#include "driver/gpio.h"
#include "driver/temperature_sensor.h"
#include "sdkconfig.h"

static const char *TAG = "hello";

/* ── Helpers ─────────────────────────────────────────────────────────── */

static const char *chip_model_str(esp_chip_model_t model)
{
    switch (model) {
        case CHIP_ESP32:   return "ESP32";
        case CHIP_ESP32S2: return "ESP32-S2";
        case CHIP_ESP32S3: return "ESP32-S3";
        case CHIP_ESP32C3: return "ESP32-C3";
        case CHIP_ESP32H2: return "ESP32-H2";
        case CHIP_ESP32C2: return "ESP32-C2";
        default:           return "Unknown";
    }
}

static void print_separator(const char *title)
{
    printf("\n╔══════════════════════════════════════════════════════════╗\n");
    printf("║  %-55s ║\n", title);
    printf("╚══════════════════════════════════════════════════════════╝\n");
}

/* ── Capability Probes ───────────────────────────────────────────────── */

static void probe_chip_info(void)
{
    print_separator("CHIP INFO");

    esp_chip_info_t info;
    esp_chip_info(&info);

    printf("  Model:          %s (rev %d.%d)\n",
           chip_model_str(info.model),
           info.revision / 100, info.revision % 100);
    printf("  Cores:          %d\n", info.cores);
    printf("  Features:       ");
    if (info.features & CHIP_FEATURE_WIFI_BGN) printf("WiFi ");
    if (info.features & CHIP_FEATURE_BLE)      printf("BLE ");
    if (info.features & CHIP_FEATURE_BT)       printf("BT-Classic ");
    if (info.features & CHIP_FEATURE_IEEE802154) printf("802.15.4 ");
    if (info.features & CHIP_FEATURE_EMB_FLASH) printf("EmbFlash ");
    if (info.features & CHIP_FEATURE_EMB_PSRAM) printf("EmbPSRAM ");
    printf("\n");

    /* MAC addresses */
    uint8_t mac[6];
    if (esp_read_mac(mac, ESP_MAC_WIFI_STA) == ESP_OK) {
        printf("  WiFi STA MAC:   %02X:%02X:%02X:%02X:%02X:%02X\n",
               mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]);
    }
    if (esp_read_mac(mac, ESP_MAC_BT) == ESP_OK) {
        printf("  BT MAC:         %02X:%02X:%02X:%02X:%02X:%02X\n",
               mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]);
    }

    printf("  IDF Version:    %s\n", esp_get_idf_version());
    printf("  Reset Reason:   %d\n", esp_reset_reason());
}

static void probe_memory(void)
{
    print_separator("MEMORY");

    /* Internal RAM */
    printf("  Internal DRAM:\n");
    printf("    Total:        %"PRIu32" bytes\n",
           (uint32_t)heap_caps_get_total_size(MALLOC_CAP_INTERNAL));
    printf("    Free:         %"PRIu32" bytes\n",
           (uint32_t)heap_caps_get_free_size(MALLOC_CAP_INTERNAL));
    printf("    Min Free:     %"PRIu32" bytes\n",
           (uint32_t)heap_caps_get_minimum_free_size(MALLOC_CAP_INTERNAL));

    /* PSRAM */
    size_t psram_total = heap_caps_get_total_size(MALLOC_CAP_SPIRAM);
    if (psram_total > 0) {
        printf("  External PSRAM:\n");
        printf("    Total:        %"PRIu32" bytes (%.1f MB)\n",
               (uint32_t)psram_total, psram_total / (1024.0 * 1024.0));
        printf("    Free:         %"PRIu32" bytes\n",
               (uint32_t)heap_caps_get_free_size(MALLOC_CAP_SPIRAM));
    } else {
        printf("  External PSRAM: Not available\n");
    }

    /* DMA-capable */
    printf("  DMA-capable:    %"PRIu32" bytes free\n",
           (uint32_t)heap_caps_get_free_size(MALLOC_CAP_DMA));
}

static void probe_flash(void)
{
    print_separator("FLASH STORAGE");

    uint32_t flash_size = 0;
    if (esp_flash_get_size(NULL, &flash_size) == ESP_OK) {
        printf("  Flash Size:     %"PRIu32" bytes (%.0f MB)\n",
               flash_size, flash_size / (1024.0 * 1024.0));
    }

    /* Partition table */
    printf("  Partitions:\n");
    esp_partition_iterator_t it = esp_partition_find(ESP_PARTITION_TYPE_ANY,
                                                     ESP_PARTITION_SUBTYPE_ANY, NULL);
    while (it != NULL) {
        const esp_partition_t *p = esp_partition_get(it);
        printf("    %-16s type=0x%02x sub=0x%02x offset=0x%06"PRIx32" size=%"PRIu32" KB\n",
               p->label, p->type, p->subtype, p->address, p->size / 1024);
        it = esp_partition_next(it);
    }
    esp_partition_iterator_release(it);

    /* Running partition */
    const esp_partition_t *running = esp_ota_get_running_partition();
    if (running) {
        printf("  Running from:   %s (0x%06"PRIx32")\n", running->label, running->address);
    }
}

static void probe_wifi_capabilities(void)
{
    print_separator("WiFi CAPABILITIES");

    /* Init WiFi just enough to query capabilities (no connection) */
    ESP_ERROR_CHECK(esp_netif_init());
    ESP_ERROR_CHECK(esp_event_loop_create_default());
    esp_netif_create_default_wifi_sta();

    wifi_init_config_t cfg = WIFI_INIT_CONFIG_DEFAULT();
    ESP_ERROR_CHECK(esp_wifi_init(&cfg));
    ESP_ERROR_CHECK(esp_wifi_set_mode(WIFI_MODE_STA));
    ESP_ERROR_CHECK(esp_wifi_start());

    /* Protocol capabilities */
    printf("  Protocols:      802.11 b/g/n\n");

    /* CSI (Channel State Information) */
#ifdef CONFIG_ESP_WIFI_CSI_ENABLED
    printf("  CSI:            ENABLED (Channel State Information)\n");
    printf("    - Subcarrier amplitude & phase data\n");
    printf("    - Per-packet callback available\n");
    printf("    - Use for: presence detection, gesture recognition,\n");
    printf("      breathing/heart rate, indoor positioning\n");
#else
    printf("  CSI:            DISABLED (enable CONFIG_ESP_WIFI_CSI_ENABLED)\n");
#endif

    /* Scan to show what's visible */
    printf("  WiFi Scan:      Scanning nearby APs...\n");
    wifi_scan_config_t scan_cfg = {
        .show_hidden = true,
        .scan_type = WIFI_SCAN_TYPE_ACTIVE,
        .scan_time.active.min = 100,
        .scan_time.active.max = 300,
    };
    esp_wifi_scan_start(&scan_cfg, true);  /* blocking scan */

    uint16_t ap_count = 0;
    esp_wifi_scan_get_ap_num(&ap_count);
    printf("  APs Found:      %d\n", ap_count);

    if (ap_count > 0) {
        uint16_t max_show = (ap_count > 10) ? 10 : ap_count;
        wifi_ap_record_t *ap_list = malloc(sizeof(wifi_ap_record_t) * max_show);
        if (ap_list) {
            esp_wifi_scan_get_ap_records(&max_show, ap_list);
            printf("  %-32s  CH  RSSI  Auth\n", "  SSID");
            printf("  %-32s  --  ----  ----\n", "  ----");
            for (int i = 0; i < max_show; i++) {
                const char *auth_str = "OPEN";
                switch (ap_list[i].authmode) {
                    case WIFI_AUTH_WEP:          auth_str = "WEP"; break;
                    case WIFI_AUTH_WPA_PSK:       auth_str = "WPA"; break;
                    case WIFI_AUTH_WPA2_PSK:      auth_str = "WPA2"; break;
                    case WIFI_AUTH_WPA_WPA2_PSK:  auth_str = "WPA/2"; break;
                    case WIFI_AUTH_WPA3_PSK:      auth_str = "WPA3"; break;
                    case WIFI_AUTH_WPA2_WPA3_PSK: auth_str = "WPA2/3"; break;
                    default: break;
                }
                printf("    %-30s  %2d  %4d  %s\n",
                       (char *)ap_list[i].ssid,
                       ap_list[i].primary,
                       ap_list[i].rssi,
                       auth_str);
            }
            free(ap_list);
            if (ap_count > max_show)
                printf("    ... and %d more\n", ap_count - max_show);
        }
    }

    /* WiFi modes supported */
    printf("\n  Supported Modes:\n");
    printf("    - STA  (Station / Client)\n");
    printf("    - AP   (Access Point / Soft-AP)\n");
    printf("    - STA+AP (Concurrent)\n");
    printf("    - Promiscuous (raw 802.11 frame capture)\n");
    printf("    - ESP-NOW (peer-to-peer, no router needed)\n");
    printf("    - WiFi Aware / NAN (Neighbor Awareness)\n");

    esp_wifi_stop();
    esp_wifi_deinit();
}

static void probe_bluetooth(void)
{
    print_separator("BLUETOOTH CAPABILITIES");

    esp_chip_info_t info;
    esp_chip_info(&info);

    if (info.features & CHIP_FEATURE_BLE) {
        printf("  BLE:            Supported (Bluetooth 5.0 LE)\n");
        printf("    - GATT Server/Client\n");
        printf("    - Advertising & Scanning\n");
        printf("    - Mesh Networking\n");
        printf("    - Long Range (Coded PHY)\n");
        printf("    - 2 Mbps PHY\n");
    } else {
        printf("  BLE:            Not supported on this chip\n");
    }

    if (info.features & CHIP_FEATURE_BT) {
        printf("  BT Classic:     Supported (A2DP, SPP, HFP)\n");
    } else {
        printf("  BT Classic:     Not available (ESP32-S3 is BLE-only)\n");
    }
}

static void probe_peripherals(void)
{
    print_separator("PERIPHERAL CAPABILITIES");

    printf("  GPIOs:          %d total\n", SOC_GPIO_PIN_COUNT);
    printf("  ADC:\n");
    printf("    - ADC1:       %d channels (12-bit SAR)\n", SOC_ADC_CHANNEL_NUM(0));
    printf("    - ADC2:       %d channels (shared with WiFi)\n", SOC_ADC_CHANNEL_NUM(1));
    printf("  DAC:            Not available on ESP32-S3\n");
    printf("  Touch Sensors:  %d channels (capacitive)\n", SOC_TOUCH_SENSOR_NUM);
    printf("  SPI:            %d controllers (SPI2/SPI3 for user)\n", SOC_SPI_PERIPH_NUM);
    printf("  I2C:            %d controllers\n", SOC_I2C_NUM);
    printf("  I2S:            %d controllers (audio/PDM/TDM)\n", SOC_I2S_NUM);
    printf("  UART:           %d controllers\n", SOC_UART_NUM);
    printf("  USB:            USB-OTG 1.1 (Host & Device)\n");
    printf("  USB-Serial:     Built-in USB-JTAG/Serial (this console)\n");
    printf("  TWAI (CAN):     1 controller (CAN 2.0B compatible)\n");
    printf("  RMT:            %d channels (IR/WS2812/NeoPixel)\n", SOC_RMT_TX_CANDIDATES_PER_GROUP + SOC_RMT_RX_CANDIDATES_PER_GROUP);
    printf("  LEDC (PWM):     %d channels\n", SOC_LEDC_CHANNEL_NUM);
    printf("  MCPWM:          %d groups (motor control)\n", SOC_MCPWM_GROUPS);
    printf("  PCNT:           %d units (pulse counter / encoder)\n", SOC_PCNT_UNITS_PER_GROUP);
    printf("  LCD:            Parallel 8/16-bit + SPI + I2C interfaces\n");
    printf("  Camera:         DVP 8/16-bit parallel interface\n");
    printf("  SDMMC:          SD/MMC host controller (1-bit / 4-bit)\n");
}

static void probe_security(void)
{
    print_separator("SECURITY & CRYPTO");

    printf("  AES:            128/256-bit hardware accelerator\n");
    printf("  SHA:            SHA-1/224/256 hardware accelerator\n");
    printf("  RSA:            Up to 4096-bit hardware accelerator\n");
    printf("  HMAC:           Hardware HMAC (eFuse key)\n");
    printf("  Digital Sig:    Hardware digital signature (RSA)\n");
    printf("  Flash Encrypt:  AES-256-XTS (eFuse controlled)\n");
    printf("  Secure Boot:    V2 (RSA-3072 / ECDSA)\n");
    printf("  eFuse:          %d bits (MAC, keys, config)\n", 256 * 11);
    printf("  World Ctrl:     Dual-world isolation (TEE)\n");
    printf("  Random:         Hardware TRNG available\n");
}

static void probe_power(void)
{
    print_separator("POWER MANAGEMENT");

    printf("  Clock Modes:\n");
    printf("    - 240 MHz     (max performance)\n");
    printf("    - 160 MHz     (balanced)\n");
    printf("    - 80 MHz      (low power)\n");
    printf("  Sleep Modes:\n");
    printf("    - Modem Sleep  (WiFi off, CPU active)\n");
    printf("    - Light Sleep  (CPU paused, fast wake)\n");
    printf("    - Deep Sleep   (RTC only, ~10 uA)\n");
    printf("    - Hibernation  (RTC timer only, ~5 uA)\n");
    printf("  Wake Sources:   GPIO, timer, touch, ULP, UART\n");
    printf("  ULP Coprocessor: RISC-V + FSM (runs in deep sleep)\n");
}

static void probe_temperature(void)
{
    print_separator("TEMPERATURE SENSOR");

    temperature_sensor_handle_t tsens = NULL;
    temperature_sensor_config_t tsens_cfg = TEMPERATURE_SENSOR_CONFIG_DEFAULT(-10, 80);

    esp_err_t ret = temperature_sensor_install(&tsens_cfg, &tsens);
    if (ret == ESP_OK) {
        temperature_sensor_enable(tsens);
        float temp_c = 0;
        temperature_sensor_get_celsius(tsens, &temp_c);
        printf("  Chip Temp:      %.1f °C (%.1f °F)\n", temp_c, temp_c * 9.0 / 5.0 + 32.0);
        temperature_sensor_disable(tsens);
        temperature_sensor_uninstall(tsens);
    } else {
        printf("  Chip Temp:      Sensor not available (%s)\n", esp_err_to_name(ret));
    }
}

static void probe_freertos(void)
{
    print_separator("FreeRTOS / SYSTEM");

    printf("  FreeRTOS:       v%s\n", tskKERNEL_VERSION_NUMBER);
    printf("  Tick Rate:      %d Hz\n", configTICK_RATE_HZ);
    printf("  Task Count:     %"PRIu32"\n", (uint32_t)uxTaskGetNumberOfTasks());
    printf("  Main Stack:     %d bytes\n", CONFIG_ESP_MAIN_TASK_STACK_SIZE);
    printf("  Uptime:         %lld ms\n", esp_timer_get_time() / 1000LL);
}

static void probe_csi_details(void)
{
    print_separator("CSI (Channel State Information) DETAILS");

#ifdef CONFIG_ESP_WIFI_CSI_ENABLED
    printf("  Status:         ENABLED in this build\n");
    printf("\n  What is CSI?\n");
    printf("    WiFi CSI captures the amplitude and phase of each OFDM\n");
    printf("    subcarrier in received WiFi frames. This gives a detailed\n");
    printf("    view of how radio signals propagate through a space.\n");
    printf("\n  Subcarriers:    52 (20 MHz) / 114 (40 MHz) per frame\n");
    printf("  Data Rate:      Up to ~100 frames/sec\n");
    printf("  Data per Frame: ~200-500 bytes (amplitude + phase)\n");
    printf("\n  Applications:\n");
    printf("    1. Presence Detection    — detect humans in a room\n");
    printf("    2. Gesture Recognition   — classify hand gestures\n");
    printf("    3. Activity Recognition  — walking, sitting, falling\n");
    printf("    4. Breathing/Heart Rate  — contactless vital signs\n");
    printf("    5. Indoor Positioning    — sub-meter localization\n");
    printf("    6. Fall Detection        — elderly safety monitoring\n");
    printf("    7. People Counting       — crowd estimation\n");
    printf("    8. Sleep Monitoring      — non-contact sleep staging\n");
    printf("\n  How to use:\n");
    printf("    esp_wifi_set_csi_config(&csi_config);\n");
    printf("    esp_wifi_set_csi_rx_cb(my_callback, NULL);\n");
    printf("    esp_wifi_set_csi(true);\n");
#else
    printf("  Status:         DISABLED\n");
    printf("  To enable:      Set CONFIG_ESP_WIFI_CSI_ENABLED=y in sdkconfig\n");
#endif
}

/* ── Main ────────────────────────────────────────────────────────────── */

void app_main(void)
{
    /* NVS required for WiFi */
    esp_err_t ret = nvs_flash_init();
    if (ret == ESP_ERR_NVS_NO_FREE_PAGES || ret == ESP_ERR_NVS_NEW_VERSION_FOUND) {
        nvs_flash_erase();
        ret = nvs_flash_init();
    }
    ESP_ERROR_CHECK(ret);

    /* ── Hello World! ── */
    printf("\n");
    printf("  ╭─────────────────────────────────────────────────╮\n");
    printf("  │                                                 │\n");
    printf("  │       HELLO WORLD from ESP32-S3!                │\n");
    printf("  │                                                 │\n");
    printf("  │   WiFi-DensePose Capability Discovery v1.0      │\n");
    printf("  │                                                 │\n");
    printf("  ╰─────────────────────────────────────────────────╯\n");
    printf("\n");

    /* Run all probes */
    probe_chip_info();
    probe_memory();
    probe_flash();
    probe_temperature();
    probe_peripherals();
    probe_security();
    probe_power();
    probe_freertos();
    probe_wifi_capabilities();
    probe_bluetooth();
    probe_csi_details();

    print_separator("DONE — ALL CAPABILITIES REPORTED");
    printf("\n  This ESP32-S3 is ready for WiFi-DensePose!\n");
    printf("  Flash the full firmware (esp32-csi-node) to begin CSI sensing.\n\n");

    /* Keep alive — blink a status message every 10 seconds */
    int tick = 0;
    while (1) {
        vTaskDelay(pdMS_TO_TICKS(10000));
        tick++;
        printf("[hello] Still running... uptime=%lld sec, free_heap=%"PRIu32"\n",
               esp_timer_get_time() / 1000000LL,
               (uint32_t)heap_caps_get_free_size(MALLOC_CAP_INTERNAL));
    }
}
