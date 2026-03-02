import { useEffect, useState } from 'react';
import { rssiService, type WifiNetwork } from '@/services/rssi.service';
import { useSettingsStore } from '@/stores/settingsStore';

export function useRssiScanner(): { networks: WifiNetwork[]; isScanning: boolean } {
  const enabled = useSettingsStore((state) => state.rssiScanEnabled);
  const [networks, setNetworks] = useState<WifiNetwork[]>([]);
  const [isScanning, setIsScanning] = useState(false);

  useEffect(() => {
    if (!enabled) {
      rssiService.stopScanning();
      setIsScanning(false);
      return;
    }

    const unsubscribe = rssiService.subscribe((result) => {
      setNetworks(result);
    });
    rssiService.startScanning(2000);
    setIsScanning(true);

    return () => {
      unsubscribe();
      rssiService.stopScanning();
      setIsScanning(false);
    };
  }, [enabled]);

  return { networks, isScanning };
}
