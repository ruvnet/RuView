# Host-side unit tests for ADR-081 pure-C logic.
#
# These tests exercise adaptive_controller_decide() and the rv_feature_state
# helpers (CRC32, finalize) using plain gcc/clang, with a minimal esp_err.h
# shim. No ESP-IDF, no FreeRTOS, no QEMU required.
#
# Usage:
#   cd firmware/esp32-csi-node/tests/host
#   make
#   ./test_adaptive_controller
#   ./test_rv_feature_state

MAIN_DIR := ../../main
CC      ?= cc
CFLAGS  ?= -O2 -std=c11 -Wall -Wextra -Wno-unused-parameter \
           -D_POSIX_C_SOURCE=199309L \
           -I. -I$(MAIN_DIR)
LDLIBS  ?= -lrt

# Pure-C sources under test. We compile only the files that have no
# ESP-IDF dependency in their bodies: rv_feature_state.c is 100% pure.
# adaptive_controller.c uses FreeRTOS for the timer plumbing, so for the
# host test we compile only the decide() portion by isolating it in a
# small unity file (TEST_ADAPT_PURE below).
FEATURE_STATE_SRCS := $(MAIN_DIR)/rv_feature_state.c

# adaptive_controller.c pulls in FreeRTOS headers that don't exist on
# host; we include its decide() function by defining TEST_ADAPT_PURE
# before including the .c. The decide() body itself has no ESP-IDF deps.
# Simpler: just recompile decide() here via a small shim.

TESTS := test_adaptive_controller test_rv_feature_state test_rv_mesh

all: $(TESTS)

test_adaptive_controller: test_adaptive_controller.c $(MAIN_DIR)/adaptive_controller_decide.c $(MAIN_DIR)/adaptive_controller.h $(MAIN_DIR)/rv_radio_ops.h
	$(CC) $(CFLAGS) test_adaptive_controller.c $(MAIN_DIR)/adaptive_controller_decide.c -o $@ $(LDLIBS)

test_rv_feature_state: test_rv_feature_state.c $(FEATURE_STATE_SRCS) $(MAIN_DIR)/rv_feature_state.h $(MAIN_DIR)/rv_radio_ops.h
	$(CC) $(CFLAGS) test_rv_feature_state.c $(FEATURE_STATE_SRCS) -o $@ $(LDLIBS)

# Mesh plane encoder/decoder: compile rv_mesh.c with RV_MESH_HOST_TEST
# so the firmware-only send helpers (stream_sender, esp_log) are hidden.
test_rv_mesh: test_rv_mesh.c $(MAIN_DIR)/rv_mesh.c $(MAIN_DIR)/rv_mesh.h $(FEATURE_STATE_SRCS) $(MAIN_DIR)/rv_radio_ops.h
	$(CC) $(CFLAGS) -DRV_MESH_HOST_TEST=1 \
	    test_rv_mesh.c $(MAIN_DIR)/rv_mesh.c $(FEATURE_STATE_SRCS) \
	    -o $@ $(LDLIBS)

check: all
	./test_adaptive_controller
	@echo ""
	./test_rv_feature_state
	@echo ""
	./test_rv_mesh

clean:
	rm -f $(TESTS) *.o

.PHONY: all check clean
