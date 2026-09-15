"""Mock multi-device UDP packet injector for the Phase 8 tracking gateway.

Simulates N devices (MACs) walking loops inside the demo 20x15 m map, each seen
by 4 ESP32 nodes. Emits log-distance RSSI (with per-device noise) as UDP JSON to
the gateway on 0.0.0.0:5555, so 06-cyber-hud.html shows multiple independent
tracked capsules with no hardware.

Run alongside:  python serve-demo.py --gateway
Then:           python mock_udp_injector.py            # 2 devices
                python mock_udp_injector.py --devices 4 --rate 15
"""
import argparse
import json
import math
import random
import socket
import time

NODES = [  # must match 06-cyber-hud.html DEFAULT_MAP node ids/positions
    ("ESP32_01", 1.2, 0.5, 1.0),
    ("ESP32_02", 18.5, 2.1, 1.2),
    ("ESP32_03", 9.0, 14.2, 2.5),
    ("ESP32_04", 0.8, 13.5, 1.8),
]
P0_DBM, PLE = -40.0, 2.2   # log-distance model the HUD should be calibrated near


def range_to_rssi(r_m, jitter):
    r = max(r_m, 0.3)
    return P0_DBM - 10.0 * PLE * math.log10(r) + random.gauss(0.0, jitter)


def device_path(seed):
    rng = random.Random(seed)
    # a lissajous-ish loop kept inside the 20x15 map
    ax, ay = rng.uniform(5, 8), rng.uniform(3, 5.5)
    cx, cy = rng.uniform(8, 12), rng.uniform(6, 9)
    fx, fy = rng.uniform(0.10, 0.20), rng.uniform(0.13, 0.24)
    ph = rng.uniform(0, 6.28)
    return lambda t: (cx + ax * math.sin(fx * t + ph),
                      cy + ay * math.sin(fy * t))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--host", default="127.0.0.1")
    ap.add_argument("--port", type=int, default=5555)
    ap.add_argument("--devices", type=int, default=2)
    ap.add_argument("--rate", type=float, default=10.0, help="frames/sec per device")
    ap.add_argument("--jitter", type=float, default=1.5, help="RSSI noise sigma (dB)")
    args = ap.parse_args()

    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    macs = [f"AA:BB:CC:00:00:{i:02X}" for i in range(1, args.devices + 1)]
    paths = [device_path(i) for i in range(args.devices)]
    print(f"injecting {args.devices} device(s) → udp {args.host}:{args.port} "
          f"@ {args.rate} Hz/device ({', '.join(macs)})  Ctrl-C to stop")

    t0 = time.time()
    period = 1.0 / max(args.rate, 0.5)
    try:
        while True:
            t = time.time() - t0
            for mac, path in zip(macs, paths):
                x, y = path(t)
                for nid, nx, ny, nz in NODES:
                    r = math.dist((x, y, 1.0), (nx, ny, nz))
                    pkt = {"node_id": nid, "mac": mac,
                           "rssi": round(range_to_rssi(r, args.jitter), 1)}
                    sock.sendto(json.dumps(pkt).encode("utf-8"), (args.host, args.port))
            time.sleep(period)
    except KeyboardInterrupt:
        print("\nstopped")


if __name__ == "__main__":
    main()
