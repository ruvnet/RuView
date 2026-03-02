import { useMemo } from 'react';
import { ScrollView, useWindowDimensions, View } from 'react-native';
import { ConnectionBanner } from '@/components/ConnectionBanner';
import { ThemedText } from '@/components/ThemedText';
import { ThemedView } from '@/components/ThemedView';
import { colors } from '@/theme/colors';
import { spacing } from '@/theme/spacing';
import { usePoseStore } from '@/stores/poseStore';
import { type ConnectionStatus } from '@/types/sensing';
import { useOccupancyGrid } from './useOccupancyGrid';
import { FloorPlanSvg } from './FloorPlanSvg';
import { ZoneLegend } from './ZoneLegend';

const getLastUpdateSeconds = (timestamp?: number): string => {
  if (!timestamp) {
    return 'N/A';
  }

  const ageMs = Date.now() - timestamp;
  const secs = Math.max(0, ageMs / 1000);
  return `${secs.toFixed(1)}s`;
};

const resolveBannerState = (status: ConnectionStatus): 'connected' | 'simulated' | 'disconnected' => {
  if (status === 'connecting') {
    return 'disconnected';
  }

  return status;
};

export const ZonesScreen = () => {
  const connectionStatus = usePoseStore((state) => state.connectionStatus);
  const lastFrame = usePoseStore((state) => state.lastFrame);
  const signalField = usePoseStore((state) => state.signalField);

  const { gridValues, personPositions } = useOccupancyGrid(signalField);

  const { width } = useWindowDimensions();
  const mapSize = useMemo(() => Math.max(240, Math.min(width - spacing.md * 2, 520)), [width]);

  return (
    <ThemedView style={{ flex: 1, backgroundColor: colors.bg }}>
      <ScrollView contentContainerStyle={{ padding: spacing.md, paddingBottom: spacing.xxl }}>
        <ConnectionBanner status={resolveBannerState(connectionStatus)} />
        <View
          style={{
            marginTop: 28,
            marginBottom: spacing.md,
          }}
        >
          <ThemedText preset="labelLg" style={{ color: colors.textSecondary, marginBottom: 8 }}>
            Floor Plan — Occupancy Heatmap
          </ThemedText>
        </View>

        <FloorPlanSvg
          gridValues={gridValues}
          personPositions={personPositions}
          size={mapSize}
          style={{ alignSelf: 'center' }}
        />

        <ZoneLegend />

        <View
          style={{
            marginTop: spacing.md,
            flexDirection: 'row',
            justifyContent: 'space-between',
            gap: spacing.md,
          }}
        >
          <ThemedText preset="bodyMd">Occupancy: {personPositions.length} persons detected</ThemedText>
          <ThemedText preset="bodyMd">Last update: {getLastUpdateSeconds(lastFrame?.timestamp)}</ThemedText>
        </View>
      </ScrollView>
    </ThemedView>
  );
};

export default ZonesScreen;
