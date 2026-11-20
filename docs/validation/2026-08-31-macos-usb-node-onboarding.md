# Native macOS USB onboarding firmware witness

## Evidence class

MEASURED on 2026-08-31 with one physical ESP32 S3. This is not an ESP32 C6 or
release distribution qualification.

## Hardware and firmware

The board was identified by the flashing tool as ESP32 S3 QFN56 revision 0.2
with 2 MB embedded PSRAM and 8 MB flash. Firmware 0.8.9 was built for ESP32 S3
with the displayless DevKitC defaults. The private configuration region was
backed up before flashing. The flash operation omitted NVS, preserving the
existing WiFi configuration.

No base MAC, WiFi credential, device digest, or raw CSI is retained in this
witness.

## Results

1. The initial native USB scan failed because the ESP-IDF secondary USB
   Serial/JTAG console mirrors output but does not forward input to STDIN.
2. Firmware was corrected to read the bounded onboarding protocol from the
   secondary VFS while retaining STDIN for UART console boards.
3. A direct nonce challenge then returned ESP32 S3, firmware 0.8.9, configured
   state, the current node identity, and the private server route.
4. The Mac app changed the collided identity from 1 to unused identity 5 while
   preserving WiFi. It showed success only after receiving fresh server
   evidence at minus 56 dBm.
5. A five minute monitor collected 299 samples with zero node 5 missing events,
   zero node 5 stale events, zero server health failures, minus 57 to minus 50
   dBm RSSI, and 216 ms maximum packet age.
6. Firmware uptime exceeded 376 seconds. Captured logs advanced from CSI
   callback 10,600 through 11,000, emitted a node 5 synchronization packet, and
   showed ESP NOW transmit failures equal to zero without reboot, watchdog, or
   panic evidence.
7. The final channel isolation hardening image was flashed and challenged
   again. A following 60 sample check had zero missing, stale, or health failure
   events, a maximum packet age of 100 ms, and minus 57 to minus 50 dBm RSSI.

## Software gates

The nonce and configuration parser host test passed, the complete ESP32 S3
firmware image built under ESP-IDF 5.4, and the application image occupied
941,056 bytes with 55 percent of the smallest application partition free.

## Remaining gates

New WiFi credential entry, signed Mac distribution, and cross platform serial
bridges remain unmeasured.

## ESP32 C6 follow up

On 2026-09-11, an ESP32 C6 on `cu.usbserial-3120` completed the Mac app flash,
hello, configure, and verify sequence with firmware 0.8.12. The app preserved
the existing node identity 3 and verified fresh sensing server evidence at
minus 34 dBm. The board returned `RUVIEW_HELLO_OK_V1` within the bounded serial
exchange after the C6 mmWave defaults moved from the UART0 RX conflict on GPIO
17 to GPIO 4 and GPIO 5. This closes only the physical C6 onboarding gate. It
does not qualify new WiFi credential entry or signed distribution.
