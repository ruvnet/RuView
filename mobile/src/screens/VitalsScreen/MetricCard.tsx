import { useEffect, useMemo, useState } from 'react';
import { StyleSheet, View } from 'react-native';
import {
  runOnJS,
  useAnimatedReaction,
  useSharedValue,
  withSpring,
} from 'react-native-reanimated';
import { SparklineChart } from '@/components/SparklineChart';
import { ThemedText } from '@/components/ThemedText';
import { colors } from '@/theme/colors';

type MetricCardProps = {
  label: string;
  value: number | string;
  unit?: string;
  color?: string;
  sparklineData?: number[];
};

const formatMetricValue = (value: number, unit?: string) => {
  if (!Number.isFinite(value)) {
    return '--';
  }
  const decimals = Math.abs(value) >= 100 ? 0 : Math.abs(value) >= 10 ? 2 : 3;
  const text = value.toFixed(decimals);
  return unit ? `${text} ${unit}` : text;
};

export const MetricCard = ({ label, value, unit, color = colors.accent, sparklineData }: MetricCardProps) => {
  const numericValue = typeof value === 'number' ? value : null;
  const [displayValue, setDisplayValue] = useState(() =>
    numericValue !== null ? formatMetricValue(numericValue, unit) : String(value ?? '--'),
  );

  const valueAnimation = useSharedValue(numericValue ?? 0);

  const finalValue = useMemo(
    () => (numericValue !== null ? numericValue : NaN),
    [numericValue],
  );

  useEffect(() => {
    if (numericValue === null) {
      setDisplayValue(String(value ?? '--'));
      return;
    }

    valueAnimation.value = withSpring(finalValue, {
      damping: 18,
      stiffness: 160,
      mass: 1,
    });
  }, [finalValue, numericValue, value, valueAnimation]);

  useAnimatedReaction(
    () => valueAnimation.value,
    (current) => {
      runOnJS(setDisplayValue)(formatMetricValue(current, unit));
    },
    [unit],
  );

  return (
    <View style={[styles.card, { borderColor: color, shadowColor: color, shadowOpacity: 0.35 }]} accessibilityRole="summary">
      <ThemedText preset="labelMd" style={styles.label}>
        {label}
      </ThemedText>
      <ThemedText preset="displayMd" style={styles.value}>
        {displayValue}
      </ThemedText>
      {sparklineData && sparklineData.length > 0 && (
        <View style={styles.sparklineWrap}>
          <SparklineChart data={sparklineData} color={color} height={56} />
        </View>
      )}
    </View>
  );
};

const styles = StyleSheet.create({
  card: {
    backgroundColor: colors.surface,
    borderWidth: 1,
    borderRadius: 14,
    padding: 12,
    marginBottom: 10,
    gap: 6,
    shadowOffset: {
      width: 0,
      height: 0,
    },
    shadowRadius: 12,
    elevation: 4,
  },
  label: {
    color: colors.textSecondary,
    textTransform: 'uppercase',
    letterSpacing: 0.8,
  },
  value: {
    color: colors.textPrimary,
    marginBottom: 2,
  },
  sparklineWrap: {
    marginTop: 4,
    borderTopWidth: 1,
    borderTopColor: colors.border,
    paddingTop: 8,
  },
});
