import { useEffect } from 'react';
import { StyleSheet, View } from 'react-native';
import Animated, { useAnimatedProps, useSharedValue, withTiming } from 'react-native-reanimated';
import Svg, { Circle, G, Text as SvgText } from 'react-native-svg';

type GaugeArcProps = {
  value: number;
  max: number;
  label: string;
  unit: string;
  color: string;
  size?: number;
};

const AnimatedCircle = Animated.createAnimatedComponent(Circle);

export const GaugeArc = ({ value, max, label, unit, color, size = 140 }: GaugeArcProps) => {
  const radius = (size - 20) / 2;
  const circumference = 2 * Math.PI * radius;
  const arcLength = circumference * 0.75;
  const strokeWidth = 12;
  const progress = useSharedValue(0);

  const normalized = Math.max(0, Math.min(max > 0 ? value / max : 0, 1));
  const displayText = `${value.toFixed(1)} ${unit}`;

  useEffect(() => {
    progress.value = withTiming(normalized, { duration: 600 });
  }, [normalized, progress]);

  const animatedStroke = useAnimatedProps(() => {
    const dashOffset = arcLength - arcLength * progress.value;
    return {
      strokeDashoffset: dashOffset,
    };
  });

  return (
    <View style={styles.wrapper}>
      <Svg width={size} height={size} viewBox={`0 0 ${size} ${size}`}>
        <G transform={`rotate(-135 ${size / 2} ${size / 2})`}>
          <Circle
            cx={size / 2}
            cy={size / 2}
            r={radius}
            strokeWidth={strokeWidth}
            stroke="#1E293B"
            fill="none"
            strokeDasharray={`${arcLength} ${circumference}`}
            strokeLinecap="round"
          />
          <AnimatedCircle
            cx={size / 2}
            cy={size / 2}
            r={radius}
            strokeWidth={strokeWidth}
            stroke={color}
            fill="none"
            strokeDasharray={`${arcLength} ${circumference}`}
            strokeLinecap="round"
            animatedProps={animatedStroke}
          />
        </G>
        <SvgText
          x={size / 2}
          y={size / 2 - 4}
          fill="#E2E8F0"
          fontSize={18}
          fontFamily="Courier New"
          fontWeight="700"
          textAnchor="middle"
        >
          {displayText}
        </SvgText>
        <SvgText
          x={size / 2}
          y={size / 2 + 16}
          fill="#94A3B8"
          fontSize={10}
          fontFamily="Courier New"
          textAnchor="middle"
          letterSpacing="0.6"
        >
          {label}
        </SvgText>
      </Svg>
    </View>
  );
};

const styles = StyleSheet.create({
  wrapper: {
    alignItems: 'center',
    justifyContent: 'center',
  },
});
