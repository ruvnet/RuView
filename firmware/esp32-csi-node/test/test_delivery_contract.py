#!/usr/bin/env python3
"""Static delivery contracts that do not require an ESP-IDF toolchain."""

from pathlib import Path
import unittest


FIRMWARE_ROOT = Path(__file__).resolve().parents[1]
REPOSITORY_ROOT = FIRMWARE_ROOT.parents[1]


class FirmwareDeliveryContractTests(unittest.TestCase):
    def test_c6_uses_native_usb_serial_jtag_as_primary_console(self) -> None:
        config = (FIRMWARE_ROOT / "sdkconfig.defaults.esp32c6").read_text()
        self.assertIn("CONFIG_ESP_CONSOLE_USB_SERIAL_JTAG=y", config)
        self.assertNotIn("CONFIG_ESP_CONSOLE_SECONDARY_USB_SERIAL_JTAG=y", config)

        onboarding = (FIRMWARE_ROOT / "main/serial_onboarding.c").read_text()
        self.assertNotIn('open("/dev/secondary"', onboarding)

    def test_tracked_c6_onboarding_app_matches_version_txt(self) -> None:
        expected = (FIRMWARE_ROOT / "version.txt").read_text().strip().encode()
        app = (FIRMWARE_ROOT / "release_bins/c6-onboarding/esp32-csi-node.bin").read_bytes()
        self.assertTrue(
            expected + b"\0" in app,
            f"tracked C6 app does not embed version {expected.decode()}",
        )

    def test_firmware_tags_publish_downloadable_release_assets(self) -> None:
        workflow = (REPOSITORY_ROOT / ".github/workflows/firmware-ci.yml").read_text()
        self.assertIn("publish-release-assets:", workflow)
        self.assertIn("esp32-csi-node-firmware-*", workflow)
        self.assertIn("softprops/action-gh-release@", workflow)


if __name__ == "__main__":
    unittest.main()
