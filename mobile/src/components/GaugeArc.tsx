import { useEffect, useMemo } from 'react';
import { StyleSheet, View } from 'react-native';
import Animated, { interpolateColor, useAnimatedProps, useSharedValue, withSpring } from 'react-native-reanimated';
import Svg, { Circle, G, Text as SvgText } from 'react-native-svg';

type GaugeArcProps = {
  value: number;
  min?: number;
  max: number;
  label: string;
  unit: string;
  color: string;
  colorTo?: string;
  size?: number;
};

const AnimatedCircle = Animated.createAnimatedComponent(Circle);

const clamp = (value: number, min: number, max: number) => Math.max(min, Math.min(max, value));

export const GaugeArc = ({ value, min = 0, max, label, unit, color, colorTo, size = 140 }: GaugeArcProps) => {
  const radius = (size - 20) / 2;
  const circumference = 2 * Math.PI * radius;
  const arcLength = circumference * 0.75;
  const strokeWidth = 12;
  const progress = useSharedValue(0);

  const normalized = useMemo(() => {
    const span = max - min;
    const safeSpan = span > 0 ? span : 1;
    return clamp((value - min) / safeSpan, 0, 1);
  }, [value, min, max]);

  const displayValue = useMemo(() => {
    if (!Number.isFinite(value)) {
      return '--';
    }
    return `${Math.max(min, Math.min(max, value)).toFixed(1)} ${unit}`;
  }, [max, min, unit, value]);

  useEffect(() => {
    progress.value = withSpring(normalized, {
      damping: 16,
      stiffness: 140,
      mass: 1,
    });
  }, [normalized, progress]);

  const animatedStroke = useAnimatedProps(() => {
    const dashOffset = arcLength - arcLength * progress.value;
    const strokeColor = colorTo ? interpolateColor(progress.value, [0, 1], [color, colorTo]) : color;

    return {
      strokeDashoffset: dashOffset,
      stroke: strokeColor,
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
          y={size / 2 - 8}
          fill="#E2E8F0"
          fontSize={Math.round(size * 0.16)}
          fontFamily="Courier New"
          fontWeight="700"
          textAnchor="middle"
        >
          {displayValue}
        </SvgText>
        <SvgText
          x={size / 2}
          y={size / 2 + 18}
          fill="#94A3B8"
          fontSize={Math.round(size * 0.085)}
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
