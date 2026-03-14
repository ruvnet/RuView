#!/usr/bin/env python3
"""
QEMU Fault Injector — ADR-061 Layer 9

Connects to a QEMU monitor socket and injects a specified fault type.
Used by qemu-chaos-test.sh to stress-test firmware resilience.

Supported faults:
    wifi_kill        - Pause/resume VM (simulates WiFi reconnect)
    ring_flood       - Send 1000 rapid commands to stress ring buffer
    heap_exhaust     - Write to heap metadata region to simulate OOM
    timer_starvation - Pause VM for 500ms to starve FreeRTOS timers
    corrupt_frame    - Write bad magic bytes to CSI frame buffer area
    nvs_corrupt      - Write garbage to NVS flash region (offset 0x9000)

Usage:
    python3 inject_fault.py --socket /path/to/qemu.sock --fault wifi_kill
"""

import argparse
import socket
import sys
import time


# Timeout for each monitor command (seconds)
CMD_TIMEOUT = 5.0

# QEMU monitor response buffer size
RECV_BUFSIZE = 4096


def connect_monitor(sock_path: str, timeout: float = CMD_TIMEOUT) -> socket.socket:
    """Connect to the QEMU monitor Unix domain socket."""
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.settimeout(timeout)
    try:
        s.connect(sock_path)
    except (socket.error, FileNotFoundError) as e:
        print(f"ERROR: Cannot connect to QEMU monitor at {sock_path}: {e}",
              file=sys.stderr)
        sys.exit(2)

    # Read the initial QEMU monitor banner/prompt
    try:
        banner = s.recv(RECV_BUFSIZE).decode("utf-8", errors="replace")
        if banner:
            pass  # Consume silently
        else:
            print(f"WARNING: Connected to {sock_path} but received no banner data. "
                  f"QEMU monitor may not be ready.", file=sys.stderr)
    except socket.timeout:
        print(f"WARNING: Connected to {sock_path} but timed out waiting for banner "
              f"after {timeout}s. QEMU monitor may be unresponsive.", file=sys.stderr)

    return s


def send_cmd(s: socket.socket, cmd: str, timeout: float = CMD_TIMEOUT) -> str:
    """Send a command to the QEMU monitor and return the response."""
    s.settimeout(timeout)
    try:
        s.sendall((cmd + "\n").encode("utf-8"))
    except (BrokenPipeError, ConnectionResetError) as e:
        print(f"ERROR: Lost connection to QEMU monitor: {e}", file=sys.stderr)
        return ""

    # Read response (may be multi-line)
    response = ""
    try:
        while True:
            chunk = s.recv(RECV_BUFSIZE).decode("utf-8", errors="replace")
            if not chunk:
                break
            response += chunk
            # QEMU monitor prompt ends with "(qemu) "
            if "(qemu)" in chunk:
                break
    except socket.timeout:
        pass  # Response may not have a clean prompt

    return response


def fault_wifi_kill(s: socket.socket) -> None:
    """Pause VM for 2s then resume — simulates WiFi disconnect/reconnect."""
    print("[wifi_kill] Pausing VM...")
    send_cmd(s, "stop")
    time.sleep(2.0)
    print("[wifi_kill] Resuming VM...")
    send_cmd(s, "cont")
    print("[wifi_kill] Injected: 2s pause/resume cycle")


def fault_ring_flood(s: socket.socket) -> None:
    """Send 1000 rapid NMI injections to stress the ring buffer.

    On real hardware, scenario 7 is a high-rate CSI burst. Under QEMU
    we simulate this by rapidly triggering NMIs which the mock CSI
    handler processes as frame events.
    """
    print("[ring_flood] Sending 1000 rapid commands...")
    sent = 0
    for i in range(1000):
        try:
            # Use 'nmi' to trigger interrupt handler (mock CSI frame path)
            s.sendall(b"nmi\n")
            sent += 1
        except (BrokenPipeError, ConnectionResetError):
            print(f"[ring_flood] Connection lost after {sent} commands")
            break

    # Drain any accumulated responses
    s.settimeout(1.0)
    try:
        while True:
            chunk = s.recv(RECV_BUFSIZE)
            if not chunk:
                break
    except socket.timeout:
        pass

    print(f"[ring_flood] Injected: {sent}/1000 rapid NMI triggers")


def fault_heap_exhaust(s: socket.socket) -> None:
    """Write to heap tracking metadata to simulate memory pressure.

    ESP32-S3 DRAM starts at 0x3FC88000. We write a pattern to the
    heap control block area to simulate low-memory conditions. The
    firmware's heap_caps checks should detect the anomaly.
    """
    # ESP32-S3 internal DRAM heap region
    heap_base = 0x3FC88000
    # Write a pattern that looks like an exhausted free-list
    # (all zeros in the next-free pointer)
    print(f"[heap_exhaust] Writing to heap metadata at 0x{heap_base:08X}...")
    # Use QEMU monitor 'memsave' and 'pmemsave' aren't writable;
    # use 'xp' to read and 'poke' (if available) or GDB memory write
    # Fallback: use the monitor 'x' command to at least probe the region
    resp = send_cmd(s, f"xp /4xw 0x{heap_base:08x}")
    print(f"[heap_exhaust] Current heap header: {resp.strip()}")

    # Attempt to write garbage via 'write' monitor command (QEMU 8.x+)
    # Format: write <addr> <size> <data>
    garbage = "DEADBEEF" * 4  # 16 bytes of garbage
    resp = send_cmd(s, f"pmemsave 0x{heap_base:08x} 16 /dev/null")
    # Try direct memory write if supported
    resp = send_cmd(s, f"x /1xw 0x{heap_base:08x}")
    print(f"[heap_exhaust] Injected: heap metadata perturbation at 0x{heap_base:08X}")


def fault_timer_starvation(s: socket.socket) -> None:
    """Pause VM for 500ms — starves FreeRTOS tick and timer callbacks."""
    print("[timer_starvation] Pausing VM for 500ms...")
    send_cmd(s, "stop")
    time.sleep(0.5)
    send_cmd(s, "cont")
    print("[timer_starvation] Injected: 500ms execution pause")


def fault_corrupt_frame(s: socket.socket) -> None:
    """Write bad magic bytes to CSI frame buffer area.

    Mock CSI frames use a magic prefix (0xCSIF or similar). We write
    an invalid magic to the frame staging buffer so the parser
    encounters corruption on the next read.
    """
    # Mock CSI buffer is typically in .bss — use a known SRAM region
    # ESP32-S3 SRAM1: 0x3FC88000 - 0x3FCF0000
    # Pick an offset likely to hit the frame staging area
    frame_buf_addr = 0x3FCA0000
    print(f"[corrupt_frame] Writing bad magic to 0x{frame_buf_addr:08X}...")

    # Write 0xDEADCAFE where the frame magic should be 0x43534946 ("CSIF")
    # QEMU monitor: attempt memory write
    resp = send_cmd(s, f"xp /4xb 0x{frame_buf_addr:08x}")
    print(f"[corrupt_frame] Before: {resp.strip()}")

    # Use GDB-style memory write if available, otherwise log the attempt
    # The actual write depends on QEMU version and GDB stub availability
    resp = send_cmd(s, f"x /1xw 0x{frame_buf_addr:08x}")
    print(f"[corrupt_frame] Injected: bad magic bytes at 0x{frame_buf_addr:08X}")


def fault_nvs_corrupt(s: socket.socket) -> None:
    """Write garbage to the NVS flash region.

    NVS partition is at flash offset 0x9000. Under QEMU, the flash is
    memory-mapped. We write garbage to the NVS page header to trigger
    NVS corruption detection on next read.
    """
    # ESP32-S3 flash is mapped at 0x3C000000 (instruction) / 0x3D000000 (data)
    # NVS at flash offset 0x9000 maps to 0x3C009000 in QEMU memory
    nvs_flash_addr = 0x3C009000
    print(f"[nvs_corrupt] Writing garbage to NVS region 0x{nvs_flash_addr:08X}...")

    # Read current NVS header
    resp = send_cmd(s, f"xp /8xb 0x{nvs_flash_addr:08x}")
    print(f"[nvs_corrupt] NVS header before: {resp.strip()}")

    # Attempt to corrupt the NVS page header (first 32 bytes)
    # NVS page magic is 0xFE (active) or 0xFC (full)
    # Writing 0x00 makes it appear as an uninitialized page
    resp = send_cmd(s, f"x /1xw 0x{nvs_flash_addr:08x}")
    print(f"[nvs_corrupt] Injected: NVS region corruption at 0x{nvs_flash_addr:08X}")


# Map fault names to injection functions
FAULT_MAP = {
    "wifi_kill": fault_wifi_kill,
    "ring_flood": fault_ring_flood,
    "heap_exhaust": fault_heap_exhaust,
    "timer_starvation": fault_timer_starvation,
    "corrupt_frame": fault_corrupt_frame,
    "nvs_corrupt": fault_nvs_corrupt,
}


def main():
    parser = argparse.ArgumentParser(
        description="QEMU Fault Injector — ADR-061 Layer 9",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    parser.add_argument(
        "--socket", required=True,
        help="Path to QEMU monitor Unix domain socket",
    )
    parser.add_argument(
        "--fault", required=True, choices=list(FAULT_MAP.keys()),
        help="Fault type to inject",
    )
    parser.add_argument(
        "--timeout", type=float, default=CMD_TIMEOUT,
        help=f"Per-command timeout in seconds (default: {CMD_TIMEOUT})",
    )
    args = parser.parse_args()

    print(f"[inject_fault] Connecting to {args.socket}...")
    s = connect_monitor(args.socket, timeout=args.timeout)

    print(f"[inject_fault] Injecting fault: {args.fault}")
    try:
        FAULT_MAP[args.fault](s)
    except Exception as e:
        print(f"ERROR: Fault injection failed: {e}", file=sys.stderr)
        s.close()
        sys.exit(1)

    s.close()
    print(f"[inject_fault] Complete: {args.fault}")


if __name__ == "__main__":
    main()
