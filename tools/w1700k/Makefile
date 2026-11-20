CC ?= cc
CFLAGS ?= -O2 -std=c11 -Wall -Wextra -Werror
BUILD ?= build
.PHONY: all check test clean
all: $(BUILD)/w1700k-diagnostics $(BUILD)/w1700k-fixture
$(BUILD):
	mkdir -p $@
$(BUILD)/w1700k-diagnostics: diagnostics.c vendor/w1700k_stream.h | $(BUILD)
	$(CC) $(CFLAGS) $< -lm -o $@
$(BUILD)/w1700k-fixture: fixture.c vendor/w1700k_stream.h | $(BUILD)
	$(CC) $(CFLAGS) $< -o $@
check test:
	python3 test_bridge.py
clean:
	rm -f $(BUILD)/w1700k-diagnostics $(BUILD)/w1700k-fixture
