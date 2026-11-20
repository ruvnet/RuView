#!/usr/bin/env python3
"""Static delivery contracts that do not require an ESP-IDF toolchain."""

from pathlib import Path
import argparse
import hashlib
import json
import sys
import tempfile
import unittest


FIRMWARE_ROOT = Path(__file__).resolve().parents[1]
REPOSITORY_ROOT = FIRMWARE_ROOT.parents[1]
IMAGE_CHIP_IDS = {"esp32s3": 9, "esp32c6": 13, "esp32c5": 23}


def app_version(path: Path) -> str:
    """Read the first segment's ESP-IDF app descriptor, not arbitrary strings.

    IDF image header + segment header occupy 32 bytes; esp_app_desc_t has
    magic 0xABCD5432 and a 32-byte version at descriptor offset 16.
    """
    with path.open("rb") as stream:
        header = stream.read(80)
    if (len(header) != 80 or header[0] != 0xE9
            or header[32:36] != bytes.fromhex("3254cdab")):
        raise ValueError("missing ESP-IDF application descriptor")
    return header[48:80].split(b"\0", 1)[0].decode("ascii")


def verify_app(path: Path, expected: str, target: str | None = None) -> str:
    """Refuse a correctly versioned image built for the wrong release target."""
    actual = app_version(path)
    if actual != expected:
        raise ValueError(f"built application version {actual!r} does not match {expected!r}")
    if target is not None:
        with path.open("rb") as stream:
            header = stream.read(24)
        chip_id = int.from_bytes(header[12:14], "little")
        if chip_id != IMAGE_CHIP_IDS[target]:
            raise ValueError(f"application chip ID {chip_id} does not match {target}")
    return actual


def verify_flash_layout(path: Path, target: str) -> None:
    """Match IDF's generated plan, including the C5-specific ROM boot address."""
    if path.stat().st_size > 65536:
        raise ValueError("oversized flash layout")
    layout = json.loads(path.read_text(encoding="utf-8"))
    expected = {
        0x2000 if target == "esp32c5" else 0x0: "bootloader/bootloader.bin",
        0x8000: "partition_table/partition-table.bin",
        0xF000: "ota_data_initial.bin",
        0x20000: "esp32-csi-node.bin",
    }
    if layout.get("extra_esptool_args", {}).get("chip") != target:
        raise ValueError("flash layout chip does not match release target")
    actual = {int(offset, 0): filename for offset, filename in layout.get("flash_files", {}).items()}
    if actual != expected:
        raise ValueError(f"unexpected {target} flash layout: {actual}")


class FirmwareDeliveryContractTests(unittest.TestCase):
    def test_c5_rom_bootloader_address_cannot_reuse_s3_c6_layout(self) -> None:
        layout = {
            "extra_esptool_args": {"chip": "esp32c5"},
            "flash_files": {
                "0x2000": "bootloader/bootloader.bin",
                "0x8000": "partition_table/partition-table.bin",
                "0xf000": "ota_data_initial.bin",
                "0x20000": "esp32-csi-node.bin",
            },
        }
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "flasher_args.json"
            path.write_text(json.dumps(layout), encoding="utf-8")
            verify_flash_layout(path, "esp32c5")
            layout["flash_files"]["0x0"] = layout["flash_files"].pop("0x2000")
            path.write_text(json.dumps(layout), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "flash layout"):
                verify_flash_layout(path, "esp32c5")
    def test_release_target_rejects_same_version_from_another_chip(self) -> None:
        # Synthetic image descriptor, not firmware/hardware qualification.
        image = bytearray(80)
        image[0] = 0xE9
        image[12:14] = (23).to_bytes(2, "little")
        image[32:36] = bytes.fromhex("3254cdab")
        image[48:53] = b"1.2.3"
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "app.bin"
            path.write_bytes(image)
            self.assertEqual(verify_app(path, "1.2.3", "esp32c5"), "1.2.3")
            for target in ("esp32s3", "esp32c6"):
                with self.assertRaisesRegex(ValueError, "chip ID"):
                    verify_app(path, "1.2.3", target)
            with self.assertRaisesRegex(ValueError, "version"):
                verify_app(path, "1.2.4", "esp32c5")
    def test_c6_uses_native_usb_serial_jtag_as_primary_console(self) -> None:
        config = (FIRMWARE_ROOT / "sdkconfig.defaults.esp32c6").read_text(encoding="utf-8")
        self.assertIn("CONFIG_ESP_CONSOLE_USB_SERIAL_JTAG=y", config)
        self.assertNotIn("CONFIG_ESP_CONSOLE_SECONDARY_USB_SERIAL_JTAG=y", config)

        onboarding = (FIRMWARE_ROOT / "main/serial_onboarding.c").read_text(encoding="utf-8")
        self.assertNotIn('open("/dev/secondary"', onboarding)

    def test_historical_c6_snapshot_matches_its_own_version_and_checksums(self) -> None:
        snapshot = FIRMWARE_ROOT / "release_bins/c6-onboarding"
        expected = (snapshot / "version.txt").read_text(encoding="utf-8").strip()
        self.assertEqual(app_version(snapshot / "esp32-csi-node.bin"), expected)
        for line in (snapshot / "SHA256SUMS.txt").read_text(encoding="ascii").splitlines():
            digest, filename = line.split()
            self.assertEqual(hashlib.sha256((snapshot / filename).read_bytes()).hexdigest(), digest)

    def test_app_version_rejects_truncated_or_wrong_descriptor(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "app.bin"
            for data in [b"", b"0.8.14\0" * 20, b"\xe9" + b"\0" * 79]:
                path.write_bytes(data)
                with self.assertRaises(ValueError):
                    app_version(path)

    def test_firmware_tags_publish_downloadable_release_assets(self) -> None:
        workflow = (REPOSITORY_ROOT / ".github/workflows/firmware-ci.yml").read_text(encoding="utf-8")
        self.assertIn("publish-release-assets:", workflow)
        self.assertIn("esp32-csi-node-firmware-*", workflow)
        self.assertIn("softprops/action-gh-release@", workflow)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--built-app", type=Path)
    parser.add_argument("--target", choices=IMAGE_CHIP_IDS)
    parser.add_argument("--flasher-args", type=Path)
    args, remaining = parser.parse_known_args()
    if args.flasher_args and not (args.built_app and args.target):
        parser.error("--flasher-args requires --built-app and --target")
    if args.built_app:
        expected = (FIRMWARE_ROOT / "version.txt").read_text(encoding="utf-8").strip()
        try:
            actual = verify_app(args.built_app, expected, args.target)
            if args.flasher_args:
                verify_flash_layout(args.flasher_args, args.target)
        except ValueError as error:
            raise SystemExit(str(error)) from error
        print(f"PASS: built application descriptor version {actual}, target={args.target or 'unchecked'}")
    elif args.target:
        parser.error("--target requires --built-app")
    unittest.main(argv=[sys.argv[0], *remaining])
