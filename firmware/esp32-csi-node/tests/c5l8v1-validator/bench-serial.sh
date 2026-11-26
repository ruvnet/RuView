#!/usr/bin/env bash
# usage: bench-serial.sh <port> [seconds] [--reset]
# USB serial frame-format check (no Wi-Fi): reads the console of a flashed C5L8/v1 build and
# validates every C5L8-SELFTEST-SYNTHETIC frame with the server validator (c5l8-check). It
# never writes flash. The self-test prints at boot, so pass --reset: esptool then resets the
# chip with a watchdog reset. On the C5's USB-Serial-JTAG an RTS pulse latches ROM download
# mode, so the reader opens the port with DTR/RTS deasserted and never toggles them.
# PYTHON must have pyserial and esptool (the ESP-IDF python env does). Log: ./bench-logs/.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
port=${1:?serial port, e.g. /dev/cu.usbmodemXXXX}; secs=${2:-20}; reset=${3:-}
py=${PYTHON:-python3}
mkdir -p "$here/bench-logs"; log="$here/bench-logs/c5l8-serial-$(date +%Y%m%d-%H%M%S).log"
if [ "$reset" = "--reset" ]; then
  "$py" -m esptool --chip esp32c5 --port "$port" --after watchdog-reset chip-id >"$log.reset" 2>&1 \
    || { echo "reset failed, see $log.reset" >&2; exit 1; }
fi
"$py" - "$port" "$secs" "$log" <<'PY'
import sys, time, serial
port, secs, log = sys.argv[1], float(sys.argv[2]), sys.argv[3]
def opened():
    s = serial.Serial()
    s.port, s.baudrate, s.timeout = port, 115200, 0.2
    s.dtr = False; s.rts = False      # do not toggle reset/boot lines on open
    s.open()
    return s
s = opened()
end = time.time() + secs
with open(log, "wb") as f:
    while time.time() < end:
        try:
            f.write(s.read(4096))
        except (serial.SerialException, OSError):   # USB re-enumerated after reset
            try: s.close()
            except Exception: pass
            while time.time() < end:
                try: s = opened(); break
                except (serial.SerialException, OSError): time.sleep(0.05)
print(log)
PY
cargo run --quiet --offline --manifest-path "$here/Cargo.toml" --bin c5l8-check < "$log"
