#!/usr/bin/env python3
"""Synthetic CSI UDP emitter for testing the calibration CLI end-to-end.

Emits the same 0xC511_0001 frame format the ESP32-S3 firmware produces, so the
`wifi-densepose calibrate` CLI can be exercised without a live ESP32 in the
loop. Generates HT20 frames (52 active subcarriers, 1 antenna) at 20 Hz.
"""
import argparse
import math
import random
import socket
import struct
import time


MAGIC = 0xC511_0001


def build_packet(node_id: int, seq: int, freq_mhz: int, rssi: int,
                 amps: list[float], phases: list[float]) -> bytes:
    n_ant = 1
    n_sc = len(amps)
    header = struct.pack(
        "<I B B H I I b b B B",
        MAGIC,
        node_id,
        n_ant,
        n_sc,        # u16 @6
        freq_mhz,    # u32 @8
        seq,         # u32 @12
        rssi,        # i8  @16
        -95,         # i8  @17 noise_floor
        0,           # u8  @18 ppdu_type
        0,           # u8  @19 flags
    )
    iq = bytearray()
    for amp, phase in zip(amps, phases):
        i = max(-127, min(127, int(amp * math.cos(phase))))
        q = max(-127, min(127, int(amp * math.sin(phase))))
        iq.extend(struct.pack("bb", i, q))
    return bytes(header) + bytes(iq)


def main() -> None:
    p = argparse.ArgumentParser()
    p.add_argument("--host", default="127.0.0.1")
    p.add_argument("--port", type=int, default=5005)
    p.add_argument("--duration-s", type=float, default=35.0,
                   help="emit duration; default 35s so a 30s capture sees the full stream")
    p.add_argument("--rate-hz", type=float, default=20.0)
    p.add_argument("--n-sc", type=int, default=52)
    p.add_argument("--motion-after-s", type=float, default=-1.0,
                   help="if >=0, inject amplitude jitter after this many seconds")
    args = p.parse_args()

    random.seed(42)
    base_amps = [40.0 + 10.0 * math.cos(k * 0.2) for k in range(args.n_sc)]
    base_phases = [0.5 * math.sin(k * 0.3) for k in range(args.n_sc)]

    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    period = 1.0 / args.rate_hz
    started = time.time()
    seq = 0
    print(f"emitting CSI to {args.host}:{args.port} at {args.rate_hz} Hz, "
          f"{args.n_sc} sc/frame, duration {args.duration_s}s", flush=True)

    while True:
        elapsed = time.time() - started
        if elapsed >= args.duration_s:
            break
        amps = list(base_amps)
        phases = list(base_phases)
        # Mild stationary jitter (~0.5 amplitude units RMS)
        for k in range(args.n_sc):
            amps[k] += random.gauss(0.0, 0.5)
            phases[k] += random.gauss(0.0, 0.01)
        if args.motion_after_s >= 0 and elapsed >= args.motion_after_s:
            for k in range(args.n_sc):
                amps[k] += random.gauss(0.0, 8.0)
                phases[k] += random.gauss(0.0, 0.3)
        pkt = build_packet(node_id=42, seq=seq, freq_mhz=2412, rssi=-55,
                           amps=amps, phases=phases)
        sock.sendto(pkt, (args.host, args.port))
        seq += 1
        time.sleep(period)

    print(f"emitted {seq} frames", flush=True)


if __name__ == "__main__":
    main()
