# =====================================================================
# run_win_wifi.py - Run the RuView sensing server using the laptop's
# built-in WiFi NIC (Windows netsh RSSI). No ESP32 hardware needed.
#
#   * Forces WindowsWifiCollector on the configured interface
#   * Broadcasts sensing frames on ws://localhost:8765
#   * UI: serve E:\RuView\ui on another port, then in observatory.html
#     set the WebSocket URL to ws://localhost:8765
#
# Interface can be overridden:  set RUVIEW_WIFI_IFACE=YourIfName
# =====================================================================
import asyncio
import logging
import os
import signal
import sys

# The `v1` package root is E:\RuView\archive
V1_ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'archive')
sys.path.insert(0, V1_ROOT)

from v1.src.sensing.rssi_collector import WindowsWifiCollector  # noqa: E402
from v1.src.sensing.ws_server import SensingWebSocketServer      # noqa: E402

IFACE = os.environ.get('RUVIEW_WIFI_IFACE', 'WLAN 2')


class NativeWifiServer(SensingWebSocketServer):
    """Force the built-in Windows NIC collector (skip ESP32 probe / simulated)."""

    def _create_collector(self):
        collector = WindowsWifiCollector(interface=IFACE, sample_rate_hz=2.0)
        collector.collect_once()          # sanity check: netsh reachable + connected
        self.source = 'windows_wifi'
        return collector


def main():
    logging.basicConfig(
        level=logging.INFO,
        format='%(asctime)s [%(levelname)s] %(name)s: %(message)s',
    )
    server = NativeWifiServer()

    loop = asyncio.new_event_loop()
    asyncio.set_event_loop(loop)

    def _shutdown(sig, frame):
        print('\nShutting down...')
        server.stop()
        loop.stop()

    signal.signal(signal.SIGINT, _shutdown)
    signal.signal(signal.SIGTERM, _shutdown)

    try:
        loop.run_until_complete(server.run())
    except KeyboardInterrupt:
        pass
    finally:
        server.stop()
        loop.close()


if __name__ == '__main__':
    main()
