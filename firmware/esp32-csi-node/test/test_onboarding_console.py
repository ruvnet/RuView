"""Execute the production console preparation across driver and IDF seams."""
import pathlib
import subprocess
import tempfile
import unittest


class OnboardingConsoleTests(unittest.TestCase):
    def test_driver_lifecycle_and_legacy_boundary(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            (root / "driver").mkdir()
            (root / "esp_err.h").write_text(
                "#pragma once\ntypedef int esp_err_t;\n#define ESP_OK 0\n")
            (root / "esp_idf_version.h").write_text(
                "#pragma once\n#ifndef ESP_IDF_VERSION_MAJOR\n"
                "#define ESP_IDF_VERSION_MAJOR 6\n#endif\n")
            (root / "driver/usb_serial_jtag.h").write_text(
                "#pragma once\n#include <stdbool.h>\n#include <stdint.h>\n"
                "#include \"esp_err.h\"\n"
                "typedef struct { uint32_t tx_buffer_size, rx_buffer_size; } usb_serial_jtag_driver_config_t;\n"
                "#define USB_SERIAL_JTAG_DRIVER_CONFIG_DEFAULT() (usb_serial_jtag_driver_config_t){256,256}\n"
                "bool usb_serial_jtag_is_driver_installed(void);\n"
                "esp_err_t usb_serial_jtag_driver_install(usb_serial_jtag_driver_config_t *config);\n")
            (root / "driver/usb_serial_jtag_vfs.h").write_text(
                "#pragma once\nvoid usb_serial_jtag_vfs_use_driver(void);\n")
            (root / "probe.c").write_text(r'''
#include <assert.h>
#include <stdbool.h>
#include "onboarding_console.h"
#include "driver/usb_serial_jtag.h"
static bool installed;
static int install_result, installs, switches;
bool usb_serial_jtag_is_driver_installed(void) { return installed; }
esp_err_t usb_serial_jtag_driver_install(usb_serial_jtag_driver_config_t *config) {
    assert(config->rx_buffer_size > 0 && config->tx_buffer_size > 0);
    installs++;
    if (install_result == 0) installed = true;
    return install_result;
}
void usb_serial_jtag_vfs_use_driver(void) { assert(installed); switches++; }
int main(void) {
#if defined(CONFIG_ESP_CONSOLE_USB_SERIAL_JTAG) && ESP_IDF_VERSION_MAJOR >= 6
    assert(ruview_onboarding_console_prepare() == 0);
    assert(installs == 1 && switches == 1);
    assert(ruview_onboarding_console_prepare() == 0);
    assert(installs == 1 && switches == 2);
    installed = false;
    install_result = -7;
    assert(ruview_onboarding_console_prepare() == -7);
    assert(installs == 2 && switches == 2);
#else
    assert(ruview_onboarding_console_prepare() == 0);
    assert(installs == 0 && switches == 0);
#endif
    return 0;
}
''')
            production = pathlib.Path(__file__).resolve().parents[1] / "main"
            for defines in [[], ["-DCONFIG_ESP_CONSOLE_USB_SERIAL_JTAG=1"],
                            ["-DCONFIG_ESP_CONSOLE_USB_SERIAL_JTAG=1", "-DESP_IDF_VERSION_MAJOR=5"]]:
                binary = root / "probe"
                subprocess.run(["cc", "-std=c11", "-Wall", "-Wextra", "-Werror",
                                *defines, "-I", str(root), "-I", str(production),
                                str(root / "probe.c"), "-o", str(binary)], check=True)
                subprocess.run([str(binary)], check=True)


if __name__ == "__main__":
    unittest.main()
