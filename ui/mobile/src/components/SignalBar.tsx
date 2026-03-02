import { useEffect } from 'react';
import { StyleSheet, View } from 'react-native';
import Animated, { useAnimatedStyle, useSharedValue, withTiming } from 'react-native-reanimated';
import { ThemedText } from './ThemedText';
import { colors } from '../theme/colors';

type SignalBarProps = {
  value: number;
  label: string;
  color?: string;
};

const clamp01 = (value: number) => Math.max(0, Math.min(1, value));

export const SignalBar = ({ value, label, color = colors.accent }: SignalBarProps) => {
  const progress = useSharedValue(clamp01(value));

  useEffect(() => {
    progress.value = withTiming(clamp01(value), { duration: 250 });
  }, [value, progress]);

  const animatedFill = useAnimatedStyle(() => ({
    width: `${progress.value * 100}%`,
  }));

  return (
    <View style={styles.container}>
      <ThemedText preset="bodySm" style={styles.label}>
        {label}
      </ThemedText>
      <View style={styles.track}>
        <Animated.View style={[styles.fill, { backgroundColor: color }, animatedFill]} />
      </View>
      <ThemedText preset="bodySm" style={styles.percent}>
        {Math.round(clamp01(value) * 100)}%
      </ThemedText>
    </View>
  );
};

const styles = StyleSheet.create({
  container: {
    gap: 6,
  },
  label: {
    marginBottom: 4,
  },
  track: {
    height: 8,
    borderRadius: 4,
    backgroundColor: colors.surfaceAlt,
    overflow: 'hidden',
  },
  fill: {
    height: '100%',
    borderRadius: 4,
  },
  percent: {
    textAlign: 'right',
    color: colors.textSecondary,
  },
});
