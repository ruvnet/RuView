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

// Metro resolves the correct platform file automatically:
//   rssi.service.android.ts  (Android)
//   rssi.service.ios.ts      (iOS)
//   rssi.service.web.ts      (Web)
// This file only exports the shared types.
// The platform entry is re-exported from the index barrel below.

import { Platform } from 'react-native';

// Lazy require to avoid bundling native modules on web
function getPlatformService(): RssiService {
  if (Platform.OS === 'android') {
    return require('./rssi.service.android').rssiService;
  } else if (Platform.OS === 'ios') {
    return require('./rssi.service.ios').rssiService;
  } else {
    return require('./rssi.service.web').rssiService;
  }
}

export const rssiService: RssiService = getPlatformService();
