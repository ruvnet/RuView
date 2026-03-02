import { useMemo } from 'react';
import { View, StyleSheet } from 'react-native';
import { usePoseStore } from '@/stores/poseStore';
import { GaugeArc } from '@/components/GaugeArc';
import { colors } from '@/theme/colors';
import { ThemedText } from '@/components/ThemedText';

const BREATHING_MIN_BPM = 0;
const BREATHING_MAX_BPM = 30;
const BREATHING_BAND_MAX = 0.3;

const clamp = (value: number, min: number, max: number) => Math.max(min, Math.min(max, value));

const deriveBreathingValue = (
  breathingBand?: number,
  breathingBpm?: number,
): number => {
  if (typeof breathingBpm === 'number' && Number.isFinite(breathingBpm)) {
    return clamp(breathingBpm, BREATHING_MIN_BPM, BREATHING_MAX_BPM);
  }

  const bandValue = typeof breathingBand === 'number' && Number.isFinite(breathingBand) ? breathingBand : 0;
  const normalized = clamp(bandValue / BREATHING_BAND_MAX, 0, 1);
  return normalized * BREATHING_MAX_BPM;
};

export const BreathingGauge = () => {
  const breathingBand = usePoseStore((state) => state.features?.breathing_band_power);
  const breathingBpm = usePoseStore((state) => state.lastFrame?.vital_signs?.breathing_bpm);

  const value = useMemo(
    () => deriveBreathingValue(breathingBand, breathingBpm),
    [breathingBand, breathingBpm],
  );

  return (
    <View style={styles.container}>
      <ThemedText preset="labelMd" style={styles.label}>
        BREATHING
      </ThemedText>
      <GaugeArc value={value} min={BREATHING_MIN_BPM} max={BREATHING_MAX_BPM} label="" unit="BPM" color={colors.accent} />
      <ThemedText preset="labelMd" color="textSecondary" style={styles.unit}>
        BPM
      </ThemedText>
    </View>
  );
};

const styles = StyleSheet.create({
  container: {
    alignItems: 'center',
    justifyContent: 'center',
    gap: 6,
  },
  label: {
    color: '#94A3B8',
    letterSpacing: 1,
  },
  unit: {
    marginTop: -12,
    marginBottom: 4,
  },
});
