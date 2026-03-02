import { useMemo } from 'react';
import { View, ViewStyle } from 'react-native';
import { colors } from '../theme/colors';

type SparklineChartProps = {
  data: number[];
  color?: string;
  height?: number;
  style?: ViewStyle;
};

const defaultHeight = 72;

export const SparklineChart = ({
  data,
  color = colors.accent,
  height = defaultHeight,
  style,
}: SparklineChartProps) => {
  const normalizedData = data.length > 0 ? data : [0];

  const chartData = useMemo(
    () =>
      normalizedData.map((value, index) => ({
        x: index,
        y: value,
      })),
    [normalizedData],
  );

  const yValues = normalizedData.map((value) => Number(value) || 0);
  const yMin = Math.min(...yValues);
  const yMax = Math.max(...yValues);
  const yPadding = yMax - yMin === 0 ? 1 : (yMax - yMin) * 0.2;

  return (
    <View style={style}>
      <View
        accessibilityRole="image"
        style={{
          height,
          width: '100%',
          borderRadius: 4,
          borderWidth: 1,
          borderColor: color,
          opacity: 0.2,
          backgroundColor: 'transparent',
        }}
      >
        <View
          style={{
            flex: 1,
            justifyContent: 'center',
            alignItems: 'center',
          }}
        >
          {chartData.map((point) => (
            <View key={point.x} style={{ position: 'absolute', left: `${(point.x / Math.max(normalizedData.length - 1, 1)) * 100}%` }} />
          ))}
        </View>
      </View>
    </View>
  );
};
