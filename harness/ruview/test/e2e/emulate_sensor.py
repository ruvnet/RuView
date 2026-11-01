# SPDX-License-Identifier: MIT
# Emulates a serial sensor on a pseudo-terminal for the device e2e suite
# (ADR-373). Evidence produced through it is SYNTHETIC, never hardware.
#   mr60     Seeed MR60BHA2 frames (breathing, heart, presence, distance)
#   rplidar  answers SCAN (A5 20) with the descriptor + 5-byte nodes; STOP (A5 25)
# Usage: emulate_sensor.py <mr60|rplidar> <seconds>  -> prints "READY <pts>".
import os, pty, struct, sys, time, tty
kind, secs = sys.argv[1], float(sys.argv[2])
master, slave = pty.openpty()
tty.setraw(slave)
name = os.ttyname(slave)
print('READY', name, flush=True)
def ck(b):
    x = 0
    for v in b: x ^= v
    return (~x) & 0xff
def mr60(t, p):
    h = bytes([1, 0, 1, len(p) >> 8, len(p) & 255, t >> 8, t & 255])
    return h + bytes([ck(h)]) + p + bytes([ck(p)])
end = time.time() + secs
if kind == 'mr60':
    while time.time() < end:
        os.write(master, mr60(0x0A14, struct.pack('<f', 14.8)) + mr60(0x0A15, struct.pack('<f', 68.5))
                 + mr60(0x0F09, b'\x01') + mr60(0x0A16, struct.pack('<If', 1, 87.0)))
        time.sleep(0.1)
else:
    import select
    started = False
    a = 0
    while time.time() < end:
        r, _, _ = select.select([master], [], [], 0.01)
        if r:
            cmd = os.read(master, 64)
            if b'\xa5\x20' in cmd and not started:
                started = True
                os.write(master, bytes([0xa5, 0x5a, 0x05, 0, 0, 0x40, 0x81]))
            if b'\xa5\x25' in cmd: started = False
        if started:
            out = b''
            for _ in range(36):
                q = int(a * 64); d = int((1200 + 300 * ((a // 90) % 2)) * 4)
                s = 1 if a == 0 else 2
                out += bytes([(47 << 2) | s, ((q & 0x7f) << 1) | 1, q >> 7, d & 255, d >> 8])
                a = (a + 10) % 360
            os.write(master, out)
            time.sleep(0.02)
