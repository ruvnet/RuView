import React from 'react';
import { render } from '@testing-library/react-native';
import { ThemeProvider } from '@/theme/ThemeContext';

jest.mock('react-native-svg', () => {
  const { View } = require('react-native');
  return {
    __esModule: true,
    default: View,
    Svg: View,
    Circle: View,
    G: View,
    Text: View,
    Rect: View,
    Line: View,
    Path: View,
  };
});

import { OccupancyGrid } from '@/components/OccupancyGrid';

const renderWithTheme = (ui: React.ReactElement) =>
  render(<ThemeProvider>{ui}</ThemeProvider>);

describe('OccupancyGrid', () => {
  it('renders without crashing with empty values', () => {
    const { toJSON } = renderWithTheme(<OccupancyGrid values={[]} />);
    expect(toJSON()).not.toBeNull();
  });

  it('renders with a full 400-element values array', () => {
    const values = new Array(400).fill(0.5);
    const { toJSON } = renderWithTheme(<OccupancyGrid values={values} />);
    expect(toJSON()).not.toBeNull();
  });

  it('renders with person positions', () => {
    const values = new Array(400).fill(0.3);
    const positions = [
      { x: 5, y: 5 },
      { x: 15, y: 10 },
    ];
    const { toJSON } = renderWithTheme(
      <OccupancyGrid values={values} personPositions={positions} />,
    );
    expect(toJSON()).not.toBeNull();
  });

  it('renders with custom size', () => {
    const values = new Array(400).fill(0);
    const { toJSON } = renderWithTheme(
      <OccupancyGrid values={values} size={200} />,
    );
    expect(toJSON()).not.toBeNull();
  });

  it('handles values outside 0-1 range by clamping', () => {
    const values = [-0.5, 0, 0.5, 1.5, NaN, 2, ...new Array(394).fill(0)];
    const { toJSON } = renderWithTheme(<OccupancyGrid values={values} />);
    expect(toJSON()).not.toBeNull();
  });
});
