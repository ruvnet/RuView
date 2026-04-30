#!/usr/bin/env python3
"""
UDP relay that rewrites byte 4 (node_id) of CSI/vitals packets based on source IP.

Workaround for firmware bug (issues #374/#386): the WiFi stack clobbers
g_nvs_config.node_id between main and csi_collector_init, so all CSI packets ship
with node_id=1 regardless of NVS. We patch byte 4 here using a source-IP map.

Listens on 0.0.0.0:5005 (where ESP32s send) and forwards rewritten packets to
127.0.0.1:<dest_port> (where sensing-server listens with --udp-port).

Usage: csi_node_id_relay.py [--listen-port 5005] [--dest-port 5099]
"""
import argparse
import socket
import sys

# Source IP -> node_id mapping. Update as new ESPs join.
IP_TO_NODE_ID = {
    "192.168.13.222": 1,   # ESP32-S3 #1, MAC 10:b4:1d:ea:eb:a0
    "192.168.3.246":  2,   # ESP32-S3 #2, MAC b8:f8:62:f9:d5:58
}

# Magics whose byte 4 is node_id (LE). 0xC5110001 CSI, 0xC5110002 vitals,
# 0xC5110004 wasm-output (ADR-040). All three put node_id at offset 4.
NODE_ID_MAGICS = {
    bytes.fromhex("010011c5"),
    bytes.fromhex("020011c5"),
    bytes.fromhex("040011c5"),
}

def rewrite_packet(data: bytes, src_ip: str,
                   ip_map: dict | None = None,
                   magics: set | None = None) -> bytes:
    """Return packet with byte[4] (node_id) corrected for src_ip, or data unchanged."""
    if ip_map is None:
        ip_map = IP_TO_NODE_ID
    if magics is None:
        magics = NODE_ID_MAGICS
    nid = ip_map.get(src_ip)
    if not (isinstance(nid, int) and 0 <= nid <= 255):
        return data
    if len(data) >= 5 and data[:4] in magics and data[4] != nid:
        return bytes([data[0], data[1], data[2], data[3], nid]) + data[5:]
    return data


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--listen-port", type=int, default=5005)
    ap.add_argument("--dest-port",   type=int, default=5099)
    ap.add_argument("--dest-host",   default="127.0.0.1")
    args = ap.parse_args()

    rx = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    rx.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    rx.bind(("0.0.0.0", args.listen_port))
    tx = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    dest = (args.dest_host, args.dest_port)

    print(f"relay: 0.0.0.0:{args.listen_port} -> {args.dest_host}:{args.dest_port}", flush=True)
    print(f"map:   {IP_TO_NODE_ID}", flush=True)

    counts = {}
    rewrites = 0
    total_forwarded = 0
    while True:
        try:
            data, addr = rx.recvfrom(4096)
        except KeyboardInterrupt:
            break
        src_ip = addr[0]
        rewritten = rewrite_packet(data, src_ip)
        if rewritten is not data:
            data = rewritten
            rewrites += 1
        tx.sendto(data, dest)
        counts[src_ip] = counts.get(src_ip, 0) + 1
        total_forwarded += 1
        if total_forwarded % 200 == 0:
            print(f"forwarded: {counts}  rewrites={rewrites}", flush=True)

if __name__ == "__main__":
    main()
