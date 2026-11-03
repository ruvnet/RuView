.PHONY: all build build-server clean start stop test scan run run-verbose run-presence listen install-helper dashboard help

HELPER := mac_wifi
BRIDGE := target/release/macos-rssi-bridge

# Sensing-server expectations.
TARGET_HOST ?= 127.0.0.1
TARGET_PORT ?= 5005
INTERVAL    ?= 1.5

# Sensing-server (built from the v2 workspace and run from v2/ so it can
# resolve its default --ui-path of ../ui).
SENSING_SERVER_DIR := ../../v2
SENSING_SERVER_BIN := target/release/sensing-server
SENSING_UI_URL     := http://localhost:8080
BRIDGE_DASHBOARD   := http://localhost:9090/dashboard

all: build

help:
	@echo "macos-rssi-bridge — Mac WiFi card → RuView sensing-server bridge"
	@echo ""
	@echo "  make start         build + run everything: sensing-server + bridge + presence + open UIs"
	@echo "  make stop          kill any running sensing-server / bridge / presence processes"
	@echo "  make build         compile mac_wifi (Swift) + macos-rssi-bridge (Rust)"
	@echo "  make build-server  compile the v2 sensing-server (release)"
	@echo "  make scan          run a single multi-BSSID scan, print JSON"
	@echo "  make run           start the bridge → udp://$(TARGET_HOST):$(TARGET_PORT) + http://localhost:9090"
	@echo "  make run-verbose   same, with per-frame stats on stderr"
	@echo "  make run-presence  poll bridge /aps, POST single pose to sensing-server /api/v1/pose/external"
	@echo "  make dashboard     open the multistatic RF tomography UI in your browser"
	@echo "  make listen        debug: print frames arriving on $(TARGET_PORT) (no sensing-server)"
	@echo "  make test          run Rust unit tests"
	@echo "  make clean         delete build artifacts"
	@echo ""
	@echo "Variables: TARGET_HOST=$(TARGET_HOST) TARGET_PORT=$(TARGET_PORT) INTERVAL=$(INTERVAL)"

build: $(HELPER) $(BRIDGE)

$(HELPER): mac_wifi.swift
	swiftc -O mac_wifi.swift -o $(HELPER)

$(BRIDGE): src/main.rs Cargo.toml
	cargo build --release

build-server:
	cd $(SENSING_SERVER_DIR) && cargo build --release -p wifi-densepose-sensing-server --no-default-features

# One-shot: builds everything, starts sensing-server + bridge + presence
# injector under a single process group, opens both UIs, waits for Ctrl-C,
# and kills all three children cleanly. The sensing-server is run with
# cwd=v2/ so its default --ui-path of ../ui resolves; the bridge stays in
# this directory so ./mac_wifi works.
#
# --source esp32 is required: the server's "auto" probe only binds UDP if it
# detects frames *before* the bridge starts sending. The bridge IS the ESP32
# source (synthetic, but same wire format), so pin it explicitly. Otherwise
# the server falls back to `simulate`, never binds UDP, and you get
# ECONNREFUSED on the bridge + a UI showing simulated (fake) motion.
#
# presence_to_pose.py is required for the 3D Observatory: the sensing-server
# computes motion stats from CSI frames, but rendering a single coherent
# figure (instead of the placeholder five-skeleton fallback) needs an
# explicit POST to /api/v1/pose/external. The script polls the bridge's
# /aps endpoint and translates per-AP RSSI variance into one honest pose.
start: build build-server
	@$(MAKE) -s stop >/dev/null 2>&1 || true
	@echo "[start] sensing-server + macos-rssi-bridge + presence injector → $(SENSING_UI_URL)"
	@trap 'echo; echo "[start] stopping…"; kill $$SERVER_PID $$BRIDGE_PID $$PRESENCE_PID 2>/dev/null; wait 2>/dev/null; exit 0' INT TERM; \
	  ( cd $(SENSING_SERVER_DIR) && ./$(SENSING_SERVER_BIN) \
	      --source esp32 --udp-port $(TARGET_PORT) ) & \
	  SERVER_PID=$$!; \
	  echo "[start] sensing-server pid=$$SERVER_PID (source=esp32) — waiting 3s for UDP bind…"; \
	  sleep 3; \
	  ./$(BRIDGE) --helper ./$(HELPER) --target-host $(TARGET_HOST) \
	    --target-port $(TARGET_PORT) --interval $(INTERVAL) --verbose & \
	  BRIDGE_PID=$$!; \
	  echo "[start] bridge pid=$$BRIDGE_PID — dashboard: $(BRIDGE_DASHBOARD) — waiting 2s for /aps…"; \
	  sleep 2; \
	  python3 presence_to_pose.py --bridge-url http://127.0.0.1:9090/aps \
	    --server-url $(SENSING_UI_URL)/api/v1/pose/external --verbose & \
	  PRESENCE_PID=$$!; \
	  echo "[start] presence injector pid=$$PRESENCE_PID — feeding /api/v1/pose/external @ 10 Hz"; \
	  ( sleep 2; open $(SENSING_UI_URL) 2>/dev/null; open $(BRIDGE_DASHBOARD) 2>/dev/null ) & \
	  echo "[start] press Ctrl-C to stop all three"; \
	  wait

stop:
	-@pkill -f "$(SENSING_SERVER_BIN)" 2>/dev/null && echo "[stop] sensing-server killed" || echo "[stop] no sensing-server"
	-@pkill -f "$(BRIDGE)"             2>/dev/null && echo "[stop] bridge killed"          || echo "[stop] no bridge"
	-@pkill -f "presence_to_pose.py"   2>/dev/null && echo "[stop] presence injector killed" || echo "[stop] no presence injector"

test:
	cargo test --release

scan: $(HELPER)
	./$(HELPER) --scan-once

run: build
	./$(BRIDGE) --helper ./$(HELPER) --target-host $(TARGET_HOST) \
		--target-port $(TARGET_PORT) --interval $(INTERVAL)

run-verbose: build
	./$(BRIDGE) --helper ./$(HELPER) --target-host $(TARGET_HOST) \
		--target-port $(TARGET_PORT) --interval $(INTERVAL) --verbose

# Standalone presence injector. Requires the bridge to already be running
# (so /aps responds) and the sensing-server to be up on $(SENSING_UI_URL).
run-presence:
	python3 presence_to_pose.py --bridge-url http://127.0.0.1:9090/aps \
	  --server-url $(SENSING_UI_URL)/api/v1/pose/external --verbose

# Open the RF tomography dashboard. Assumes the bridge is already running
# (start it with `make run` or `make run-verbose` in another terminal).
dashboard:
	@open http://localhost:9090/dashboard 2>/dev/null || \
	  echo "Open http://localhost:9090/dashboard in your browser"

# Debug helper — prints magic/seq/rssi for each frame the bridge emits.
# Doesn't require sensing-server.
listen:
	@python3 listen.py $(TARGET_PORT)

# Optional: install the Swift helper system-wide so anything on $PATH can find it,
# matching the default lookup of `MacosCoreWlanScanner::new()` in the Rust crate.
install-helper: $(HELPER)
	install -m 0755 $(HELPER) /usr/local/bin/mac_wifi
	@echo "[install] /usr/local/bin/mac_wifi"

clean:
	rm -f $(HELPER)
	cargo clean
