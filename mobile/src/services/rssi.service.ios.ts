import type { RssiService, WifiNetwork } from './rssi.service';

class IosRssiService implements RssiService {
  private timer: ReturnType<typeof setInterval> | null = null;
  private listeners = new Set<(networks: WifiNetwork[]) => void>();

  startScanning(intervalMs: number): void {
    console.warn('iOS RSSI scanning not available; returning synthetic network data.');
    this.stopScanning();
    this.timer = setInterval(() => {
      this.broadcast([{ ssid: 'WiFi-DensePose', bssid: undefined, level: -60 }]);
    }, intervalMs);
    this.broadcast([{ ssid: 'WiFi-DensePose', bssid: undefined, level: -60 }]);
  }

  stopScanning(): void {
    if (this.timer) {
      clearInterval(this.timer);
      this.timer = null;
    }
  }

  subscribe(listener: (networks: WifiNetwork[]) => void): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  private broadcast(networks: WifiNetwork[]): void {
    this.listeners.forEach((listener) => {
      try {
        listener(networks);
      } catch {
        // listener safety
      }
    });
  }
}

export const rssiService = new IosRssiService();
