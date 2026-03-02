declare module '@react-native-wifi-reborn' {
  interface NativeWifiNetwork {
    SSID?: string;
    BSSID?: string;
    level?: number;
    levelDbm?: number;
  }

  const WifiManager: {
    loadWifiList: () => Promise<NativeWifiNetwork[]>;
  };

  export default WifiManager;
}
