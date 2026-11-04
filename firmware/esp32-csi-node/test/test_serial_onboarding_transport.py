"""Execute production onboarding endpoints with bounded IDF driver/NVS test doubles."""
from pathlib import Path
import subprocess
import tempfile
import unittest


class OnboardingTransportTests(unittest.TestCase):
    def test_real_endpoint_parser_nonce_and_driver_boundaries(self):
        production = Path(__file__).resolve().parents[1] / "main"
        headers = {
            "esp_err.h": "typedef int esp_err_t;\n#define ESP_OK 0\n#define ESP_ERR_INVALID_ARG -1\n#define ESP_ERR_NO_MEM -2\nconst char *esp_err_to_name(int);\n",
            "esp_idf_version.h": "#define ESP_IDF_VERSION_MAJOR 6\n",
            "esp_efuse.h": "",
            "esp_log.h": '#define ESP_LOGW(tag, ...) ((void)(tag))\n#define ESP_LOGI(tag, ...) ((void)(tag))\n',
            "esp_mac.h": "#include <stdint.h>\n#include \"esp_err.h\"\nesp_err_t esp_base_mac_addr_get(uint8_t *);\n",
            "esp_system.h": "void esp_restart(void);\n",
            "esp_timer.h": "#include <stdint.h>\nint64_t esp_timer_get_time(void);\n",
            "esp_app_desc.h": "typedef struct { char version[32]; } esp_app_desc_t;\nconst esp_app_desc_t *esp_app_get_description(void);\n",
            "freertos/FreeRTOS.h": "#include <stdint.h>\ntypedef int BaseType_t;\n#define pdPASS 1\n#define pdMS_TO_TICKS(ms) (ms)\n",
            "freertos/task.h": "#include \"freertos/FreeRTOS.h\"\nvoid vTaskDelay(uint32_t);\nBaseType_t xTaskCreate(void (*)(void *), const char *, unsigned, void *, unsigned, void *);\n",
            "nvs.h": "#include <stdint.h>\ntypedef int nvs_handle_t;\n#define NVS_READWRITE 1\nint nvs_open(const char *, int, nvs_handle_t *);\nint nvs_set_str(nvs_handle_t, const char *, const char *);\nint nvs_set_u16(nvs_handle_t, const char *, uint16_t);\nint nvs_set_u8(nvs_handle_t, const char *, uint8_t);\nint nvs_commit(nvs_handle_t);\nvoid nvs_close(nvs_handle_t);\n",
            "psa/crypto.h": "#include <stdint.h>\n#include <stddef.h>\n#define PSA_SUCCESS 0\n#define PSA_ALG_SHA_256 1\nint psa_crypto_init(void);\nint psa_hash_compute(int, const uint8_t *, size_t, uint8_t *, size_t, size_t *);\n",
            "driver/usb_serial_jtag.h": "#include <stdbool.h>\n#include <stdint.h>\n#include <stddef.h>\ntypedef struct { uint32_t tx_buffer_size, rx_buffer_size; } usb_serial_jtag_driver_config_t;\n#define USB_SERIAL_JTAG_DRIVER_CONFIG_DEFAULT() (usb_serial_jtag_driver_config_t){256,256}\nbool usb_serial_jtag_is_driver_installed(void);\nint usb_serial_jtag_driver_install(usb_serial_jtag_driver_config_t *);\nint usb_serial_jtag_read_bytes(void *, uint32_t, uint32_t);\nint usb_serial_jtag_write_bytes(const void *, size_t, uint32_t);\n",
            "driver/usb_serial_jtag_vfs.h": "void usb_serial_jtag_vfs_use_driver(void);\n",
        }
        with tempfile.TemporaryDirectory(prefix="ruview-usb-transport-") as directory:
            root = Path(directory)
            for name, body in headers.items():
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("#pragma once\n" + body)
            (root / "probe.c").write_text(r'''
#include <assert.h>
#include <string.h>
#include <stdio.h>
#include "serial_onboarding.c"
static int64_t clock_us = 1000;
static int commits, reboots, installs, switches, driver_result, tasks;
static bool installed, fail_tx;
static char tx[2048];
static size_t tx_length;
static esp_app_desc_t app = {.version = "0.8.test"};
const esp_app_desc_t *esp_app_get_description(void) { return &app; }
const char *esp_err_to_name(int e) { (void)e; return "stub"; }
int esp_base_mac_addr_get(uint8_t *out) { memset(out, 1, 6); return 0; }
int psa_crypto_init(void) { return 0; }
int psa_hash_compute(int a, const uint8_t *in, size_t n, uint8_t *out, size_t cap, size_t *size) {
    (void)a; (void)in; (void)n; assert(cap >= 32); memset(out, 0xab, 32); *size = 32; return 0;
}
int64_t esp_timer_get_time(void) { return clock_us; }
void esp_restart(void) { reboots++; }
void vTaskDelay(uint32_t ticks) { assert(ticks <= 500); }
BaseType_t xTaskCreate(void (*f)(void *), const char *n, unsigned stack, void *a, unsigned p, void *h) {
    (void)f; (void)n; (void)a; (void)p; (void)h; assert(stack == 4096); tasks++; return 1;
}
int nvs_open(const char *n, int mode, nvs_handle_t *h) { assert(!strcmp(n, "csi_cfg") && mode == 1); *h = 1; return 0; }
int nvs_set_str(nvs_handle_t h, const char *k, const char *v) { (void)h; (void)k; (void)v; return 0; }
int nvs_set_u16(nvs_handle_t h, const char *k, uint16_t v) { (void)h; (void)k; (void)v; return 0; }
int nvs_set_u8(nvs_handle_t h, const char *k, uint8_t v) { (void)h; (void)k; (void)v; return 0; }
int nvs_commit(nvs_handle_t h) { (void)h; commits++; return 0; }
void nvs_close(nvs_handle_t h) { (void)h; }
bool usb_serial_jtag_is_driver_installed(void) { return installed; }
int usb_serial_jtag_driver_install(usb_serial_jtag_driver_config_t *config) {
    assert(config->rx_buffer_size == 512 && config->tx_buffer_size == 1024); installs++;
    if (driver_result) return driver_result;
    installed = true; return 0;
}
void usb_serial_jtag_vfs_use_driver(void) { assert(installed); switches++; }
int usb_serial_jtag_read_bytes(void *out, uint32_t capacity, uint32_t ticks) {
    assert(installed && capacity >= 1 && ticks == 0); ((char *)out)[0] = 'x'; return 1;
}
int usb_serial_jtag_write_bytes(const void *bytes, size_t length, uint32_t ticks) {
    assert(installed && ticks == 20 && tx_length + length < sizeof(tx));
    if (fail_tx) return 0;
    memcpy(tx + tx_length, bytes, length); tx_length += length; tx[tx_length] = 0; return (int)length;
}
static void feed(onboarding_endpoint_t *endpoint, const char *text) {
    consume_bytes(endpoint, (const uint8_t *)text, (ssize_t)strlen(text));
}
int main(void) {
    FILE *capture = tmpfile(); assert(capture); assert(dup2(fileno(capture), STDOUT_FILENO) >= 0);
    assert(serial_onboarding_start(NULL) == ESP_ERR_INVALID_ARG && installs == 0);
    nvs_config_t boot_config = {.node_id = 15, .target_ip = "192.168.1.2", .target_port = 5005, .wifi_ssid = "synthetic"};
    driver_result = -7;
    assert(serial_onboarding_start(&boot_config) == ESP_OK && tasks == 1 && !installed);
    installs = 0; driver_result = 0;
    assert(ruview_onboarding_usb_read((uint8_t *)tx, 1) == 0);
    assert(!ruview_onboarding_usb_write("x", 1));
    assert(ruview_onboarding_console_prepare() == 0 && installs == 1);
    assert(ruview_onboarding_console_prepare() == 0 && installs == 1 && switches == 2);
    uint8_t byte = 0; assert(ruview_onboarding_usb_read(&byte, 1) == 1 && byte == 'x');
    tx_length = 0;
    onboarding_endpoint_t uart = {0}, usb = {.native_usb = true};
    const char *hello = "RUVIEW_HELLO_V1 0123456789abcdef0123456789abcdef\n";
    const char *config = "RUVIEW_CONFIG_V1 0123456789abcdef0123456789abcdef 15 192.168.1.3 5005 KEEP KEEP\n";
    // No endpoint can use the other's challenge, even with identical nonce bytes.
    feed(&usb, hello); assert(strstr(tx, "RUVIEW_HELLO_OK_V1") && usb.issued_nonce[0]);
    feed(&uart, config); assert(commits == 0);
    feed(&usb, config); assert(commits == 1 && reboots == 1 && !usb.issued_nonce[0]);
    feed(&usb, config); assert(commits == 1);
    // UART still works, and its complete response does not enter USB's TX queue.
    size_t before = tx_length; feed(&uart, hello); assert(tx_length == before);
    feed(&usb, config); assert(commits == 1);
    feed(&uart, config); assert(commits == 2 && reboots == 2);
    // Fragmented commands are not combined across endpoints.
    feed(&usb, "RUVIEW_HELLO_V1 ");
    feed(&uart, "0123456789abcdef0123456789abcdef\n");
    assert(!usb.issued_nonce[0]);
    feed(&usb, "0123456789abcdef0123456789abcdef\n"); assert(usb.issued_nonce[0]);
    // A full line overflow is discarded and cleared, then the next HELLO works.
    char flood[600]; memset(flood, 'z', sizeof(flood)); consume_bytes(&usb, (uint8_t *)flood, sizeof(flood));
    feed(&usb, "\n"); assert(usb.length == 0 && !usb.overflow && usb.line[0] == 0);
    feed(&usb, hello); clock_us += ONBOARDING_NONCE_TTL_US + 1;
    feed(&usb, config); assert(commits == 2);
    // A failed HELLO write cannot leave an accepted challenge behind.
    fail_tx = true; feed(&usb, hello); assert(!usb.issued_nonce[0]);
    feed(&usb, config); assert(commits == 2);
    rewind(capture); char uart_output[2048] = {0}; assert(fread(uart_output, 1, sizeof(uart_output)-1, capture) > 0);
    assert(strstr(uart_output, "RUVIEW_HELLO_OK_V1") && strstr(uart_output, "RUVIEW_CONFIG_OK_V1"));
    fclose(capture); return 0;
}
''')
            binary = root / "probe"
            subprocess.run(["cc", "-std=c11", "-Wall", "-Wextra", "-Werror",
                            "-DCONFIG_IDF_TARGET_ESP32C5=1", "-DCONFIG_ESP_CONSOLE_UART=1",
                            "-DCONFIG_RUVIEW_ONBOARDING_NATIVE_USB=1",
                            "-I", str(root), "-I", str(production), str(root / "probe.c"),
                            str(production / "serial_onboarding_protocol.c"), "-o", str(binary)], check=True)
            subprocess.run([str(binary)], check=True)


if __name__ == "__main__":
    unittest.main()
