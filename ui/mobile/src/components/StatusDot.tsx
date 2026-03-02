import { useEffect } from 'react';
import { StyleSheet, ViewStyle } from 'react-native';
import Animated, {
  cancelAnimation,
  Easing,
  useAnimatedStyle,
  useSharedValue,
  withRepeat,
  withSequence,
  withTiming,
} from 'react-native-reanimated';
import { colors } from '../theme/colors';

type StatusType = 'connected' | 'simulated' | 'disconnected' | 'connecting';

type StatusDotProps = {
  status: StatusType;
  size?: number;
  style?: ViewStyle;
};

const resolveColor = (status: StatusType): string => {
  if (status === 'connecting') return colors.warn;
  return colors[status];
};

export const StatusDot = ({ status, size = 10, style }: StatusDotProps) => {
  const scale = useSharedValue(1);
  const opacity = useSharedValue(1);
  const isConnecting = status === 'connecting';

  useEffect(() => {
    if (isConnecting) {
      scale.value = withRepeat(
        withSequence(
          withTiming(1.35, { duration: 800, easing: Easing.out(Easing.cubic) }),
          withTiming(1, { duration: 800, easing: Easing.in(Easing.cubic) }),
        ),
        -1,
      );
      opacity.value = withRepeat(
        withSequence(
          withTiming(0.4, { duration: 800, easing: Easing.out(Easing.quad) }),
          withTiming(1, { duration: 800, easing: Easing.in(Easing.quad) }),
        ),
        -1,
      );
      return;
    }

    cancelAnimation(scale);
    cancelAnimation(opacity);
    scale.value = 1;
    opacity.value = 1;
  }, [isConnecting, opacity, scale]);

  const animatedStyle = useAnimatedStyle(() => ({
    transform: [{ scale: scale.value }],
    opacity: opacity.value,
  }));

  return (
    <Animated.View
      style={[
        styles.dot,
        {
          width: size,
          height: size,
          backgroundColor: resolveColor(status),
          borderRadius: size / 2,
        },
        animatedStyle,
        style,
      ]}
    />
  );
};

const styles = StyleSheet.create({
  dot: {
    borderRadius: 999,
  },
});
