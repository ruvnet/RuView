"""Synthetic IDF failures through production digest, HELLO and CONFIG admission.

Uses current onboarding_identity.h and exact production function bytes. No
firmware/hardware claim; the protocol parser is linked without replacement.
"""
import pathlib
import subprocess
import tempfile
import unittest


class HelloIdentityTests(unittest.TestCase):
    def test_identity_failure_withholds_hello_and_revokes_nonce(self):
        main = pathlib.Path(__file__).resolve().parents[1] / "main"
        source = (main / "serial_onboarding.c").read_text()

        def function(start, end):
            self.assertEqual(source.count(start), 1)
            self.assertEqual(source.count(end), 1)
            return source[source.index(start):source.index(end)]

        production = function("static bool emit_hello(", "static esp_err_t commit_config(")
        production += function("static void process_line(", "static void consume_bytes(")
        prelude = r'''
#include <assert.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "serial_onboarding_protocol.h"
#include "onboarding_identity.h"
static int fault, commits, restarts;
static char s_issued_nonce[RUVIEW_ONBOARDING_NONCE_HEX_LEN + 1];
static int64_t s_nonce_issued_at_us;
#define ONBOARDING_NONCE_TTL_US (60LL * 1000LL * 1000LL)
static struct { char wifi_ssid[33]; char target_ip[16]; unsigned target_port, node_id; }
    s_current_config = {"synthetic", "192.168.1.2", 5005, 1};
typedef struct { char version[16]; } esp_app_desc_t;
static const esp_app_desc_t *esp_app_get_description(void) {
    static const esp_app_desc_t app = {"test"}; return &app;
}
static const char *chip_name(void) { return "esp32c6"; }
static int64_t esp_timer_get_time(void) { return 1000; }
static const char *esp_err_to_name(esp_err_t result) { (void)result; return "test"; }
static esp_err_t commit_config(const ruview_onboarding_config_t *request) {
    assert(request->preserve_wifi); commits++; return ESP_OK;
}
#define pdMS_TO_TICKS(ms) (ms)
static void vTaskDelay(unsigned ms) { assert(ms == 400); }
static void esp_restart(void) { restarts++; }
esp_err_t esp_base_mac_addr_get(uint8_t *mac) {
    memset(mac, 0x12, 6); return fault == 1 ? -1 : ESP_OK;
}
psa_status_t psa_crypto_init(void) { return fault == 2 ? -1 : PSA_SUCCESS; }
psa_status_t psa_hash_compute(psa_algorithm_t alg, const uint8_t *input,
    size_t input_size, uint8_t *hash, size_t capacity, size_t *length) {
    (void)input; assert(alg == PSA_ALG_SHA_256); assert(input_size == 22);
    assert(capacity == 32); memset(hash, 0xab, capacity);
    *length = fault == 4 ? 31 : 32; return fault == 3 ? -1 : PSA_SUCCESS;
}
'''
        scenario = r'''
static void hello(const char *nonce) {
    char line[128]; snprintf(line, sizeof(line), "RUVIEW_HELLO_V1 %s", nonce);
    process_line(line);
}
static void config(const char *nonce) {
    char line[256]; snprintf(line, sizeof(line),
        "RUVIEW_CONFIG_V1 %s 5 192.168.1.2 5005 KEEP KEEP", nonce);
    process_line(line);
}
int main(void) {
    const char *old_nonce = "00112233445566778899aabbccddeeff";
    const char *failed_nonce = "ffeeddccbbaa99887766554433221100";
    for (int failure = 1; failure <= 4; failure++) {
        fault = 0; hello(old_nonce);
        assert(strcmp(s_issued_nonce, old_nonce) == 0);
        fault = failure; hello(failed_nonce);
        assert(s_issued_nonce[0] == '\0' && s_nonce_issued_at_us == 0);
        config(old_nonce); config(failed_nonce);
        assert(commits == 0 && restarts == 0);
    }
    fault = 0; hello(old_nonce); config(old_nonce);
    assert(commits == 1 && restarts == 1 && s_issued_nonce[0] == '\0');
    config(old_nonce); assert(commits == 1 && restarts == 1);
    return 0;
}
'''
        with tempfile.TemporaryDirectory(prefix="ruview-hello-identity-") as temporary:
            root = pathlib.Path(temporary)
            probe = root / "probe.c"
            probe.write_text(prelude + production + scenario)
            binary = root / "probe"
            subprocess.run([
                "cc", "-std=c11", "-Wall", "-Wextra", "-Werror", "-pedantic",
                "-I", str(main.parent / "test/identity_stubs"), "-I", str(main),
                str(probe), str(main / "serial_onboarding_protocol.c"),
                "-o", str(binary),
            ], check=True)
            result = subprocess.run([str(binary)], check=True, text=True, capture_output=True)
            lines = result.stdout.splitlines()
            self.assertEqual(sum(line.startswith("RUVIEW_HELLO_ERR_V1 ") for line in lines), 4)
            self.assertEqual(sum(line.startswith("RUVIEW_HELLO_OK_V1 ") for line in lines), 5)
            self.assertEqual(sum(line.startswith("RUVIEW_CONFIG_OK_V1 ") for line in lines), 1)
            self.assertTrue(all("device_digest=0000000000000000" not in line for line in lines))
            errors = [line for line in lines if line.startswith("RUVIEW_HELLO_ERR_V1 ")]
            self.assertTrue(all("nonce=ffeeddccbbaa99887766554433221100 reason=identity_unavailable" in line for line in errors))
            # Causal controls: removing either production guard or old-nonce
            # revocation must fail the same end-to-end admission assertions.
            guard = production[production.index('    if (strcmp(digest, "0000000000000000")'):production.index('    const bool configured')]
            revoke = "        s_issued_nonce[0] = '\\0';\n"
            self.assertEqual(production.count(guard), 1)
            self.assertEqual(production.count(revoke), 1)
            for mutant in [production.replace(guard, ""), production.replace(revoke, "")]:
                probe.write_text(prelude + mutant + scenario)
                subprocess.run([
                    "cc", "-std=c11", "-Wall", "-Wextra", "-Werror", "-pedantic",
                    "-I", str(main.parent / "test/identity_stubs"), "-I", str(main),
                    str(probe), str(main / "serial_onboarding_protocol.c"),
                    "-o", str(binary),
                ], check=True)
                bad = subprocess.run([str(binary)], text=True, capture_output=True)
                self.assertNotEqual(bad.returncode, 0, "identity admission mutant survived")



if __name__ == "__main__":
    unittest.main()
