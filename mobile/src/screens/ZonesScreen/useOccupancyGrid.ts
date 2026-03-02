import { useMemo } from 'react';
import type { Classification, SignalField } from '@/types/sensing';
import { usePoseStore } from '@/stores/poseStore';

const GRID_SIZE = 20;
const CELL_COUNT = GRID_SIZE * GRID_SIZE;

type Point = {
  x: number;
  y: number;
};

const clamp01 = (value: number): number => {
  if (Number.isNaN(value)) {
    return 0;
  }

  return Math.max(0, Math.min(1, value));
};

const parseNumber = (value: unknown): number | null => {
  return typeof value === 'number' && Number.isFinite(value) ? value : null;
};

const parsePoint = (value: unknown): Point | null => {
  if (!value || typeof value !== 'object') {
    return null;
  }

  const record = value as Record<string, unknown>;
  const x = parseNumber(record.x);
  const y = parseNumber(record.y);

  if (x === null || y === null) {
    return null;
  }

  return {
    x,
    y,
  };
};

const collectPositions = (value: unknown): Point[] => {
  if (!Array.isArray(value)) {
    return [];
  }

  return value
    .map((entry) => parsePoint(entry))
    .filter((point): point is Point => point !== null)
    .map((point) => ({
      x: point.x,
      y: point.y,
    }));
};

const readClassificationPositions = (classification: Classification | undefined): Point[] => {
  const source = classification as unknown as Record<string, unknown>;

  return (
    collectPositions(source?.persons) ??
    collectPositions(source?.personPositions) ??
    collectPositions(source?.positions) ??
    []
  );
};

export const useOccupancyGrid = (signalField: SignalField | null): { gridValues: number[]; personPositions: Point[] } => {
  const classification = usePoseStore((state) => state.classification) as Classification | undefined;

  const gridValues = useMemo(() => {
    const sourceValues = signalField?.values;

    if (!sourceValues || sourceValues.length === 0) {
      return new Array(CELL_COUNT).fill(0);
    }

    const normalized = new Array(CELL_COUNT).fill(0);
    const sourceLength = Math.min(CELL_COUNT, sourceValues.length);

    for (let i = 0; i < sourceLength; i += 1) {
      const value = parseNumber(sourceValues[i]);
      normalized[i] = clamp01(value ?? 0);
    }

    return normalized;
  }, [signalField?.values]);

  const personPositions = useMemo(() => {
    const positions = readClassificationPositions(classification);

    if (positions.length > 0) {
      return positions
        .map(({ x, y }) => ({
          x: Math.max(0, Math.min(GRID_SIZE - 1, x)),
          y: Math.max(0, Math.min(GRID_SIZE - 1, y)),
        }))
        .slice(0, 16);
    }

    return [] as Point[];
  }, [classification]);

  return {
    gridValues,
    personPositions,
  };
};
