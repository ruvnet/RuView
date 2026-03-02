import { useEffect, useMemo, useRef } from 'react';
import { StyleProp, ViewStyle } from 'react-native';
import Animated, { interpolateColor, useAnimatedProps, useSharedValue, withTiming, type SharedValue } from 'react-native-reanimated';
import Svg, { Circle, G, Rect } from 'react-native-svg';
import { colors } from '../theme/colors';

type Point = {
  x: number;
  y: number;
};

type OccupancyGridProps = {
  values: number[];
  personPositions?: Point[];
  size?: number;
  style?: StyleProp<ViewStyle>;
};

const GRID_DIMENSION = 20;
const CELLS = GRID_DIMENSION * GRID_DIMENSION;

const toColor = (value: number): string => {
  const clamped = Math.max(0, Math.min(1, value));
  let r: number;
  let g: number;
  let b: number;

  if (clamped < 0.5) {
    const t = clamped * 2;
    r = Math.round(255 * 0);
    g = Math.round(255 * t);
    b = Math.round(255 * (1 - t));
  } else {
    const t = (clamped - 0.5) * 2;
    r = Math.round(255 * t);
    g = Math.round(255 * (1 - t));
    b = 0;
  }

  return `rgb(${r}, ${g}, ${b})`;
};

const AnimatedRect = Animated.createAnimatedComponent(Rect);

const normalizeValues = (values: number[]) => {
  const normalized = new Array(CELLS).fill(0);
  for (let i = 0; i < CELLS; i += 1) {
    const value = values?.[i] ?? 0;
    normalized[i] = Number.isFinite(value) ? Math.max(0, Math.min(1, value)) : 0;
  }
  return normalized;
};

type CellProps = {
  index: number;
  size: number;
  progress: SharedValue<number>;
  previousColors: string[];
  nextColors: string[];
};

const Cell = ({ index, size, progress, previousColors, nextColors }: CellProps) => {
  const col = index % GRID_DIMENSION;
  const row = Math.floor(index / GRID_DIMENSION);
  const cellSize = size / GRID_DIMENSION;
  const x = col * cellSize;
  const y = row * cellSize;

  const animatedProps = useAnimatedProps(() => ({
    fill: interpolateColor(
      progress.value,
      [0, 1],
      [previousColors[index] ?? colors.surfaceAlt, nextColors[index] ?? colors.surfaceAlt],
    ),
  }));

  return (
    <AnimatedRect
      x={x}
      y={y}
      width={cellSize}
      height={cellSize}
      rx={1}
      animatedProps={animatedProps}
    />
  );
};

export const OccupancyGrid = ({
  values,
  personPositions = [],
  size = 320,
  style,
}: OccupancyGridProps) => {
  const normalizedValues = useMemo(() => normalizeValues(values), [values]);
  const previousColors = useRef<string[]>(normalizedValues.map(toColor));
  const nextColors = useRef<string[]>(normalizedValues.map(toColor));
  const progress = useSharedValue(1);

  useEffect(() => {
    const next = normalizeValues(values);
    previousColors.current = normalizedValues.map(toColor);
    nextColors.current = next.map(toColor);
    progress.value = 0;
    progress.value = withTiming(1, { duration: 500 });
  }, [values, normalizedValues, progress]);

  const markers = useMemo(() => {
    const cellSize = size / GRID_DIMENSION;
    return personPositions.map(({ x, y }, idx) => {
      const clampedX = Math.max(0, Math.min(GRID_DIMENSION - 1, Math.round(x)));
      const clampedY = Math.max(0, Math.min(GRID_DIMENSION - 1, Math.round(y)));
      const cx = (clampedX + 0.5) * cellSize;
      const cy = (clampedY + 0.5) * cellSize;
      const markerRadius = Math.max(3, cellSize * 0.25);
      return (
        <Circle
          key={`person-${idx}`}
          cx={cx}
          cy={cy}
          r={markerRadius}
          fill={colors.accent}
          stroke={colors.textPrimary}
          strokeWidth={1}
        />
      );
    });
  }, [personPositions, size]);

  return (
    <Svg width={size} height={size} style={style} viewBox={`0 0 ${size} ${size}`}>
      <G>
        {Array.from({ length: CELLS }).map((_, index) => (
          <Cell
            key={index}
            index={index}
            size={size}
            progress={progress}
            previousColors={previousColors.current}
            nextColors={nextColors.current}
          />
        ))}
      </G>
      {markers}
    </Svg>
  );
};
