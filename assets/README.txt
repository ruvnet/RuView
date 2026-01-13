WiFi-Mat v3.2 - AI Thermal Monitor + WiFi CSI Sensing
======================================================

Embedded AI system combining thermal monitoring with WiFi-based
presence detection, inspired by WiFi-DensePose technology.

For Heltec ESP32-S3 with OLED Display

CORE CAPABILITIES:
------------------
* Thermal Pattern Learning - Spiking Neural Network (LIF neurons)
* WiFi CSI Sensing - Through-wall motion/presence detection
* Breathing Detection - Respiratory rate from WiFi phase
* Anomaly Detection - Ruvector-inspired attention weights
* HNSW Indexing - Fast O(log n) pattern matching
* Power Optimization - Adaptive sleep modes

VISUAL INDICATORS:
------------------
* Animated motion figure when movement detected
* Radar sweep with detection blips
* Breathing wave visualization with BPM
* Status bar: WiFi/Motion/Alert icons
* Screen flash on anomaly or motion alerts
* Dynamic confidence bars

DISPLAY MODES (cycle with double-tap):
--------------------------------------
1. STATS  - Temperature, zone, patterns, attention level
2. GRAPH  - Temperature history graph (40 samples)
3. PTRNS  - Learned pattern list with scores
4. ANOM   - Anomaly detection with trajectory view
5. AI     - Power optimization metrics
6. CSI    - WiFi CSI motion sensing with radar
7. RF     - RF device presence detection
8. INFO   - Device info, uptime, memory

AI POWER OPTIMIZATION (AI mode):
--------------------------------
* Mode: ACTIVE/LIGHT/DEEP sleep states
* Energy: Estimated power savings (0-95%)
* Neurons: Active vs idle neuron ratio
* HNSW: Hierarchical search efficiency
* Spikes: Neural spike efficiency
* Attn: Pattern attention weights

WIFI CSI SENSING (CSI mode):
----------------------------
Uses WiFi Channel State Information for through-wall sensing:

* MOTION/STILL - Real-time motion detection
* Radar Animation - Sweep with confidence blips
* Breathing Wave - Sine wave + BPM when detected
* Confidence % - Detection confidence level
* Detection Count - Cumulative motion events
* Variance Metrics - Signal variance analysis

Technology based on WiFi-DensePose concepts:
- Phase unwrapping for movement detection
- Amplitude variance for presence sensing
- Frequency analysis for breathing rate
- No cameras needed - works through walls

BUTTON CONTROLS:
----------------
* TAP (quick)     - Learn current thermal pattern
* DOUBLE-TAP      - Cycle display mode
* HOLD 1 second   - Pause/Resume monitoring
* HOLD 2 seconds  - Reset all learned patterns
* HOLD 3+ seconds - Show device info

INSTALLATION:
-------------
1. Connect Heltec ESP32-S3 via USB
2. Run flash.bat (Windows) or flash.ps1 (PowerShell)
3. Enter COM port when prompted (e.g., COM7)
4. Wait for flash to complete (~60 seconds)
5. Device auto-connects to configured WiFi

REQUIREMENTS:
-------------
* espflash tool: cargo install espflash
* Heltec WiFi LoRa 32 V3 (ESP32-S3)
* USB-C cable
* Windows 10/11

WIFI CONFIGURATION:
-------------------
Default network: ruv.net

To change WiFi credentials, edit source and rebuild:
  C:\esp\src\main.rs (lines 43-44)

HARDWARE PINOUT:
----------------
* OLED SDA: GPIO17
* OLED SCL: GPIO18
* OLED RST: GPIO21
* OLED PWR: GPIO36 (Vext)
* Button: GPIO0 (PRG)
* Thermal: MLX90614 on I2C

TECHNICAL SPECS:
----------------
* MCU: ESP32-S3 dual-core 240MHz
* Flash: 8MB
* RAM: 512KB SRAM + 8MB PSRAM
* Display: 128x64 OLED (SSD1306)
* WiFi: 802.11 b/g/n (2.4GHz)
* Bluetooth: BLE 5.0

NEURAL NETWORK:
---------------
* Architecture: Leaky Integrate-and-Fire (LIF)
* Neurons: 16 configurable
* Patterns: Up to 32 learned
* Features: 6 sparse dimensions
* Indexing: 3-layer HNSW hierarchy

SOURCE CODE:
------------
Full Rust source: C:\esp\src\main.rs
WiFi CSI module: C:\esp\src\wifi_csi.rs
Build script: C:\esp\build.ps1

BASED ON:
---------
* Ruvector - Vector database with HNSW indexing
* WiFi-DensePose - WiFi CSI for pose estimation
* esp-rs - Rust on ESP32

LICENSE:
--------
Created with Claude Code
https://github.com/ruvnet/wifi-densepose
