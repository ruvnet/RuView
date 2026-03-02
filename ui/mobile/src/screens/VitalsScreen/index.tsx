import { useEffect } from 'react';
import { ScrollView, StyleSheet, View } from 'react-native';
import Animated, { useAnimatedStyle, useSharedValue, withSpring } from 'react-native-reanimated';
import { BreathingGauge } from './BreathingGauge';
import { HeartRateGauge } from './HeartRateGauge';
import { MetricCard } from './MetricCard';
import { ConnectionBanner } from '@/components/ConnectionBanner';
import { ModeBadge } from '@/components/ModeBadge';
import { ThemedText } from '@/components/ThemedText';
import { ThemedView } from '@/components/ThemedView';
import { SparklineChart } from '@/components/SparklineChart';
import { usePoseStore } from '@/stores/poseStore';
import { usePoseStream } from '@/hooks/usePoseStream';
import { colors } from '@/theme/colors';

type ConnectionBannerState = 'connected' | 'simulated' | 'disconnected';

const clampPercent = (value: number) => {
  const normalized = Number.isFinite(value) ? value : 0;
  return Math.max(0, Math.min(1, normalized > 1 ? normalized / 100 : normalized));
};

export default function VitalsScreen() {
  usePoseStream();

  const connectionStatus = usePoseStore((state) => state.connectionStatus);
  const isSimulated = usePoseStore((state) => state.isSimulated);
  const features = usePoseStore((state) => state.features);
  const classification = usePoseStore((state) => state.classification);
  const rssiHistory = usePoseStore((state) => state.rssiHistory);

  const confidence = clampPercent(classification?.confidence ?? 0);
  const badgeLabel = (classification?.motion_level ?? 'ABSENT').toUpperCase();

  const bannerStatus: ConnectionBannerState = connectionStatus === 'connected' ? 'connected' : connectionStatus === 'simulated' ? 'simulated' : 'disconnected';

  const confidenceProgress = useSharedValue(0);

  useEffect(() => {
    confidenceProgress.value = withSpring(confidence, {
      damping: 16,
      stiffness: 150,
      mass: 1,
    });
  }, [confidence, confidenceProgress]);

  const animatedConfidenceStyle = useAnimatedStyle(() => ({
    width: `${confidenceProgress.value * 100}%`,
  }));

  const classificationColor =
    classification?.motion_level === 'active'
      ? colors.success
      : classification?.motion_level === 'present_still'
        ? colors.warn
        : colors.muted;

  return (
    <ThemedView style={styles.screen}>
      <ConnectionBanner status={bannerStatus} />

      <ScrollView contentContainerStyle={styles.content} showsVerticalScrollIndicator={false}>
        <View style={styles.headerRow}>{isSimulated ? <ModeBadge mode="SIM" /> : null}</View>

        <View style={styles.gaugesRow}>
          <View style={styles.gaugeCard}>
            <BreathingGauge />
          </View>
          <View style={styles.gaugeCard}>
            <HeartRateGauge />
          </View>
        </View>

        <View style={styles.section}>
          <ThemedText preset="labelLg" color="textSecondary">
            RSSI HISTORY
          </ThemedText>
          <SparklineChart data={rssiHistory.length > 0 ? rssiHistory : [0]} color={colors.accent} />
        </View>

        <MetricCard label="Variance" value={features?.variance ?? 0} unit="" sparklineData={rssiHistory} color={colors.accent} />
        <MetricCard
          label="Motion Band"
          value={features?.motion_band_power ?? 0}
          unit=""
          color={colors.success}
        />
        <MetricCard
          label="Breath Band"
          value={features?.breathing_band_power ?? 0}
          unit=""
          color={colors.warn}
        />
        <MetricCard
          label="Spectral Entropy"
          value={features?.spectral_entropy ?? 0}
          unit=""
          color={colors.connected}
        />

        <View style={styles.classificationSection}>
          <ThemedText preset="labelLg" style={styles.rowLabel}>
            Classification: {badgeLabel}
          </ThemedText>
          <View style={[styles.badgePill, { borderColor: classificationColor, backgroundColor: `${classificationColor}18` }]}>
            <ThemedText preset="labelMd" style={{ color: classificationColor }}>
              {badgeLabel}
            </ThemedText>
          </View>
          <View style={styles.confidenceContainer}>
            <ThemedText preset="bodySm" color="textSecondary">
              Confidence
            </ThemedText>
            <View style={styles.confidenceBarTrack}>
              <Animated.View style={[styles.confidenceBarFill, animatedConfidenceStyle]} />
            </View>
            <ThemedText preset="bodySm">{Math.round(confidence * 100)}%</ThemedText>
          </View>
        </View>
      </ScrollView>
    </ThemedView>
  );
}

const styles = StyleSheet.create({
  screen: {
    flex: 1,
    backgroundColor: colors.bg,
    paddingTop: 40,
    paddingHorizontal: 12,
  },
  content: {
    paddingTop: 12,
    paddingBottom: 30,
    gap: 12,
  },
  headerRow: {
    alignItems: 'flex-end',
  },
  gaugesRow: {
    flexDirection: 'row',
    gap: 12,
  },
  gaugeCard: {
    flex: 1,
    backgroundColor: '#111827',
    borderRadius: 16,
    borderWidth: 1,
    borderColor: 'rgba(50,184,198,0.45)',
    paddingVertical: 10,
    paddingHorizontal: 8,
    alignItems: 'center',
    justifyContent: 'center',
    shadowColor: colors.accent,
    shadowOpacity: 0.3,
    shadowOffset: {
      width: 0,
      height: 0,
    },
    shadowRadius: 12,
    elevation: 4,
  },
  section: {
    backgroundColor: colors.surface,
    borderRadius: 14,
    borderWidth: 1,
    borderColor: 'rgba(50,184,198,0.35)',
    padding: 12,
    gap: 10,
  },
  classificationSection: {
    backgroundColor: colors.surface,
    borderRadius: 14,
    borderWidth: 1,
    borderColor: 'rgba(50,184,198,0.35)',
    padding: 12,
    gap: 10,
    marginBottom: 6,
  },
  rowLabel: {
    color: colors.textSecondary,
    marginBottom: 8,
  },
  badgePill: {
    alignSelf: 'flex-start',
    borderWidth: 1,
    borderRadius: 999,
    paddingHorizontal: 10,
    paddingVertical: 4,
    marginBottom: 4,
  },
  confidenceContainer: {
    gap: 6,
  },
  confidenceBarTrack: {
    height: 10,
    borderRadius: 999,
    backgroundColor: colors.surfaceAlt,
    overflow: 'hidden',
  },
  confidenceBarFill: {
    height: '100%',
    backgroundColor: colors.success,
    borderRadius: 999,
  },
});
