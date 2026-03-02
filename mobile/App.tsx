import { useEffect } from 'react';
import { NavigationContainer, DarkTheme } from '@react-navigation/native';
import { GestureHandlerRootView } from 'react-native-gesture-handler';
import { StatusBar } from 'expo-status-bar';
import { SafeAreaProvider } from 'react-native-safe-area-context';
import { apiService } from '@/services/api.service';
import { rssiService } from '@/services/rssi.service';
import { wsService } from '@/services/ws.service';
import { ThemeProvider } from './src/theme/ThemeContext';
import { usePoseStore } from './src/stores/poseStore';
import { useSettingsStore } from './src/stores/settingsStore';
import { RootNavigator } from './src/navigation/RootNavigator';

export default function App() {
  const serverUrl = useSettingsStore((state) => state.serverUrl);
  const rssiScanEnabled = useSettingsStore((state) => state.rssiScanEnabled);

  useEffect(() => {
    apiService.setBaseUrl(serverUrl);
    const unsubscribe = wsService.subscribe(usePoseStore.getState().handleFrame);
    wsService.connect(serverUrl);

    return () => {
      unsubscribe();
      wsService.disconnect();
    };
  }, [serverUrl]);

  useEffect(() => {
    if (!rssiScanEnabled) {
      rssiService.stopScanning();
      return;
    }

    const unsubscribe = rssiService.subscribe(() => {
      // Consumers can subscribe elsewhere for RSSI events.
    });
    rssiService.startScanning(2000);

    return () => {
      unsubscribe();
      rssiService.stopScanning();
    };
  }, [rssiScanEnabled]);

  useEffect(() => {
    (globalThis as { __appStartTime?: number }).__appStartTime = Date.now();
  }, []);

  const navigationTheme = {
    ...DarkTheme,
    colors: {
      ...DarkTheme.colors,
      background: '#0A0E1A',
      card: '#0D1117',
      text: '#E2E8F0',
      border: '#1E293B',
      primary: '#32B8C6',
    },
  };

  return (
    <GestureHandlerRootView style={{ flex: 1 }}>
      <SafeAreaProvider>
        <ThemeProvider>
          <NavigationContainer theme={navigationTheme}>
            <RootNavigator />
          </NavigationContainer>
        </ThemeProvider>
      </SafeAreaProvider>
      <StatusBar style="light" />
    </GestureHandlerRootView>
  );
}
