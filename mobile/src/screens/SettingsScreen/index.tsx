import { useEffect, useMemo, useState } from 'react';
import { Linking, ScrollView, View } from 'react-native';
import { ThemedText } from '@/components/ThemedText';
import { ThemedView } from '@/components/ThemedView';
import { colors } from '@/theme/colors';
import { spacing } from '@/theme/spacing';
import { WS_PATH } from '@/constants/websocket';
import { apiService } from '@/services/api.service';
import { wsService } from '@/services/ws.service';
import { useSettingsStore } from '@/stores/settingsStore';
import { Alert, Pressable, Platform } from 'react-native';
import { ThemePicker } from './ThemePicker';
import { RssiToggle } from './RssiToggle';
import { ServerUrlInput } from './ServerUrlInput';

type GlowCardProps = {
  title: string;
  children: React.ReactNode;
};

const GlowCard = ({ title, children }: GlowCardProps) => {
  return (
    <View
      style={{
        backgroundColor: '#0F141E',
        borderRadius: 14,
        borderWidth: 1,
        borderColor: `${colors.accent}35`,
        padding: spacing.md,
        marginBottom: spacing.md,
      }}
    >
      <ThemedText preset="labelMd" style={{ marginBottom: spacing.sm, color: colors.textPrimary }}>
        {title}
      </ThemedText>
      {children}
    </View>
  );
};

const ScanIntervalPicker = ({
  value,
  onChange,
}: {
  value: number;
  onChange: (value: number) => void;
}) => {
  const options = [1, 2, 5];

  return (
    <View style={{ flexDirection: 'row', gap: spacing.sm, marginTop: spacing.sm }}>
      {options.map((interval) => {
        const isActive = interval === value;
        return (
          <Pressable
            key={interval}
            onPress={() => onChange(interval)}
            style={{
              flex: 1,
              borderWidth: 1,
              borderColor: isActive ? colors.accent : colors.border,
              borderRadius: 8,
              backgroundColor: isActive ? `${colors.accent}20` : colors.surface,
              alignItems: 'center',
            }}
          >
            <ThemedText
              preset="bodySm"
              style={{
                color: isActive ? colors.accent : colors.textSecondary,
                paddingVertical: 8,
              }}
            >
              {interval}s
            </ThemedText>
          </Pressable>
        );
      })}
    </View>
  );
};

export const SettingsScreen = () => {
  const serverUrl = useSettingsStore((state) => state.serverUrl);
  const rssiScanEnabled = useSettingsStore((state) => state.rssiScanEnabled);
  const theme = useSettingsStore((state) => state.theme);
  const setServerUrl = useSettingsStore((state) => state.setServerUrl);
  const setRssiScanEnabled = useSettingsStore((state) => state.setRssiScanEnabled);
  const setTheme = useSettingsStore((state) => state.setTheme);

  const [draftUrl, setDraftUrl] = useState(serverUrl);
  const [scanInterval, setScanInterval] = useState(2);

  useEffect(() => {
    setDraftUrl(serverUrl);
  }, [serverUrl]);

  const intervalSummary = useMemo(() => `${scanInterval}s`, [scanInterval]);

  const handleSaveUrl = () => {
    const newUrl = draftUrl.trim();
    setServerUrl(newUrl);
    wsService.disconnect();
    wsService.connect(newUrl);
    apiService.setBaseUrl(newUrl);
  };

  const handleOpenGitHub = async () => {
    const handled = await Linking.canOpenURL('https://github.com');
    if (!handled) {
      Alert.alert('Unable to open link', 'Please open https://github.com manually in your browser.');
      return;
    }

    await Linking.openURL('https://github.com');
  };

  return (
    <ThemedView style={{ flex: 1, backgroundColor: colors.bg, padding: spacing.md }}>
      <ScrollView
        contentContainerStyle={{
          paddingBottom: spacing.xl,
        }}
      >
        <GlowCard title="SERVER">
          <ServerUrlInput value={draftUrl} onChange={setDraftUrl} onSave={handleSaveUrl} />
        </GlowCard>

        <GlowCard title="SENSING">
          <RssiToggle enabled={rssiScanEnabled} onChange={setRssiScanEnabled} />
          <ThemedText preset="bodyMd" style={{ marginTop: spacing.md }}>
            Scan interval
          </ThemedText>
          <ScanIntervalPicker value={scanInterval} onChange={setScanInterval} />
          <ThemedText preset="bodySm" style={{ color: colors.textSecondary, marginTop: spacing.sm }}>
            Active interval: {intervalSummary}
          </ThemedText>
          {Platform.OS === 'ios' && (
            <ThemedText preset="bodySm" style={{ color: colors.textSecondary, marginTop: spacing.sm }}>
              iOS: RSSI scanning uses stubbed telemetry in this build.
            </ThemedText>
          )}
        </GlowCard>

        <GlowCard title="APPEARANCE">
          <ThemePicker value={theme} onChange={setTheme} />
        </GlowCard>

        <GlowCard title="ABOUT">
          <ThemedText preset="bodyMd" style={{ marginBottom: spacing.xs }}>
            WiFi-DensePose Mobile v1.0.0
          </ThemedText>
          <ThemedText
            preset="bodySm"
            style={{ color: colors.accent, marginBottom: spacing.sm }}
            onPress={handleOpenGitHub}
          >
            View on GitHub
          </ThemedText>
          <ThemedText preset="bodySm">WebSocket: {WS_PATH}</ThemedText>
          <ThemedText preset="bodySm" style={{ color: colors.textSecondary }}>
            Triage priority mapping: Immediate/Delayed/Minor/Deceased/Unknown
          </ThemedText>
        </GlowCard>
      </ScrollView>
    </ThemedView>
  );
};

export default SettingsScreen;
