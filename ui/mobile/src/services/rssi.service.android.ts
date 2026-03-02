import type { RssiService, WifiNetwork } from './rssi.service';
import WifiManager from '@react-native-wifi-reborn';

type NativeWifiNetwork = {
  SSID?: string;
  BSSID?: string;
  level?: number;
  levelDbm?: number;
};

class AndroidRssiService implements RssiService {
  private timer: ReturnType<typeof setInterval> | null = null;
  private listeners = new Set<(networks: WifiNetwork[]) => void>();

  startScanning(intervalMs: number): void {
    this.stopScanning();
    this.scanOnce();
    this.timer = setInterval(() => {
      this.scanOnce();
    }, intervalMs);
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

  private async scanOnce(): Promise<void> {
    try {
      const results = (await WifiManager.loadWifiList()) as NativeWifiNetwork[];
      const mapped = results.map((item) => ({
        ssid: item.SSID || '',
        bssid: item.BSSID,
        level: typeof item.level === 'number' ? item.level : typeof item.levelDbm === 'number' ? item.levelDbm : -100,
      }));
      this.broadcast(mapped.filter((n) => n.ssid.length > 0));
    } catch {
      this.broadcast([]);
    }
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

export const rssiService = new AndroidRssiService();
