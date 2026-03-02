import { useMemo } from 'react';
import { StyleSheet, View } from 'react-native';
import { usePoseStore } from '@/stores/poseStore';
import { GaugeArc } from '@/components/GaugeArc';
import { colors } from '@/theme/colors';
import { ThemedText } from '@/components/ThemedText';

const HEART_MIN_BPM = 40;
const HEART_MAX_BPM = 120;
const MOTION_BAND_MAX = 0.5;
const BREATH_BAND_MAX = 0.3;

const clamp = (value: number, min: number, max: number) => Math.max(min, Math.min(max, value));

const deriveHeartRate = (
  heartbeat?: number,
  motionBand?: number,
  breathingBand?: number,
): number => {
  if (typeof heartbeat === 'number' && Number.isFinite(heartbeat)) {
    return clamp(heartbeat, HEART_MIN_BPM, HEART_MAX_BPM);
  }

  const motionValue = typeof motionBand === 'number' && Number.isFinite(motionBand) ? clamp(motionBand / MOTION_BAND_MAX, 0, 1) : 0;
  const breathValue = typeof breathingBand === 'number' && Number.isFinite(breathingBand) ? clamp(breathingBand / BREATH_BAND_MAX, 0, 1) : 0;

  const normalized = 0.7 * motionValue + 0.3 * breathValue;
  return HEART_MIN_BPM + normalized * (HEART_MAX_BPM - HEART_MIN_BPM);
};

export const HeartRateGauge = () => {
  const heartProxyBpm = usePoseStore((state) => state.lastFrame?.vital_signs?.hr_proxy_bpm);
  const motionBand = usePoseStore((state) => state.features?.motion_band_power);
  const breathingBand = usePoseStore((state) => state.features?.breathing_band_power);

  const value = useMemo(
    () => deriveHeartRate(heartProxyBpm, motionBand, breathingBand),
    [heartProxyBpm, motionBand, breathingBand],
  );

  return (
    <View style={styles.container}>
      <ThemedText preset="labelMd" style={styles.label}>
        HR PROXY
      </ThemedText>
      <GaugeArc
        value={value}
        min={HEART_MIN_BPM}
        max={HEART_MAX_BPM}
        label=""
        unit="BPM"
        color={colors.danger}
        colorTo={colors.success}
      />
      <ThemedText preset="bodySm" color="textSecondary" style={styles.note}>
        (estimated)
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
  note: {
    marginTop: -12,
    marginBottom: 4,
  },
});
