import { Platform } from 'react-native';

import { rssiService as androidRssiService } from './rssi.service.android';
import { rssiService as iosRssiService } from './rssi.service.ios';

export interface WifiNetwork {
  ssid: string;
  bssid?: string;
  level: number;
}

export interface RssiService {
  startScanning(intervalMs: number): void;
  stopScanning(): void;
  subscribe(listener: (networks: WifiNetwork[]) => void): () => void;
}

export const rssiService: RssiService =
  Platform.OS === 'android' ? androidRssiService : iosRssiService;
