#!/usr/bin/env python3
"""
ADR-122 prep probe — switch HLK-LD2402 into Engineering Mode and
dump the first few binary frames so we can verify per-gate layout
before committing to a Rust parser.

Protocol (Hi-Link, reverse-engineered from ESPHome driver):

  Command frame (host -> module):
    FD FC FB FA   (header)
    len_lo len_hi (LE u16; counts cmd+data bytes)
    cmd_lo cmd_hi (LE u16)
    [data...]
    04 03 02 01   (footer)

  Sequence to enable engineering mode:
    1. CMD 0x00FF (enable config),     data = 01 00
    2. CMD 0x0012 (set work mode),     data = 00 00 04 00 00 00   (mode=0x04)
    3. CMD 0x00FE (disable config),    data = ()

  Data frame (module -> host) in Engineering Mode:
    F4 F3 F2 F1   (data header)
    LL LL         (length, LE u16, payload bytes)
    84            (frame type = engineering)
    ...           (per-gate energy, u32 LE per gate)
    F8 F7 F6 F5   (data footer)
"""
import argparse, struct, sys, time

import serial  # pyserial

HEAD = b"\xFD\xFC\xFB\xFA"
FOOT = b"\x04\x03\x02\x01"
DATA_HEAD = b"\xF4\xF3\xF2\xF1"
DATA_FOOT = b"\xF8\xF7\xF6\xF5"

def frame(cmd: int, data: bytes = b"") -> bytes:
    body = struct.pack("<H", cmd) + data
    return HEAD + struct.pack("<H", len(body)) + body + FOOT

def hex_dump(b: bytes, n: int = 64) -> str:
    return " ".join(f"{x:02X}" for x in b[:n]) + (" ..." if len(b) > n else "")

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", default="/dev/cu.usbserial-1140")
    ap.add_argument("--baud", type=int, default=115200)
    ap.add_argument("--frames", type=int, default=20, help="how many engineering frames to capture")
    ap.add_argument("--no-restore", action="store_true", help="leave module in engineering mode")
    args = ap.parse_args()

    print(f"[probe] open {args.port} @ {args.baud}", flush=True)
    ser = serial.Serial(args.port, args.baud, timeout=0.5)
    time.sleep(0.2)
    ser.reset_input_buffer()

    # 1. enable config mode
    pkt = frame(0x00FF, b"\x01\x00")
    print(f"[probe] -> ENABLE_CONFIG: {hex_dump(pkt)}", flush=True)
    ser.write(pkt); time.sleep(0.3)
    print(f"[probe] <- {hex_dump(ser.read(64))}", flush=True)

    # 2. set work mode = engineering (0x04)
    mode_data = b"\x00\x00" + struct.pack("<I", 0x04)
    pkt = frame(0x0012, mode_data)
    print(f"[probe] -> SET_MODE engineering: {hex_dump(pkt)}", flush=True)
    ser.write(pkt); time.sleep(0.5)
    print(f"[probe] <- {hex_dump(ser.read(64))}", flush=True)

    # 3. disable config mode (data starts flowing)
    pkt = frame(0x00FE)
    print(f"[probe] -> DISABLE_CONFIG: {hex_dump(pkt)}", flush=True)
    ser.write(pkt); time.sleep(0.3)
    print(f"[probe] <- {hex_dump(ser.read(64))}", flush=True)

    # 4. capture engineering frames
    print(f"\n[probe] capturing {args.frames} engineering frames...", flush=True)
    buf = bytearray()
    frames_seen = 0
    t0 = time.time()
    while frames_seen < args.frames and (time.time() - t0) < 15:
        chunk = ser.read(512)
        if chunk:
            buf.extend(chunk)
            while True:
                i = buf.find(DATA_HEAD)
                if i < 0:
                    if len(buf) > 4096:
                        buf = buf[-4:]
                    break
                # need at least header(4) + len(2) + type(1) = 7 bytes after match
                if len(buf) < i + 7:
                    break
                length = buf[i+4] | (buf[i+5] << 8)
                # full frame = head(4) + len(2) + payload(length) + foot(4)
                end = i + 4 + 2 + length + 4
                if len(buf) < end:
                    break
                frm = bytes(buf[i:end])
                buf = buf[end:]
                ftype = frm[6]
                print(f"\n[frame #{frames_seen}] len={length} type=0x{ftype:02X} total={len(frm)} footer={frm[-4:].hex(' ').upper()}", flush=True)
                print(f"  hex: {hex_dump(frm, 160)}", flush=True)
                if ftype == 0x84 and length >= 4:
                    # payload (after type byte) starts at frm[7:7+length-1]
                    # ESPHome read showed gates start at offset 10 within frame_data
                    # which (their frame_data = bytes from data header) means
                    # offset 10 = after 4(head)+2(len)+1(type)+3 prelude bytes
                    payload = frm[7:7+length-1]
                    prelude = payload[:3]
                    gate_bytes = payload[3:]
                    n_gates = len(gate_bytes) // 4
                    energies = struct.unpack(f"<{n_gates}I", gate_bytes[:n_gates*4])
                    print(f"  prelude({len(prelude)}): {hex_dump(prelude)}", flush=True)
                    print(f"  gates({n_gates}): {energies}", flush=True)
                frames_seen += 1
                if frames_seen >= args.frames:
                    break

    print(f"\n[probe] captured {frames_seen} frame(s) in {time.time()-t0:.1f}s", flush=True)

    if not args.no_restore:
        # restore normal mode so we don't break the existing server next start
        pkt = frame(0x00FF, b"\x01\x00"); ser.write(pkt); time.sleep(0.3); ser.read(64)
        mode_data = b"\x00\x00" + struct.pack("<I", 0x64)  # production
        pkt = frame(0x0012, mode_data); ser.write(pkt); time.sleep(0.3); ser.read(64)
        pkt = frame(0x00FE); ser.write(pkt); time.sleep(0.3); ser.read(64)
        print("[probe] restored normal mode", flush=True)

    ser.close()

if __name__ == "__main__":
    main()
