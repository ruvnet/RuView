"""Preprocess the production LED block to prove the profile cannot drive it."""
import pathlib
import subprocess
import tempfile
import unittest


class S3HeadlessLedTests(unittest.TestCase):
    def test_bare_profile_removes_led_initialization_without_changing_defaults(self):
        source = (pathlib.Path(__file__).resolve().parents[1] / "main/main.c").read_text()
        start = source.index("#if !defined(CONFIG_S3_DISABLE_STATUS_LED)")
        end_marker = "#endif /* C5 LED wiring is opt-in; never drive an unknown board pin. */"
        end = source.index(end_marker, start) + len(end_marker)
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / "led.c"
            path.write_text(source[start:end])
            for defines, enabled in [
                (["-DCONFIG_IDF_TARGET_ESP32S3=1"], True),
                (["-DCONFIG_IDF_TARGET_ESP32S3=1", "-DCONFIG_S3_DISABLE_STATUS_LED=1"], False),
                (["-DCONFIG_IDF_TARGET_ESP32C6=1"], True),
                (["-DCONFIG_IDF_TARGET_ESP32C5=1", "-DCONFIG_C5_STATUS_LED_GPIO=-1"], False),
            ]:
                out = subprocess.check_output(["cc", "-E", "-P", *defines, str(path)], text=True)
                self.assertEqual("led_strip_new_rmt_device" in out, enabled)


if __name__ == "__main__":
    unittest.main()
