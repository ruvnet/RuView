# Makefile for ESP32 CSI firmware fuzz testing targets (ADR-061 Layer 6).
#
# Requirements:
#   - clang with libFuzzer support (clang 6.0+)
#   - Linux or macOS (host-based fuzzing, no ESP-IDF needed)
#
# Usage:
#   make all                  # Build all fuzz targets
#   make fuzz_serialize       # Build serialize target only
#   make fuzz_edge            # Build edge enqueue target only
#   make fuzz_nvs             # Build NVS config target only
#   make run_serialize        # Build and run serialize fuzzer (30s)
#   make run_edge             # Build and run edge fuzzer (30s)
#   make run_nvs              # Build and run NVS fuzzer (30s)
#   make run_all              # Run all fuzzers (30s each)
#   make clean                # Remove build artifacts
#
# Environment variables:
#   FUZZ_DURATION=60          # Override fuzz duration in seconds
#   FUZZ_JOBS=4               # Parallel fuzzing jobs

CC       = clang
# ADR-110: -DCONFIG_CSI_FRAME_HE_TAGGING=1 enables the byte-18/19 HE path
# in csi_collector.c so the fuzzer exercises that code as well as the
# legacy zero-fill path. CONFIG_SOC_WIFI_HE_SUPPORT is left UNSET to
# exercise the legacy S3 branch (sig_mode/cwb/stbc). Add it to CFLAGS for
# a parallel HE-stub build if you want fuzz coverage of the C6 branch.
CFLAGS   = -fsanitize=fuzzer,address,undefined -g -O1 \
           -Istubs -I../main \
           -DCONFIG_CSI_NODE_ID=1 \
           -DCONFIG_CSI_WIFI_CHANNEL=6 \
           -DCONFIG_CSI_WIFI_SSID=\"test\" \
           -DCONFIG_CSI_TARGET_IP=\"192.168.1.1\" \
           -DCONFIG_CSI_TARGET_PORT=5500 \
           -DCONFIG_ESP_WIFI_CSI_ENABLED=1 \
           -DCONFIG_CSI_FRAME_HE_TAGGING=1 \
           -Wno-unused-function

STUBS_SRC = stubs/esp_stubs.c
MAIN_DIR  = ../main

# Default fuzz duration (seconds) and jobs
FUZZ_DURATION ?= 30
FUZZ_JOBS     ?= 1

.PHONY: all clean run_serialize run_edge run_nvs run_all test_adr110 run_adr110 \
        test_vitals run_vitals test_mmwave_detect run_mmwave_detect host_tests \
        test_thermal run_thermal test_edge_grid run_edge_grid

all: fuzz_serialize fuzz_edge fuzz_nvs test_adr110 test_vitals test_mmwave_detect \
     test_thermal \
     test_edge_grid

# --- ADR-110 encoding unit tests ---
# Host-side, no libFuzzer needed — plain C99 deterministic table tests
# for mac_to_eui64() and PPDU-type → ADR-018 byte 18 mapping.
# Builds with stock cc/gcc/clang — runs in CI on Ubuntu.
test_adr110: test_adr110_encoding.c
	cc -std=c99 -Wall -Wextra -o $@ $<

run_adr110: test_adr110
	./test_adr110

# --- Vitals count + presence logic unit tests (issue #998 / #996) ---
# Host-side, no libFuzzer. Pins the person-count gate (no over-count for one
# body) and the presence hysteresis (no flicker on a dithering score). Pulls
# the named tuning constants from ../main/edge_processing.h so the test and the
# firmware can never disagree on thresholds.
test_vitals: test_vitals_count_presence.c $(MAIN_DIR)/edge_processing.h
	cc -std=c99 -Wall -Wextra -Istubs -I$(MAIN_DIR) -o $@ $< -lm

run_vitals: test_vitals
	./test_vitals

# --- mmWave LD2410 detection predicate (#1135 bug #2) ---
# Host-side, no libFuzzer. Proves a floating-UART head pattern (0xF4F3F2F1)
# without a valid frame length+tail is REJECTED, so a phantom LD2410 is never
# detected on a node with no sensor wired. Tests the real predicate the
# firmware uses (../main/mmwave_detect.h) — test and firmware can't disagree.
test_mmwave_detect: test_mmwave_detect.c $(MAIN_DIR)/mmwave_detect.h
	cc -std=c99 -Wall -Wextra -I$(MAIN_DIR) -o $@ $<

run_mmwave_detect: test_mmwave_detect
	./test_mmwave_detect

# --- Thermal state hysteresis ---
# Host-side. Exercises thermal_next_state() from ../main/thermal.h directly --
# the same function the device build runs -- so the test and the firmware cannot
# drift. Pins every edge in both directions, including the CRITICAL -> THROTTLED
# flap that had no hysteresis at all.
test_thermal: test_thermal_hysteresis.c $(MAIN_DIR)/thermal.h
	cc -std=c99 -Wall -Wextra -Istubs -I$(MAIN_DIR) -o $@ $<

run_thermal: test_thermal
	./test_thermal

# --- Edge subcarrier grid (HE20 truncation regression) ---
# Host-side. Pins EDGE_MAX_SUBCARRIERS against ../main/edge_processing.h so the
# test and the firmware cannot disagree about how wide a CSI frame may be.
#
# Built TWICE on purpose. The constant is target-conditional -- 256 on HE-capable
# parts (C6/C5), 128 pre-HE (S3) -- so a single compilation can only ever prove
# one branch. The HE build is the regression that failed before the fix; the
# pre-HE build guards against fixing C6 by spending .bss on every S3.
test_edge_grid: test_edge_subcarrier_grid.c $(MAIN_DIR)/edge_processing.h
	cc -std=c99 -Wall -Wextra -Istubs_he -Istubs -I$(MAIN_DIR) -DEXPECT_HE=1 -o test_edge_grid_he $<
	cc -std=c99 -Wall -Wextra -Istubs -I$(MAIN_DIR) -o test_edge_grid_pre_he $<

run_edge_grid: test_edge_grid
	./test_edge_grid_he
	./test_edge_grid_pre_he

host_tests: run_adr110 run_vitals run_mmwave_detect run_thermal run_edge_grid
	@echo "Host tests passed (ADR-110 + vitals #998/#996 + mmwave detect #1135 + thermal + edge grid)"

# --- Serialize fuzzer ---
# Tests csi_serialize_frame() with random wifi_csi_info_t inputs.
# Links against the real csi_collector.c (with stubs for ESP-IDF).
fuzz_serialize: fuzz_csi_serialize.c $(MAIN_DIR)/csi_collector.c $(STUBS_SRC)
	$(CC) $(CFLAGS) $^ -o $@ -lm

# --- Edge enqueue fuzzer ---
# Tests the SPSC ring buffer push/pop logic with rapid-fire enqueues.
# Self-contained: reproduces ring buffer logic from edge_processing.c.
fuzz_edge: fuzz_edge_enqueue.c $(STUBS_SRC)
	$(CC) $(CFLAGS) $^ -o $@ -lm

# --- NVS config validation fuzzer ---
# Tests all NVS config validation ranges with random values.
# Self-contained: reproduces validation logic from nvs_config.c.
fuzz_nvs: fuzz_nvs_config.c $(STUBS_SRC)
	$(CC) $(CFLAGS) $^ -o $@ -lm

# --- Run targets ---
run_serialize: fuzz_serialize
	@mkdir -p corpus_serialize
	./fuzz_serialize corpus_serialize/ -max_total_time=$(FUZZ_DURATION) -max_len=2048 -jobs=$(FUZZ_JOBS)

run_edge: fuzz_edge
	@mkdir -p corpus_edge
	./fuzz_edge corpus_edge/ -max_total_time=$(FUZZ_DURATION) -max_len=4096 -jobs=$(FUZZ_JOBS)

run_nvs: fuzz_nvs
	@mkdir -p corpus_nvs
	./fuzz_nvs corpus_nvs/ -max_total_time=$(FUZZ_DURATION) -max_len=256 -jobs=$(FUZZ_JOBS)

run_all: run_serialize run_edge run_nvs

clean:
	rm -f fuzz_serialize fuzz_edge fuzz_nvs test_adr110 test_vitals
	rm -rf corpus_serialize/ corpus_edge/ corpus_nvs/
