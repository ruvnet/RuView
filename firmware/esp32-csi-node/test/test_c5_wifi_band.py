"""Execute the actual STA_START branch for band/order/failure controls."""
import pathlib
import subprocess
import tempfile
import unittest


class C5WifiBandTests(unittest.TestCase):
    def test_default_experimental_and_other_chip_startup(self):
        source = (pathlib.Path(__file__).resolve().parents[1] / "main/main.c").read_text()
        marker = "if (event_base == WIFI_EVENT && event_id == WIFI_EVENT_STA_START) {"
        start = source.index(marker) + len(marker)
        end = source.index("} else if (event_base == WIFI_EVENT", start)
        branch = source[start:end]
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            probe = root / "probe.c"
            probe.write_text(r'''
#include <assert.h>
enum { WIFI_BAND_MODE_2G_ONLY = 2, WIFI_BAND_MODE_5G_ONLY = 5,
       WIFI_IF_STA = 0, WIFI_BW20 = 1 };
static int configured_band, connected, calls, failure, bandwidth_failure;
static int esp_wifi_set_band_mode(int band) {
    assert(calls == 0 && connected == 0);
    configured_band = band;
    calls++;
    return failure;
}
static int esp_wifi_set_bandwidth(int iface, int width) {
    assert(iface == WIFI_IF_STA && width == WIFI_BW20);
    assert(calls == 1 && connected == 0);
    calls++;
    return bandwidth_failure;
}
static void esp_wifi_connect(void) {
#if defined(CONFIG_IDF_TARGET_ESP32C5)
#if defined(CONFIG_C5_MANAGED_LLTF8_V1)
    assert(calls == 2);
#else
    assert(calls == 1);
#endif
#else
    assert(calls == 0);
#endif
    connected++;
}
#define ESP_ERROR_CHECK(expr) do { if ((expr) != 0) return; } while (0)
static void start_wifi(void) {
''' + branch + r'''
}
int main(void) {
    start_wifi();
    assert(connected == 1);
#if defined(CONFIG_IDF_TARGET_ESP32C5)
#if defined(CONFIG_C5_WIFI_5GHZ_ONLY)
    assert(configured_band == 5);
#else
    assert(configured_band == 2);
#endif
    connected = calls = configured_band = 0;
    failure = -1;
    start_wifi();
    assert(calls == 1 && connected == 0);
#if defined(CONFIG_C5_MANAGED_LLTF8_V1)
    calls = 0; failure = 0; bandwidth_failure = -7;
    start_wifi();
    assert(calls == 2 && connected == 0);
#endif
#else
    assert(configured_band == 0);
#endif
    return 0;
}
''')
            for defines in [[], ["-DCONFIG_IDF_TARGET_ESP32C5=1"],
                            ["-DCONFIG_IDF_TARGET_ESP32C5=1", "-DCONFIG_C5_WIFI_5GHZ_ONLY=1"],
                            ["-DCONFIG_IDF_TARGET_ESP32C5=1", "-DCONFIG_C5_WIFI_5GHZ_ONLY=1",
                             "-DCONFIG_C5_MANAGED_LLTF8_V1=1"]]:
                binary = root / "probe"
                subprocess.run(["cc", "-std=c11", "-Wall", "-Wextra", "-Werror",
                                "-Wno-unused-function", *defines, str(probe), "-o", str(binary)], check=True)
                subprocess.run([str(binary)], check=True)


if __name__ == "__main__":
    unittest.main()
