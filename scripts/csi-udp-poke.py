#!/usr/bin/env python3
"""Request CSI opportunities with bounded LAN unicast UDP (#2287).

Run on a wired LAN host when the AP sends ICMP at 802.11b rates. No response
service or DNS request is needed. AP rate selection still determines CSI yield.
"""
import argparse
import ipaddress
import socket
import time


def lan_address(value):
    address = ipaddress.IPv4Address(value)
    if not any(address in network for network in (
        ipaddress.IPv4Network('10.0.0.0/8'),
        ipaddress.IPv4Network('172.16.0.0/12'),
        ipaddress.IPv4Network('192.168.0.0/16'),
    )) or int(address) & 255 in (0, 255):
        raise argparse.ArgumentTypeError('use a private LAN unicast IPv4 address')
    return str(address)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('node', type=lan_address)
    parser.add_argument('--port', type=int, default=5501)
    parser.add_argument('--hz', type=int, choices=range(10, 51), default=50)
    parser.add_argument('--seconds', type=int, default=60)
    args = parser.parse_args()
    if not 1024 <= args.port <= 65535:
        parser.error('--port must be 1024..65535; use an unused node UDP port')
    if not 1 <= args.seconds <= 3600:
        parser.error('--seconds must be 1..3600')
    count = 0
    interval = 1 / args.hz
    end = time.monotonic() + args.seconds
    with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as sender:
        while time.monotonic() < end:
            started = time.monotonic()
            sender.sendto(b'RUVIEW_CSI_POKE_V1', (args.node, args.port))
            count += 1
            time.sleep(max(0, interval - (time.monotonic() - started)))
    print(f'Sent {count} UDP datagrams. Measure received CSI separately; '
          'sent traffic does not establish CSI yield or sensing accuracy.')


if __name__ == '__main__':
    main()
